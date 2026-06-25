use scriptherder::check::Check;
use scriptherder::job::Job;

// Helper mirroring test_checks.py::_run
fn run_check(cmd: &[&str], ok: &str, warn: &str, run: bool, runtime_mode_off: bool) -> Job {
    let mut job = Job::new("unittest_job", cmd.iter().map(|s| s.to_string()).collect()).unwrap();
    if run {
        job.run();
    }
    let check = Check::new(ok, warn, "unit_testing", true).unwrap();
    job.check(&check);
    if runtime_mode_off {
        let check = Check::new(ok, warn, "unit_testing", false).unwrap();
        job.check(&check);
    }
    job
}

#[test]
fn exit_status_ok() {
    let job = run_check(&["/bin/echo", "test"], "exit_status=0", "", true, false);
    assert!(job.is_ok());
}

#[test]
fn exit_status_critical() {
    let job = run_check(
        &["/usr/bin/true", "test"],
        "exit_status=1",
        "exit_status=2",
        true,
        false,
    );
    assert!(!job.is_ok());
    assert!(!job.is_warning());
    assert_eq!(job.check_status(), Some("CRITICAL"));
}

#[test]
fn file_exists_fail_reason() {
    let job = run_check(
        &["/usr/bin/false"],
        "exit_status=0,OR_file_exists=/this_file_should_not_exist",
        "",
        true,
        true,
    );
    assert!(!job.is_ok());
    assert_eq!(job.check_status(), Some("CRITICAL"));
    assert_eq!(
        job.check_reason(),
        Some("file_does_not_exist=/this_file_should_not_exist, stored_status=OK==False")
    );
}

#[test]
fn output_contains_reason() {
    let job = run_check(
        &["/bin/echo", "STATUS_TESTING_OK"],
        "exit_status=0,output_contains=TESTING",
        "",
        true,
        false,
    );
    assert!(job.is_ok());
    assert_eq!(
        job.check_reason(),
        Some("exit=0, output_contains=TESTING==True")
    );
}

#[test]
fn output_not_contains_obsolete_reason() {
    let job = run_check(
        &["/bin/echo", "STATUS_TESTING_OK"],
        "exit_status=0,output_not_contains=ERROR",
        "",
        true,
        false,
    );
    assert!(job.is_ok());
    assert_eq!(
        job.check_reason(),
        Some("exit=0, !output_contains=ERROR==True")
    );
}

#[test]
fn output_matches_reason() {
    let job = run_check(
        &["/bin/echo", "STATUS_TESTING_OK"],
        "exit_status=0,output_matches=.*TESTING.*",
        "",
        true,
        false,
    );
    assert!(job.is_ok());
    assert_eq!(
        job.check_reason(),
        Some("exit=0, output_matches=.*TESTING.*==True")
    );
}

#[test]
fn exit_status_warning() {
    let job = run_check(
        &["/bin/echo", "test"],
        "exit_status=1",
        "exit_status=0",
        true,
        false,
    );
    assert!(!job.is_ok());
    assert!(job.is_warning());
}

#[test]
fn exit_status_negated1() {
    let job = run_check(&["/usr/bin/false"], "!exit_status=0", "", true, false);
    assert!(job.is_ok());
    assert!(!job.is_warning());
}

#[test]
fn max_age() {
    let job = run_check(
        &["/bin/echo", "test"],
        "exit_status=0, max_age=10s",
        "exit_status=0, max_age=3h",
        true,
        true,
    );
    assert!(job.is_ok());
    assert!(!job.is_warning());
}

#[test]
fn max_age_negated() {
    let job = run_check(
        &["/bin/echo", "test"],
        "exit_status=0, !max_age=10s",
        "exit_status=0, max_age=3h",
        true,
        true,
    );
    assert!(!job.is_ok());
    assert!(job.is_warning());
}

#[test]
fn file_exists() {
    // exit_status=1 so ok fails; warn has OR_file_exists=/etc/services which exists
    let job = run_check(
        &["/bin/echo", "test"],
        "exit_status=1",
        "exit_status=1,OR_file_exists=/etc/services",
        true,
        true,
    );
    assert!(!job.is_ok());
    assert!(job.is_warning());
}

#[test]
fn file_exists_negated() {
    // /usr/bin/false exits 1; ok = exit_status=0,!OR_file_exists=<nonexistent>
    // negated file_exists on a missing file → criterion passes
    let job = run_check(
        &["/usr/bin/false"],
        "exit_status=0,!OR_file_exists=/this_could_be_a_FAIL_file",
        "",
        true,
        true,
    );
    assert!(job.is_ok());
}

#[test]
fn or_running() {
    // /bin/echo exits 0; ok = exit_status=1,OR_running → OR_running is True while running
    let job = run_check(
        &["/bin/echo", "test"],
        "exit_status=1,OR_running",
        "exit_status=0",
        true,
        false,
    );
    assert!(job.is_ok());
    assert!(!job.is_warning());
}

#[test]
fn or_running_negated() {
    // run=false so job never ran; ok = exit_status=1,OR_running (OR_running False, exit_status=1 not run)
    // warn = !OR_running → negated OR_running is True when not running
    let job = run_check(
        &["/bin/echo", "test"],
        "exit_status=1,OR_running",
        "!OR_running",
        false,
        false,
    );
    assert!(!job.is_ok());
    assert!(job.is_warning());
}

#[test]
fn output_contains_negated() {
    let job = run_check(
        &["/bin/echo", "STATUS_TESTING_OK"],
        "exit_status=0,!output_contains=ERROR",
        "",
        true,
        false,
    );
    assert!(job.is_ok());
    assert_eq!(
        job.check_reason(),
        Some("exit=0, !output_contains=ERROR==True")
    );
}

#[test]
fn output_matches_negated() {
    let job = run_check(
        &["/bin/echo", "STATUS_TESTING_OK"],
        "exit_status=0,!output_matches=.*ERROR.*",
        "",
        true,
        false,
    );
    assert!(job.is_ok());
    assert_eq!(
        job.check_reason(),
        Some("exit=0, !output_matches=.*ERROR.*==True")
    );
}
