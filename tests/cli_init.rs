use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn wai(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("syn").unwrap();
    cmd.current_dir(dir.path());
    cmd
}

#[test]
fn init_overwrites_existing_scaffold_files() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("CLAUDE.md"), "PREEXISTING_CLAUDE\n").unwrap();
    std::fs::write(dir.path().join("index.md"), "PREEXISTING_INDEX\n").unwrap();
    std::fs::write(dir.path().join("log.md"), "PREEXISTING_LOG\n").unwrap();
    wai(&dir).arg("init").assert().success();
    let claude = std::fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
    assert!(claude.contains("# Knowledge Base Schema"));
    assert!(!claude.contains("PREEXISTING_CLAUDE"));
    let index = std::fs::read_to_string(dir.path().join("index.md")).unwrap();
    assert!(!index.contains("PREEXISTING_INDEX"));
    let log = std::fs::read_to_string(dir.path().join("log.md")).unwrap();
    assert!(!log.contains("PREEXISTING_LOG"));
}

#[test]
fn init_vault_overwrites_existing_scaffold_files() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir(dir.path().join(".obsidian")).unwrap();
    std::fs::write(dir.path().join("CLAUDE.md"), "PREEXISTING_VAULT_CLAUDE\n").unwrap();
    wai(&dir).args(["init", "--vault"]).assert().success();
    let claude = std::fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
    assert!(claude.contains("<vault-root>/"));
    assert!(!claude.contains("PREEXISTING_VAULT_CLAUDE"));
}

#[test]
fn init_creates_expected_structure() {
    let dir = TempDir::new().unwrap();
    wai(&dir).arg("init").assert().success();

    let root = dir.path();
    assert!(root.join(".syn/config.toml").exists(), ".syn/config.toml");
    assert!(root.join("CLAUDE.md").exists(), "CLAUDE.md");
    assert!(root.join("index.md").exists(), "index.md");
    assert!(root.join("log.md").exists(), "log.md");
    assert!(root.join("raw").is_dir(), "raw/");
    assert!(root.join("wiki").is_dir(), "wiki/");
    assert!(root.join("wiki/entities").is_dir(), "wiki/entities/");
    assert!(root.join("wiki/concepts").is_dir(), "wiki/concepts/");
    assert!(root.join("wiki/sources").is_dir(), "wiki/sources/");
    assert!(root.join("wiki/synthesis").is_dir(), "wiki/synthesis/");
}

#[test]
fn init_config_has_correct_defaults() {
    let dir = TempDir::new().unwrap();
    wai(&dir).arg("init").assert().success();

    let config_str = std::fs::read_to_string(dir.path().join(".syn/config.toml")).unwrap();
    assert!(config_str.contains("claude-sonnet-4-6"), "default model");
    assert!(config_str.contains("ANTHROPIC_API_KEY"), "api key env var");
    assert!(config_str.contains("auto_commit = false"), "auto_commit default");
}

#[test]
fn init_fails_if_already_initialised() {
    let dir = TempDir::new().unwrap();
    wai(&dir).arg("init").assert().success();
    wai(&dir)
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn init_force_reinitialises() {
    let dir = TempDir::new().unwrap();
    wai(&dir).arg("init").assert().success();
    wai(&dir).arg("init").arg("--force").assert().success();
    assert!(dir.path().join(".syn/config.toml").exists());
}

#[test]
fn init_prints_next_steps() {
    let dir = TempDir::new().unwrap();
    wai(&dir)
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("ANTHROPIC_API_KEY"));
}

#[test]
fn version_flag_works() {
    Command::cargo_bin("syn")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("0.1.0"));
}
