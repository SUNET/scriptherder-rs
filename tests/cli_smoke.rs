use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_scriptherder"))
}

#[test]
fn ls_empty_datadir_runs() {
    let dir = std::env::temp_dir().join(format!("sh_cli_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = bin()
        .arg("-d")
        .arg(&dir)
        .arg("--checkdir")
        .arg(&dir)
        .arg("ls")
        .output()
        .unwrap();
    assert!(out.status.success());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn wrap_then_check_roundtrip() {
    let dir = std::env::temp_dir().join(format!("sh_cli2_{}", std::process::id()));
    let checkdir = dir.join("check");
    std::fs::create_dir_all(&checkdir).unwrap();
    std::fs::write(
        checkdir.join("smoke.ini"),
        "[check]\nok = exit_status=0, max_age=8h\n",
    )
    .unwrap();
    // wrap a successful command
    let w = bin()
        .arg("-d")
        .arg(&dir)
        .arg("--checkdir")
        .arg(&checkdir)
        .arg("wrap")
        .arg("-N")
        .arg("smoke")
        .arg("--")
        .arg("/bin/true")
        .output()
        .unwrap();
    assert!(w.status.success());
    // check should report OK (exit 0)
    let c = bin()
        .arg("-d")
        .arg(&dir)
        .arg("--checkdir")
        .arg(&checkdir)
        .arg("check")
        .arg("smoke")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&c.stdout);
    assert!(stdout.starts_with("OK:"), "got: {stdout}");
    assert_eq!(c.status.code(), Some(0));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn lastlog_empty_datadir_no_jobs_exits_1() {
    let dir = std::env::temp_dir().join(format!("sh_cli3_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = bin()
        .arg("-d")
        .arg(&dir)
        .arg("--checkdir")
        .arg(&dir)
        .arg("lastlog")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.trim(), "No jobs found", "got: {stdout}");
    assert_eq!(out.status.code(), Some(1));
    std::fs::remove_dir_all(&dir).ok();
}
