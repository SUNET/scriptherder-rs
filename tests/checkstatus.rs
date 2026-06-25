use scriptherder::check::Check;
use scriptherder::checkstatus::CheckStatus;
use scriptherder::job::Job;
use scriptherder::jobs_list::JobsList;
use std::collections::HashMap;

fn move_back(job: &mut Job, seconds: f64) {
    let s = job.start_time().unwrap();
    let e = job.end_time().unwrap();
    job.set_times_for_test(s - seconds, e - seconds);
}

fn make_checkstatus(jobs: Vec<Job>, ok: &str, warn: &str) -> CheckStatus {
    let mut checks = HashMap::new();
    checks.insert(
        "test1".to_string(),
        Check::new(ok, warn, "unit_testing", false).unwrap(),
    );
    let mut cs = CheckStatus::new(false, String::new(), checks);
    cs.check_jobs(JobsList::from_jobs(jobs, false));
    cs
}

#[test]
fn two_delayed_jobs_critical() {
    let ok = "exit_status=0, max_age=1m";
    let warn = "exit_status=0, max_age=5m";
    let mut j1 = Job::new("test1", vec!["/usr/bin/true".into()]).unwrap();
    let mut j2 = Job::new("test1", vec!["/usr/bin/true".into()]).unwrap();
    j1.run();
    j2.run();
    let rc = Check::new(ok, warn, "unit_testing", true).unwrap();
    j1.check(&rc);
    j2.check(&rc);
    move_back(&mut j1, 19.0 * 60.0);
    move_back(&mut j2, 20.0 * 60.0);
    let cs = make_checkstatus(vec![j1, j2], ok, warn);
    assert_eq!(cs.num_jobs(), 1);
    assert_eq!(
        cs.aggregate_status(),
        (
            "CRITICAL".to_string(),
            Some("age=19m>1m, age=19m>5m".to_string())
        )
    );
}

#[test]
fn two_delayed_jobs_warning() {
    let ok = "exit_status=0, max_age=1m";
    let warn = "exit_status=0, max_age=2h";
    let mut j1 = Job::new("test1", vec!["/usr/bin/true".into()]).unwrap();
    let mut j2 = Job::new("test1", vec!["/usr/bin/true".into()]).unwrap();
    j1.run();
    j2.run();
    let rc = Check::new(ok, warn, "unit_testing", true).unwrap();
    j1.check(&rc);
    j2.check(&rc);
    move_back(&mut j1, 2.0 * 60.0);
    move_back(&mut j2, 4.0 * 60.0);
    let cs = make_checkstatus(vec![j1, j2], ok, warn);
    assert_eq!(cs.num_jobs(), 1);
    assert_eq!(
        cs.aggregate_status(),
        (
            "WARNING".to_string(),
            Some("age=2m>1m, max_age=2h, stored_status=OK==True".to_string())
        )
    );
}

#[test]
fn positive_then_negative_critical() {
    let ok = "exit_status=0";
    let warn = "exit_status=0";
    let mut j1 = Job::new("test1", vec!["/usr/bin/true".into()]).unwrap();
    let mut j2 = Job::new("test1", vec!["/usr/bin/false".into()]).unwrap();
    j1.run();
    j2.run();
    let rc = Check::new(ok, warn, "unit_testing", true).unwrap();
    j1.check(&rc);
    j2.check(&rc);
    let cs = make_checkstatus(vec![j1, j2], ok, warn);
    assert_eq!(cs.num_jobs(), 1);
    assert_eq!(
        cs.aggregate_status(),
        (
            "CRITICAL".to_string(),
            Some("stored_status=OK==False".to_string())
        )
    );
}

#[test]
fn job_failed_job() {
    let ok = "exit_status=0,max_age=50m";
    let warn = "exit_status=0,max_age=1h";
    let mut j1 = Job::new("test1", vec!["/usr/bin/false".into()]).unwrap();
    j1.run();
    move_back(&mut j1, 10.0);
    let rc = Check::new(ok, warn, "unit_testing", true).unwrap();
    j1.check(&rc);
    let cs = make_checkstatus(vec![j1], ok, warn);
    assert_eq!(cs.num_jobs(), 1);
    assert_eq!(
        cs.aggregate_status(),
        (
            "CRITICAL".to_string(),
            Some("stored_status=OK==False".to_string())
        )
    );
}

#[test]
fn misconfigured_criteria() {
    let ok = "exit_status=0,max_age=50m";
    let warn = "exit_status=0,max_age=1";
    let mut j1 = Job::new("test1", vec!["/usr/bin/false".into()]).unwrap();
    j1.run();
    move_back(&mut j1, 10.0);
    let rc = Check::new(ok, warn, "unit_testing", true).unwrap();
    j1.check(&rc);
    let cs = make_checkstatus(vec![j1], ok, warn);
    assert_eq!(cs.num_jobs(), 1);
    assert_eq!(
        cs.aggregate_status(),
        (
            "CRITICAL".to_string(),
            Some("stored_status=OK==False, age=10s>1s".to_string())
        )
    );
}
