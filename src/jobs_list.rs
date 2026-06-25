use crate::error::ScriptHerderError;
use crate::job::Job;
use std::collections::HashMap;

pub struct JobsList {
    pub jobs: Vec<Job>,
}

impl JobsList {
    pub fn from_jobs(mut jobs: Vec<Job>, load_not_running: bool) -> JobsList {
        jobs.sort_by(|a, b| {
            let sa = a.start_time().unwrap_or(0.0);
            let sb = b.start_time().unwrap_or(0.0);
            sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
        });
        let _ = load_not_running; // only meaningful in from_dir
        JobsList { jobs }
    }

    pub fn from_dir(
        datadir: &str,
        checkdir: &str,
        names: &[String],
        load_not_running: bool,
    ) -> Result<JobsList, ScriptHerderError> {
        let mut jobs: Vec<Job> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(datadir) {
            for e in entries.flatten() {
                let path = e.path();
                if !path.is_file() {
                    continue;
                }
                let fname = path.to_string_lossy().to_string();
                if !fname.ends_with(".json") {
                    continue;
                }
                match Job::from_file(&fname) {
                    Ok(job) => {
                        if !names.is_empty() && names != ["ALL"] && !names.contains(&job.name()) {
                            continue;
                        }
                        jobs.push(job);
                    }
                    Err(exc) => {
                        log::warn!(
                            "Failed loading job file {:?} ({})",
                            exc.filename(),
                            exc.reason()
                        );
                    }
                }
            }
        }
        let mut list = JobsList::from_jobs(jobs, load_not_running);
        if load_not_running {
            list.load_not_running(checkdir, names);
        }
        Ok(list)
    }

    fn load_not_running(&mut self, checkdir: &str, names: &[String]) {
        let present: std::collections::HashSet<String> =
            self.jobs.iter().map(|j| j.name()).collect();
        if let Ok(entries) = std::fs::read_dir(checkdir) {
            for e in entries.flatten() {
                let path = e.path();
                if !path.is_file() {
                    continue;
                }
                let fname = path.file_name().unwrap().to_string_lossy().to_string();
                if !fname.ends_with(".ini") {
                    continue;
                }
                let name = fname[..fname.len() - 4].to_string();
                if !names.is_empty() && names != ["ALL"] && !names.contains(&name) {
                    continue;
                }
                if !present.contains(&name) {
                    if let Ok(job) = Job::new(&name, vec![]) {
                        self.jobs.push(job);
                    }
                }
            }
        }
    }

    /// Group job indices by name, preserving first-seen insertion order (matches Python dict order).
    pub fn by_name_ordered(&self) -> Vec<(String, Vec<usize>)> {
        let mut idx: HashMap<String, usize> = HashMap::new();
        let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
        for (i, job) in self.jobs.iter().enumerate() {
            let name = job.name();
            match idx.get(&name) {
                Some(&g) => groups[g].1.push(i),
                None => {
                    idx.insert(name.clone(), groups.len());
                    groups.push((name, vec![i]));
                }
            }
        }
        groups
    }

    pub fn last_of_each(&self) -> Vec<usize> {
        self.by_name_ordered()
            .into_iter()
            .map(|(_, v)| *v.last().unwrap())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::Job;

    #[test]
    fn groups_and_sorts_by_start_time() {
        let mut j1 = Job::new("a", vec!["/bin/true".into()]).unwrap();
        let mut j2 = Job::new("a", vec!["/bin/true".into()]).unwrap();
        j1.set_times_for_test(100.0, 101.0);
        j2.set_times_for_test(200.0, 201.0);
        let list = JobsList::from_jobs(vec![j2, j1], false); // out of order
        let grouped = list.by_name_ordered();
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped[0].0, "a");
        // sorted oldest-first: index 0 should be the start_time=100 job
        let last = list.last_of_each();
        assert_eq!(list.jobs[last[0]].start_time(), Some(200.0));
    }
}
