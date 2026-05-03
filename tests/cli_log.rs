use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn wai(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("syn").unwrap();
    cmd.current_dir(dir.path());
    cmd
}

#[test]
fn log_shows_entries() {
    let dir = TempDir::new().unwrap();
    wai(&dir).arg("init").assert().success();

    std::fs::write(
        dir.path().join("log.md"),
        "# Log\n\n## [2026-04-23] ingest | Article One\n\nSummary.\n\n## [2026-04-23] ingest | Article Two\n\nSummary.\n",
    ).unwrap();

    wai(&dir)
        .args(["log", "-n", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Article Two"))
        .stdout(predicate::str::contains("Article One").not());
}

#[test]
fn log_empty_shows_hint() {
    let dir = TempDir::new().unwrap();
    wai(&dir).arg("init").assert().success();

    wai(&dir)
        .arg("log")
        .assert()
        .success()
        .stderr(predicate::str::contains("ingest"));
}

#[test]
fn log_fails_outside_kb() {
    let dir = TempDir::new().unwrap();
    wai(&dir)
        .arg("log")
        .assert()
        .failure()
        .stderr(predicate::str::contains("knowledge base"));
}
