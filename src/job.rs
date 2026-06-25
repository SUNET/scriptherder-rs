use crate::error::ScriptHerderError;
use crate::util::time_to_str;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobData {
    pub version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub cmd: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_time: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_status: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_size: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_reason: Option<String>,
}

pub const EXIT_STATUS_NAMES: [&str; 4] = ["OK", "WARNING", "CRITICAL", "UNKNOWN"];

pub struct Job {
    pub data: JobData,
    output: Option<Vec<u8>>,
}

impl Job {
    pub fn new(name: &str, cmd: Vec<String>) -> Result<Job, ScriptHerderError> {
        let mut data = JobData {
            version: 2,
            name: if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            },
            cmd: cmd.clone(),
            start_time: None,
            end_time: None,
            exit_status: None,
            pid: None,
            filename: None,
            output: None,
            output_filename: None,
            output_size: None,
            check_status: None,
            check_reason: None,
        };
        if data.name.is_none() && !cmd.is_empty() {
            // basename of cmd[0]
            let base = std::path::Path::new(&cmd[0])
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(&cmd[0])
                .to_string();
            data.name = Some(base);
        }
        Ok(Job { data, output: None })
    }

    pub fn from_data(data: JobData) -> Result<Job, ScriptHerderError> {
        if data.version != 1 && data.version != 2 {
            return Err(ScriptHerderError::job_load(
                format!("Unknown version: {}", data.version),
                data.filename.clone().unwrap_or_default(),
            ));
        }
        Ok(Job { data, output: None })
    }

    pub fn from_file(filename: &str) -> Result<Job, ScriptHerderError> {
        let mut f = std::fs::File::open(filename).map_err(|e| {
            ScriptHerderError::job_load(format!("Error ({e}) loading job output"), filename)
        })?;
        let mut buf = String::new();
        f.read_to_string(&mut buf).map_err(|e| {
            ScriptHerderError::job_load(format!("Error ({e}) loading job output"), filename)
        })?;
        let mut data: JobData = serde_json::from_str(&buf)
            .map_err(|_| ScriptHerderError::job_load("JSON parsing failed", filename))?;
        data.filename = Some(filename.to_string());
        Job::from_data(data)
    }

    pub fn name(&self) -> String {
        match &self.data.name {
            Some(n) => n.clone(),
            None => self.cmd().to_string(),
        }
    }

    pub fn cmd(&self) -> &str {
        self.data.cmd.first().map(|s| s.as_str()).unwrap_or("")
    }

    pub fn args(&self) -> Vec<String> {
        self.data.cmd.iter().skip(1).cloned().collect()
    }

    pub fn start_time(&self) -> Option<f64> {
        self.data.start_time
    }

    pub fn end_time(&self) -> Option<f64> {
        self.data.end_time
    }

    pub fn exit_status(&self) -> Option<i32> {
        self.data.exit_status
    }

    pub fn pid(&self) -> Option<i32> {
        self.data.pid
    }

    pub fn filename(&self) -> Option<&str> {
        self.data.filename.as_deref()
    }

    pub fn output_filename(&self) -> Option<&str> {
        self.data.output_filename.as_deref()
    }

    pub fn check_status(&self) -> Option<&str> {
        self.data.check_status.as_deref()
    }

    pub fn check_reason(&self) -> Option<&str> {
        self.data.check_reason.as_deref()
    }

    /// A job is considered "run" (has results) when both start and end times are set.
    pub fn is_running(&self) -> bool {
        self.data.start_time.is_some() && self.data.end_time.is_some()
    }

    pub fn is_ok(&self) -> bool {
        self.check_status() == Some("OK")
    }

    pub fn is_warning(&self) -> bool {
        self.check_status() == Some("WARNING")
    }

    pub fn is_critical(&self) -> bool {
        self.check_status() == Some("CRITICAL")
    }

    pub fn age(&self) -> String {
        match self.data.start_time {
            None => "N/A".to_string(),
            Some(st) => time_to_str(now_secs() - st),
        }
    }

    pub fn duration_str(&self) -> String {
        match (self.data.end_time, self.data.start_time) {
            (Some(e), Some(s)) => time_to_str(e - s),
            _ => "NaN".to_string(),
        }
    }

    pub fn status_summary(&self) -> String {
        if !self.is_running() {
            return format!("{}[not_running]", self.name());
        }
        let age = time_to_str(now_secs() - self.data.start_time.unwrap());
        format!(
            "{}[exit={},age={}]",
            self.name(),
            self.exit_status()
                .map(|e| e.to_string())
                .unwrap_or_else(|| "None".to_string()),
            age
        )
    }

    /// Python `__str__`
    pub fn display(&self) -> String {
        self.status_summary()
    }

    pub fn set_check_status(&mut self, value: &str) -> Result<(), ScriptHerderError> {
        if !EXIT_STATUS_NAMES.contains(&value) {
            return Err(ScriptHerderError::job_load(
                format!("Unknown check_status {value:?}"),
                "",
            ));
        }
        self.data.check_status = Some(value.to_string());
        Ok(())
    }

    pub fn set_check_reason(&mut self, value: String) {
        self.data.check_reason = Some(value);
    }

    /// Determine OK/WARNING/CRITICAL status from a Check, storing status + reason.
    /// Mirrors ../src/scriptherder.py:401-420.
    pub fn check(&mut self, check: &crate::check::Check) {
        let (status, msg) = check.job_is_ok(self);
        if status {
            self.set_check_status("OK").unwrap();
            self.set_check_reason(msg.join(", "));
        } else {
            let (wstatus, warn_msg) = check.job_is_warning(self);
            let mut merged = msg.clone();
            for m in warn_msg {
                if !merged.contains(&m) {
                    merged.push(m);
                }
            }
            self.set_check_status(if wstatus { "WARNING" } else { "CRITICAL" })
                .unwrap();
            self.set_check_reason(merged.join(", "));
        }
    }

    /// Lazy-loads output: first from in-memory buffer, then from output_filename,
    /// then from the inline `data.output` string field.
    pub fn output(&mut self) -> Option<Vec<u8>> {
        if let Some(o) = &self.output {
            return Some(o.clone());
        }
        if self.data.output.is_none() {
            if let Some(fname) = self.data.output_filename.clone() {
                if let Ok(bytes) = std::fs::read(&fname) {
                    return Some(bytes);
                }
            }
        }
        self.data.output.as_ref().map(|s| s.as_bytes().to_vec())
    }

    /// Run the command, capturing stdout and stderr.
    /// Python uses `stderr=subprocess.STDOUT` which interleaves in real time;
    /// Rust's `Output` captures them separately — we concatenate stdout+stderr
    /// as the closest faithful approximation.
    pub fn run(&mut self) {
        use std::process::{Command, Stdio};
        self.data.start_time = Some(now_secs());
        // Spawn (not output()) so the child pid can be recorded, mirroring Python `proc.pid`.
        let child = Command::new(&self.data.cmd[0])
            .args(&self.data.cmd[1..])
            .current_dir("/")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()) // merged below
            .spawn();
        match child {
            Ok(c) => {
                self.data.pid = Some(c.id() as i32);
                let out = c.wait_with_output();
                self.data.end_time = Some(now_secs());
                match out {
                    Ok(out) => {
                        // Python merges stderr into stdout (STDOUT). Concatenate.
                        let mut merged = out.stdout;
                        merged.extend_from_slice(&out.stderr);
                        self.data.exit_status = Some(out.status.code().unwrap_or(-1));
                        self.output = Some(merged);
                    }
                    Err(_) => {
                        self.data.exit_status = Some(-1);
                        self.output = Some(Vec::new());
                    }
                }
            }
            Err(_) => {
                self.data.end_time = Some(now_secs());
                self.data.exit_status = Some(-1);
                self.output = Some(Vec::new());
            }
        }
    }

    /// Set start/end times directly; intended for test support only.
    pub fn set_times_for_test(&mut self, start: f64, end: f64) {
        self.data.start_time = Some(start);
        self.data.end_time = Some(end);
    }

    pub fn save_to_file(
        &mut self,
        datadir: &str,
        filename: Option<&str>,
        umask_octal: &str,
    ) -> std::io::Result<()> {
        use chrono::{Local, TimeZone};
        use std::io::Write;

        let fname = match filename {
            Some(f) => f.to_string(),
            None => {
                // sanitize name: non-alphanumeric -> '_'
                let sanitized: String = self
                    .name()
                    .chars()
                    .map(|c| if c.is_alphanumeric() { c } else { '_' })
                    .collect();
                let st = self.data.start_time.expect("start_time set before save");
                let secs = st.trunc() as i64;
                let micros = ((st.fract()) * 1_000_000.0).round() as u32;
                let dt = Local.timestamp_opt(secs, 0).single().expect("valid ts");
                // Match Python `{:03}`.format(_ts.microsecond): full microseconds, min 3-digit pad.
                let time_str = format!("{}.{:03}", dt.format("%Y%m%dT%H%M%S"), micros);
                format!(
                    "{sanitized}__ts-{time_str}_pid-{}",
                    self.data.pid.map(|p| p.to_string()).unwrap_or_default()
                )
            }
        };
        let full = std::path::Path::new(datadir).join(&fname);
        let full = full.to_str().unwrap().to_string();

        // umask from 3-digit octal string, e.g. "077"
        let umask_val = u32::from_str_radix(umask_octal, 8).unwrap_or(0o077);
        let old_umask = unsafe { libc::umask(umask_val as libc::mode_t) };

        let output_fn = format!("{full}_output");
        if self.output.is_some() {
            self.data.output_filename = Some(format!("{output_fn}.data"));
            self.data.output_size = self.output.as_ref().map(|o| o.len());
        }

        // write metadata json (indent 4, sorted keys) atomically
        let value = serde_json::to_value(&self.data).unwrap();
        let sorted = sort_json_keys(value);
        let mut buf = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
        use serde::Serialize as _;
        sorted.serialize(&mut ser).unwrap();
        let json = String::from_utf8(buf).unwrap();
        {
            let mut f = std::fs::File::create(format!("{full}.tmp"))?;
            f.write_all(json.as_bytes())?;
            f.write_all(b"\n")?;
        }
        std::fs::rename(format!("{full}.tmp"), format!("{full}.json"))?;
        self.data.filename = Some(full.clone());

        if let Some(out) = self.output.take() {
            let out_target = self.data.output_filename.clone().unwrap();
            {
                let mut fd = std::fs::File::create(format!("{out_target}.tmp"))?;
                fd.write_all(&out)?;
            }
            std::fs::rename(format!("{out_target}.tmp"), &out_target)?;
        }

        unsafe {
            libc::umask(old_umask);
        }
        Ok(())
    }
}

/// Recursively reorder JSON object keys alphabetically (Python sort_keys=True).
fn sort_json_keys(value: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            let mut sorted = serde_json::Map::new();
            let mut keys: Vec<_> = map.keys().cloned().collect();
            keys.sort();
            for k in keys {
                sorted.insert(k.clone(), sort_json_keys(map[&k].clone()));
            }
            Value::Object(sorted)
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(sort_json_keys).collect()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_reload_roundtrip() {
        let dir = std::env::temp_dir().join(format!("sh_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut job = Job::new("rt job", vec!["/bin/echo".into(), "hi".into()]).unwrap();
        job.run();
        job.save_to_file(dir.to_str().unwrap(), None, "077")
            .unwrap();
        let saved = job.filename().unwrap().to_string();
        let reloaded = Job::from_file(&format!("{saved}.json")).unwrap();
        assert_eq!(reloaded.name(), "rt job");
        assert_eq!(reloaded.exit_status(), Some(0));
        // output stored in a sibling .data file
        let out_fn = reloaded.output_filename().unwrap();
        assert!(std::path::Path::new(out_fn).exists());

        // formatting fidelity: 4-space indent and sorted keys (Python indent=4, sort_keys=True)
        let text = std::fs::read_to_string(format!("{saved}.json")).unwrap();
        assert!(
            text.lines()
                .any(|l| l.starts_with("    \"") && !l.starts_with("     ")),
            "expected a line with exactly 4-space indent before a key"
        );
        let pos_cmd = text.find("\"cmd\"").unwrap();
        let pos_name = text.find("\"name\"").unwrap();
        let pos_version = text.find("\"version\"").unwrap();
        assert!(
            pos_cmd < pos_name && pos_name < pos_version,
            "keys must be sorted: cmd < name < version"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn run_echo_captures_output_and_times() {
        let mut job = Job::new("echo_test", vec!["/bin/echo".into(), "test".into()]).unwrap();
        job.run();
        assert!(job.is_running());
        assert_eq!(job.output().unwrap(), b"test\n");
        assert!(job.start_time().unwrap() <= job.end_time().unwrap());
        assert!(job.pid().is_some());
    }

    #[test]
    fn name_defaults_to_basename() {
        let job = Job::new("", vec!["/bin/echo".into()]).unwrap();
        assert_eq!(job.name(), "echo");
    }

    #[test]
    fn rejects_unknown_version() {
        let data: JobData = serde_json::from_str(r#"{"version":9}"#).unwrap();
        assert!(Job::from_data(data).is_err());
    }
}
