use crate::job::Job;
use regex::Regex;

/// Parse time strings like "8h"/"30m"/"10". Bare number = seconds.
/// Returns None if it does not match `^(\d+)([hmsd]*)$`.
pub fn parse_time_value(value: &str) -> Option<i64> {
    let re = Regex::new(r"^(\d+)([hmsd]*)$").unwrap();
    let caps = re.captures(value)?;
    let num: i64 = caps.get(1).unwrap().as_str().parse().ok()?;
    match caps.get(2).map(|m| m.as_str()).unwrap_or("") {
        "m" => Some(num * 60),
        "h" => Some(num * 3600),
        "d" => Some(num * 86400),
        _ => Some(num),
    }
}

/// Format a number of seconds as a short human string (matches Python _time_to_str).
pub fn time_to_str(value: f64) -> String {
    if value < 1.0 {
        return format!("{}ms", (value * 1000.0) as i64);
    }
    if value < 60.0 {
        return format!("{}s", value as i64);
    }
    if value < 3600.0 {
        return format!("{}m", (value / 60.0) as i64);
    }
    if value < 86400.0 {
        return format!("{}h", (value / 3600.0) as i64);
    }
    let days = (value / 86400.0) as i64;
    format!("{}d{}h", days, ((value as i64 % 86400) / 3600))
}

/// Format the multi-job summary line (../src/scriptherder.py:1283-1295).
pub fn status_summary(num_jobs: usize, failed: &[Job]) -> String {
    let plural = if num_jobs != 1 { "s" } else { "" };
    let mut summaries: Vec<String> = failed.iter().map(|j| j.status_summary()).collect();
    summaries.sort();
    format!(
        "{}/{} job{} in this state: {}",
        failed.len(),
        num_jobs,
        plural,
        summaries.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_time_suffixes() {
        assert_eq!(parse_time_value("10"), Some(10));
        assert_eq!(parse_time_value("1m"), Some(60));
        assert_eq!(parse_time_value("8h"), Some(28800));
        assert_eq!(parse_time_value("2d"), Some(172800));
        assert_eq!(parse_time_value("bad"), None);
    }

    #[test]
    fn format_buckets() {
        assert_eq!(time_to_str(0.5), "500ms");
        assert_eq!(time_to_str(10.0), "10s");
        assert_eq!(time_to_str(19.0 * 60.0), "19m");
        assert_eq!(time_to_str(2.0 * 3600.0), "2h");
        assert_eq!(time_to_str(90000.0), "1d1h");
    }
}
