use crate::check::Check;
use crate::error::ScriptHerderError;
use crate::job::Job;
use crate::jobs_list::JobsList;
use crate::util::status_summary;
use std::collections::HashMap;

/// Aggregated status of job invocations for --mode check (../src/scriptherder.py:805-963).
pub struct CheckStatus {
    checks_ok: Vec<Job>,
    checks_warning: Vec<Job>,
    checks_unknown: Vec<Job>,
    checks_critical: Vec<Job>,
    checks: HashMap<String, Check>,
    runtime_mode: bool,
    checkdir: String,
    last_num_checked: usize,
}

impl CheckStatus {
    pub fn new(
        runtime_mode: bool,
        checkdir: String,
        checks: HashMap<String, Check>,
    ) -> CheckStatus {
        CheckStatus {
            checks_ok: vec![],
            checks_warning: vec![],
            checks_unknown: vec![],
            checks_critical: vec![],
            checks,
            runtime_mode,
            checkdir,
            last_num_checked: 0,
        }
    }

    /// Load and cache the evaluation criteria for this job name.
    pub fn get_check(&mut self, name: &str) -> Result<&Check, ScriptHerderError> {
        if !self.checks.contains_key(name) {
            let path = std::path::Path::new(&self.checkdir).join(format!("{name}.ini"));
            let p = path.to_string_lossy().to_string();
            let check = Check::from_file(&p, self.runtime_mode)
                .map_err(|_| ScriptHerderError::check_load("Failed loading check", p))?;
            self.checks.insert(name.to_string(), check);
        }
        Ok(self.checks.get(name).unwrap())
    }

    pub fn check_jobs(&mut self, jobs: JobsList) {
        self.checks_ok.clear();
        self.checks_warning.clear();
        self.checks_unknown.clear();
        self.checks_critical.clear();

        let groups = jobs.by_name_ordered();
        self.last_num_checked = groups.len();
        let mut all_jobs: Vec<Option<Job>> = jobs.jobs.into_iter().map(Some).collect();

        for (name, mut indices) in groups {
            // Load+cache and clone the check so we can mutate jobs freely.
            let check = match self.get_check(&name) {
                Ok(c) => c.clone(),
                Err(_) => {
                    let i = *indices.last().unwrap();
                    let mut job = all_jobs[i].take().unwrap();
                    job.set_check_status("UNKNOWN").unwrap();
                    job.set_check_reason("Failed to load check".into());
                    self.checks_unknown.push(job);
                    continue;
                }
            };
            let oldest = indices[0];
            indices.reverse(); // most recent first
            let mut matched = false;
            for i in indices {
                let job = all_jobs[i].as_mut().unwrap();
                job.check(&check);
                if job.is_ok() {
                    self.checks_ok.push(all_jobs[i].take().unwrap());
                    matched = true;
                    break;
                } else if job.is_warning() {
                    self.checks_warning.push(all_jobs[i].take().unwrap());
                    matched = true;
                    break;
                } else if job.is_critical() {
                    self.checks_critical.push(all_jobs[i].take().unwrap());
                    matched = true;
                    break;
                }
            }
            if !matched {
                self.checks_critical.push(all_jobs[oldest].take().unwrap());
            }
        }
    }

    pub fn num_jobs(&self) -> usize {
        self.last_num_checked
    }

    pub fn aggregate_status(&self) -> (String, Option<String>) {
        if self.num_jobs() == 1 {
            if let Some(j) = self.checks_ok.last() {
                return ("OK".into(), j.check_reason().map(String::from));
            }
            if let Some(j) = self.checks_warning.last() {
                return ("WARNING".into(), j.check_reason().map(String::from));
            }
            if let Some(j) = self.checks_critical.last() {
                return ("CRITICAL".into(), j.check_reason().map(String::from));
            }
            if let Some(j) = self.checks_unknown.last() {
                return ("UNKNOWN".into(), j.check_reason().map(String::from));
            }
            return ("FAIL".into(), Some("No jobs found?".into()));
        }
        if !self.checks_critical.is_empty() {
            return (
                "CRITICAL".into(),
                Some(status_summary(self.num_jobs(), &self.checks_critical)),
            );
        }
        if !self.checks_warning.is_empty() {
            return (
                "WARNING".into(),
                Some(status_summary(self.num_jobs(), &self.checks_warning)),
            );
        }
        if !self.checks_unknown.is_empty() {
            return (
                "UNKNOWN".into(),
                Some(status_summary(self.num_jobs(), &self.checks_unknown)),
            );
        }
        if !self.checks_ok.is_empty() {
            return (
                "OK".into(),
                Some(status_summary(self.num_jobs(), &self.checks_ok)),
            );
        }
        ("UNKNOWN".into(), Some("No jobs found?".into()))
    }
}
