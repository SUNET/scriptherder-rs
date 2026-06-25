use crate::error::ScriptHerderError;
use crate::job::Job;
use crate::util::{parse_time_value, time_to_str};
use std::collections::BTreeMap;

/// Python boolean repr used verbatim in evaluation messages.
fn py_bool(b: bool) -> &'static str {
    if b {
        "True"
    } else {
        "False"
    }
}

pub const CHECK_DEFAULT_OK: &str = "exit_status=0,max_age=8h";
pub const CHECK_DEFAULT_WARNING: &str = "exit_status=0,max_age=24h";

/// Apply ConfigParser BasicInterpolation to a value.
/// `%%`->`%`; `%(key)s`->value of key (case-folded), recursive; bare `%` is an error.
fn interpolate(
    raw: &str,
    section: &BTreeMap<String, String>,
    defaults: &BTreeMap<String, String>,
    depth: u8,
    filename: &str,
) -> Result<String, ScriptHerderError> {
    if depth > 10 {
        return Err(ScriptHerderError::check_load(
            "Interpolation too deep",
            filename,
        ));
    }
    let bytes: Vec<char> = raw.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != '%' {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        // at '%'
        if i + 1 >= bytes.len() {
            return Err(ScriptHerderError::check_load(
                "Bad interpolation: trailing %",
                filename,
            ));
        }
        match bytes[i + 1] {
            '%' => {
                out.push('%');
                i += 2;
            }
            '(' => {
                // read until ')s'
                let mut j = i + 2;
                let mut key = String::new();
                while j < bytes.len() && bytes[j] != ')' {
                    key.push(bytes[j]);
                    j += 1;
                }
                if j + 1 >= bytes.len() || bytes[j] != ')' || bytes[j + 1] != 's' {
                    return Err(ScriptHerderError::check_load(
                        "Bad interpolation syntax",
                        filename,
                    ));
                }
                let key_lc = key.to_lowercase();
                let val = section
                    .get(&key_lc)
                    .or_else(|| defaults.get(&key_lc))
                    .ok_or_else(|| {
                        ScriptHerderError::check_load(
                            format!("Bad interpolation key: {key}"),
                            filename,
                        )
                    })?;
                let resolved = interpolate(val, section, defaults, depth + 1, filename)?;
                out.push_str(&resolved);
                i = j + 2;
            }
            _ => {
                return Err(ScriptHerderError::check_load(
                    "Bad interpolation: bare %",
                    filename,
                ))
            }
        }
    }
    Ok(out)
}

/// A single parsed criterion token from an ok/warning string.
#[derive(Debug, Clone)]
pub struct Criterion {
    pub what: String,
    pub value: Option<String>,
    pub negate: bool,
}

/// Parse a comma-separated criteria string into `Criterion` items.
/// Applies backwards-compat renames, leading-`!` negation, runtime filtering,
/// and in non-runtime mode appends a `stored_status=OK` guard (see `Check::new`).
fn parse_criteria(
    data_str: &str,
    runtime_mode: bool,
    filename: &str,
) -> Result<Vec<Criterion>, ScriptHerderError> {
    let mut res = Vec::new();
    for raw in data_str.split(',') {
        let mut this = raw.trim().to_string();
        if this.is_empty() {
            continue;
        }
        // backwards-compat renames (prefix-aware)
        for (old, new) in [
            ("not_running", "!OR_running"),
            ("output_not_contains", "!output_contains"),
        ] {
            if this == old || this.starts_with(&format!("{old}=")) {
                this = format!("{new}{}", &this[old.len()..]);
            }
        }
        let mut negate = false;
        if let Some(stripped) = this.strip_prefix('!') {
            negate = true;
            this = stripped.to_string();
        }
        if !this.contains('=') {
            if this != "OR_running" {
                return Err(ScriptHerderError::check_load(
                    format!("Bad criteria: {this:?}"),
                    filename,
                ));
            }
            res.push(Criterion {
                what: this,
                value: None,
                negate,
            });
            continue;
        }
        let (what, value) = this.split_once('=').unwrap();
        let what = what.trim().to_string();
        let value = value.trim().to_string();
        let is_runtime_check = what != "max_age" && what != "OR_file_exists";
        if runtime_mode != is_runtime_check {
            continue;
        }
        res.push(Criterion {
            what,
            value: Some(value),
            negate,
        });
    }
    Ok(res)
}

/// Parsed check with ok and warning criteria.
#[derive(Clone)]
pub struct Check {
    pub filename: String,
    pub ok_criteria: Vec<Criterion>,
    pub warning_criteria: Vec<Criterion>,
}

impl Check {
    /// Build a `Check` from raw ok/warning strings.
    /// In non-runtime mode, appends `stored_status=OK` to both criteria lists.
    pub fn new(
        ok_str: &str,
        warning_str: &str,
        filename: &str,
        runtime_mode: bool,
    ) -> Result<Check, ScriptHerderError> {
        let mut ok_criteria = parse_criteria(ok_str, runtime_mode, filename)?;
        let mut warning_criteria = parse_criteria(warning_str, runtime_mode, filename)?;
        if !runtime_mode {
            let stored = Criterion {
                what: "stored_status".into(),
                value: Some("OK".into()),
                negate: false,
            };
            ok_criteria.push(stored.clone());
            warning_criteria.push(stored);
        }
        Ok(Check {
            filename: filename.to_string(),
            ok_criteria,
            warning_criteria,
        })
    }

    /// Load a `Check` from a `.check` INI file using `load_check_strings`.
    pub fn from_file(filename: &str, runtime_mode: bool) -> Result<Check, ScriptHerderError> {
        let (ok, warning) = load_check_strings(filename)?;
        Check::new(&ok, &warning, filename, runtime_mode)
    }

    /// Evaluate a job against the OK criteria.
    pub fn job_is_ok(&self, job: &mut Job) -> (bool, Vec<String>) {
        self.evaluate(&self.ok_criteria.clone(), job)
    }

    /// Evaluate a job against the WARNING criteria.
    pub fn job_is_warning(&self, job: &mut Job) -> (bool, Vec<String>) {
        self.evaluate(&self.warning_criteria.clone(), job)
    }

    fn evaluate(&self, criteria: &[Criterion], job: &mut Job) -> (bool, Vec<String>) {
        let mut ok_msgs: Vec<String> = Vec::new();
        let mut fail_msgs: Vec<String> = Vec::new();
        let (or_c, and_c): (Vec<&Criterion>, Vec<&Criterion>) =
            criteria.iter().partition(|c| c.what.starts_with("OR_"));

        for c in &or_c {
            let (status, msg) = self.call_check(c, job);
            if status {
                return (true, vec![msg]);
            }
            fail_msgs.push(msg);
        }
        if and_c.is_empty() {
            return (false, fail_msgs);
        }

        let mut res = true;
        for c in &and_c {
            let (status, msg) = self.call_check(c, job);
            if !status {
                res = false;
                fail_msgs.push(msg);
            } else {
                ok_msgs.push(msg);
            }
        }
        if res {
            (true, ok_msgs)
        } else {
            (false, fail_msgs)
        }
    }

    fn call_check(&self, c: &Criterion, job: &mut Job) -> (bool, String) {
        let (status, mut msg) = match c.what.as_str() {
            "exit_status" => check_exit_status(job, c),
            "max_age" => check_max_age(job, c),
            "output_contains" => check_output_contains(job, c),
            "output_matches" => check_output_matches(job, c),
            "OR_running" => check_or_running(job, c),
            "OR_file_exists" => check_or_file_exists(job, c),
            "stored_status" => check_stored_status(job, c),
            other => return (false, format!("{other}=unknown_criteria")),
        };
        if msg.is_empty() {
            let neg = if c.negate { "!" } else { "" };
            msg = format!("{neg}{}={}", c.what, c.value.clone().unwrap_or_default());
        }
        (status, msg)
    }
}

fn check_exit_status(job: &mut Job, c: &Criterion) -> (bool, String) {
    let value = c.value.clone().unwrap_or_default();
    let mut res = job.exit_status() == value.parse::<i32>().ok();
    if c.negate {
        res = !res;
    }
    if res {
        return (true, format!("exit={value}"));
    }
    let actual = job
        .exit_status()
        .map(|e| e.to_string())
        .unwrap_or_else(|| "None".into());
    if c.negate {
        (false, format!("exit={actual}=={value}"))
    } else {
        (false, format!("exit={actual}!={value}"))
    }
}

fn check_max_age(job: &mut Job, c: &Criterion) -> (bool, String) {
    let value = c.value.clone().unwrap_or_default();
    let secs = parse_time_value(&value).expect("max_age value parses");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let mut res = match job.end_time() {
        None => false,
        Some(e) => e > (now - secs) as f64,
    };
    if c.negate {
        res = !res;
    }
    if res {
        return (true, String::new());
    }
    if c.negate {
        (
            false,
            format!("age={}<={}", job.age(), time_to_str(secs as f64)),
        )
    } else {
        (
            false,
            format!("age={}>{}", job.age(), time_to_str(secs as f64)),
        )
    }
}

fn check_output_contains(job: &mut Job, c: &Criterion) -> (bool, String) {
    let value = c.value.clone().unwrap_or_default();
    let out = job.output().unwrap_or_default();
    let mut res = out
        .windows(value.len().max(1))
        .any(|w| w == value.as_bytes())
        || value.is_empty();
    if c.negate {
        res = !res;
    }
    let neg = if c.negate { "!" } else { "" };
    (
        res,
        format!("{neg}output_contains={value}=={}", py_bool(res)),
    )
}

fn check_output_matches(job: &mut Job, c: &Criterion) -> (bool, String) {
    use regex::bytes::Regex;
    let value = c.value.clone().unwrap_or_default();
    let out = job.output().unwrap_or_default();
    // Python re.match = anchored at start
    let re = Regex::new(&format!("^(?:{value})")).unwrap();
    let mut res = re.is_match(&out);
    if c.negate {
        res = !res;
    }
    let neg = if c.negate { "!" } else { "" };
    (
        res,
        format!("{neg}output_matches={value}=={}", py_bool(res)),
    )
}

fn check_or_running(job: &mut Job, c: &Criterion) -> (bool, String) {
    let mut res = job.is_running();
    let msg = if res { "is_running" } else { "not_running" }.to_string();
    if c.negate {
        res = !res;
    }
    (res, msg)
}

fn check_or_file_exists(_job: &mut Job, c: &Criterion) -> (bool, String) {
    let value = c.value.clone().unwrap_or_default();
    let mut res = std::path::Path::new(&value).is_file();
    let msg = if res {
        format!("file_exists={value}")
    } else {
        format!("file_does_not_exist={value}")
    };
    if c.negate {
        res = !res;
    }
    (res, msg)
}

fn check_stored_status(job: &mut Job, c: &Criterion) -> (bool, String) {
    let value = c.value.clone().unwrap_or_default();
    let mut res = job.check_status() == Some(value.as_str());
    if c.negate {
        res = !res;
    }
    let neg = if c.negate { "!" } else { "" };
    (res, format!("{neg}stored_status={value}=={}", py_bool(res)))
}

/// Read `[check]` ok/warning from an INI file (rust-ini handles sections,
/// comments, `=`/`:`, continuations). Apply defaults, merge [DEFAULT], interpolate.
pub fn load_check_strings(filename: &str) -> Result<(String, String), ScriptHerderError> {
    use ini::Ini;
    let conf = Ini::load_from_file(filename)
        .map_err(|_| ScriptHerderError::check_load("Failed reading file", filename))?;

    // Build case-folded defaults map: programmatic defaults first, then [DEFAULT] section overrides.
    // rust-ini does NOT treat [DEFAULT] specially (unlike Python ConfigParser);
    // we look it up as a regular named section.
    let mut defaults: BTreeMap<String, String> = BTreeMap::new();
    defaults.insert("ok".into(), CHECK_DEFAULT_OK.into());
    defaults.insert("warning".into(), CHECK_DEFAULT_WARNING.into());
    if let Some(def) = conf.section(Some("DEFAULT")) {
        for (k, v) in def.iter() {
            defaults.insert(k.to_lowercase(), v.to_string());
        }
    }

    // Build section map: start from defaults, then overlay [check] keys.
    let mut section: BTreeMap<String, String> = defaults.clone();
    let check = conf
        .section(Some("check"))
        .ok_or_else(|| ScriptHerderError::check_load("Failed loading file", filename))?;
    for (k, v) in check.iter() {
        section.insert(k.to_lowercase(), v.to_string());
    }

    let ok_raw = section
        .get("ok")
        .cloned()
        .ok_or_else(|| ScriptHerderError::check_load("Failed loading file", filename))?;
    let warn_raw = section
        .get("warning")
        .cloned()
        .ok_or_else(|| ScriptHerderError::check_load("Failed loading file", filename))?;

    let ok = interpolate(&ok_raw, &section, &defaults, 0, filename)?;
    let warning = interpolate(&warn_raw, &section, &defaults, 0, filename)?;
    Ok((ok, warning))
}

#[cfg(test)]
mod parse_tests {
    use super::*;

    #[test]
    fn rename_not_running() {
        let c = parse_criteria("not_running", true, "f").unwrap();
        assert_eq!(c[0].what, "OR_running");
        assert!(c[0].negate);
    }

    #[test]
    fn runtime_skips_max_age() {
        let c = parse_criteria("exit_status=0,max_age=8h", true, "f").unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].what, "exit_status");
    }

    #[test]
    fn nonruntime_keeps_max_age_only() {
        let c = parse_criteria("exit_status=0,max_age=8h", false, "f").unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].what, "max_age");
    }

    #[test]
    fn bad_single_token_errors() {
        assert!(parse_criteria("frobnicate", true, "f").is_err());
    }
}

#[cfg(test)]
mod ini_tests {
    use super::*;

    fn sec(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn percent_escape() {
        let s = sec(&[]);
        assert_eq!(interpolate("100%%", &s, &s, 0, "f").unwrap(), "100%");
    }

    #[test]
    fn key_reference() {
        let s = sec(&[("base", "exit_status=0")]);
        assert_eq!(
            interpolate("%(base)s,max_age=8h", &s, &s, 0, "f").unwrap(),
            "exit_status=0,max_age=8h"
        );
    }

    #[test]
    fn bare_percent_errors() {
        let s = sec(&[]);
        assert!(interpolate("100%done", &s, &s, 0, "f").is_err());
    }

    #[test]
    fn unknown_key_errors() {
        let s = sec(&[]);
        assert!(interpolate("%(nope)s", &s, &s, 0, "f").is_err());
    }

    #[test]
    fn load_strings_with_defaults() {
        let p = std::env::temp_dir().join(format!("chk_{}.ini", std::process::id()));
        std::fs::write(&p, "[check]\nok = exit_status=0, max_age=8h\n").unwrap();
        let (ok, warn) = load_check_strings(p.to_str().unwrap()).unwrap();
        assert_eq!(ok, "exit_status=0, max_age=8h");
        assert_eq!(warn, CHECK_DEFAULT_WARNING); // default applied
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn default_section_key_used_as_interpolation_target() {
        let p = std::env::temp_dir().join(format!("chk_def_{}.ini", std::process::id()));
        std::fs::write(
            &p,
            "[DEFAULT]\nbase = exit_status=0\n[check]\nok = %(base)s, max_age=8h\n",
        )
        .unwrap();
        let (ok, _warn) = load_check_strings(p.to_str().unwrap()).unwrap();
        assert_eq!(ok, "exit_status=0, max_age=8h");
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn default_section_supplies_warning() {
        let p = std::env::temp_dir().join(format!("chk_defw_{}.ini", std::process::id()));
        std::fs::write(
            &p,
            "[DEFAULT]\nwarning = exit_status=0, max_age=99h\n[check]\nok = exit_status=0\n",
        )
        .unwrap();
        let (_ok, warn) = load_check_strings(p.to_str().unwrap()).unwrap();
        assert_eq!(warn, "exit_status=0, max_age=99h"); // [DEFAULT] warning overrides programmatic default
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn self_referential_interpolation_errors() {
        let p = std::env::temp_dir().join(format!("chk_rec_{}.ini", std::process::id()));
        std::fs::write(&p, "[check]\nok = %(ok)s\nwarning = exit_status=0\n").unwrap();
        let result = load_check_strings(p.to_str().unwrap());
        assert!(result.is_err()); // depth guard yields CheckLoadError, no panic/overflow
        std::fs::remove_file(&p).ok();
    }
}
