mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn syn() -> Command {
    Command::cargo_bin("syn").unwrap()
}

#[test]
fn log_works_with_syn_kb_outside_vault_tree() {
    let kb = TempDir::new().unwrap();
    syn()
        .current_dir(kb.path())
        .arg("init")
        .assert()
        .success();
    let kb_path = kb.path().canonicalize().unwrap();

    let unrelated = TempDir::new().unwrap();
    syn()
        .current_dir(unrelated.path())
        .env("SYN_KB", kb_path.as_os_str())
        .arg("log")
        .assert()
        .success();
}

#[test]
fn log_works_with_registered_vault_and_flag() {
    let global_home = TempDir::new().unwrap();
    let cfg_path = global_home.path().join("config.toml");

    let kb = TempDir::new().unwrap();
    syn()
        .current_dir(kb.path())
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .arg("init")
        .assert()
        .success();
    let kb_path = kb.path().canonicalize().unwrap();

    syn()
        .current_dir(kb.path())
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .args(["vault", "add", "mine"])
        .arg(&kb_path)
        .assert()
        .success();

    let unrelated = TempDir::new().unwrap();
    syn()
        .current_dir(unrelated.path())
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .args(["--use-vault", "mine", "log"])
        .assert()
        .success();
}

#[test]
fn log_works_with_sole_registered_vault_without_default() {
    let global_home = TempDir::new().unwrap();
    let cfg_path = global_home.path().join("config.toml");

    let kb = TempDir::new().unwrap();
    syn()
        .current_dir(kb.path())
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .arg("init")
        .assert()
        .success();
    let kb_path = kb.path().canonicalize().unwrap();

    syn()
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .args(["vault", "add", "only"])
        .arg(&kb_path)
        .assert()
        .success();

    let unrelated = TempDir::new().unwrap();
    syn()
        .current_dir(unrelated.path())
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .arg("log")
        .assert()
        .success();
}

#[test]
fn log_works_with_syn_vault_env() {
    let global_home = TempDir::new().unwrap();
    let cfg_path = global_home.path().join("config.toml");

    let kb = TempDir::new().unwrap();
    syn()
        .current_dir(kb.path())
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .arg("init")
        .assert()
        .success();
    let kb_path = kb.path().canonicalize().unwrap();

    syn()
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .args(["vault", "add", "envkb"])
        .arg(&kb_path)
        .assert()
        .success();

    let unrelated = TempDir::new().unwrap();
    syn()
        .current_dir(unrelated.path())
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .env("SYN_VAULT", "envkb")
        .arg("log")
        .assert()
        .success();
}

#[test]
fn log_works_with_short_w_flag() {
    let global_home = TempDir::new().unwrap();
    let cfg_path = global_home.path().join("config.toml");

    let kb = TempDir::new().unwrap();
    syn()
        .current_dir(kb.path())
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .arg("init")
        .assert()
        .success();
    let kb_path = kb.path().canonicalize().unwrap();

    syn()
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .args(["vault", "add", "shorty"])
        .arg(&kb_path)
        .assert()
        .success();

    let unrelated = TempDir::new().unwrap();
    syn()
        .current_dir(unrelated.path())
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .args(["-w", "shorty", "log"])
        .assert()
        .success();
}

#[test]
fn log_works_with_default_vault() {
    let global_home = TempDir::new().unwrap();
    let cfg_path = global_home.path().join("config.toml");

    let kb = TempDir::new().unwrap();
    syn()
        .current_dir(kb.path())
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .arg("init")
        .assert()
        .success();
    let kb_path = kb.path().canonicalize().unwrap();

    syn()
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .args(["vault", "add", "home"])
        .arg(&kb_path)
        .assert()
        .success();

    syn()
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .args(["vault", "default", "home"])
        .assert()
        .success();

    let unrelated = TempDir::new().unwrap();
    syn()
        .current_dir(unrelated.path())
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .arg("log")
        .assert()
        .success();
}

#[test]
fn init_register_writes_global_config() {
    let global_home = TempDir::new().unwrap();
    let cfg_path = global_home.path().join("config.toml");

    let kb = TempDir::new().unwrap();
    syn()
        .current_dir(kb.path())
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .args(["init", "--register", "mykb"])
        .assert()
        .success();

    let raw = std::fs::read_to_string(&cfg_path).unwrap();
    assert!(raw.contains("mykb"));
    assert!(raw.contains("vaults"));
}

#[test]
fn vault_add_auto_inits_plain_kb() {
    let global_home = TempDir::new().unwrap();
    let cfg_path = global_home.path().join("config.toml");
    let kb = TempDir::new().unwrap();

    syn()
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .args(["vault", "add", "fresh"])
        .arg(kb.path())
        .assert()
        .success();

    assert!(
        kb.path().join(".syn").join("config.toml").is_file(),
        "expected init to create .syn/config.toml"
    );
    assert!(kb.path().join("wiki").is_dir(), "plain layout uses wiki/");
}

#[test]
fn vault_add_auto_inits_obsidian_layout_when_dot_obsidian_present() {
    let global_home = TempDir::new().unwrap();
    let cfg_path = global_home.path().join("config.toml");
    let kb = TempDir::new().unwrap();
    std::fs::create_dir(kb.path().join(".obsidian")).unwrap();

    syn()
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .args(["vault", "add", "obs"])
        .arg(kb.path())
        .assert()
        .success();

    assert!(kb.path().join("syn").is_dir(), "vault layout uses syn/");
    assert!(
        kb.path().join(".syn").join("config.toml").is_file(),
        "expected .syn/config.toml"
    );
}

#[test]
fn log_fails_outside_kb_without_syn_or_default() {
    let dir = TempDir::new().unwrap();
    syn()
        .current_dir(dir.path())
        .env("SYN_GLOBAL_CONFIG", common::empty_global_config_path(&dir))
        .arg("log")
        .assert()
        .failure()
        .stderr(predicate::str::contains("knowledge base"));
}

#[test]
fn vault_clean_removes_dot_syn_and_registry_keeps_wiki() {
    let global_home = TempDir::new().unwrap();
    let cfg_path = global_home.path().join("config.toml");
    let kb = TempDir::new().unwrap();
    syn()
        .current_dir(kb.path())
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .arg("init")
        .assert()
        .success();
    let kb_path = kb.path().canonicalize().unwrap();

    syn()
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .args(["vault", "add", "cleanme"])
        .arg(&kb_path)
        .assert()
        .success();

    assert!(
        kb.path().join(".syn").join("config.toml").is_file(),
        "precondition: .syn exists"
    );
    assert!(kb.path().join("wiki").is_dir());

    syn()
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .args(["vault", "clean", "cleanme"])
        .assert()
        .success();

    assert!(
        !kb.path().join(".syn").exists(),
        ".syn directory should be removed"
    );
    assert!(
        kb.path().join("wiki").is_dir(),
        "wiki tree should remain after clean"
    );

    let raw = std::fs::read_to_string(&cfg_path).unwrap();
    assert!(
        !raw.contains("cleanme"),
        "vault name should be gone from global config"
    );

    syn()
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .args(["vault", "clean", "cleanme"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown vault"));
}

#[test]
fn vault_clean_clears_default_vault_when_it_matches() {
    let global_home = TempDir::new().unwrap();
    let cfg_path = global_home.path().join("config.toml");
    let kb = TempDir::new().unwrap();
    syn()
        .current_dir(kb.path())
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .arg("init")
        .assert()
        .success();
    let kb_path = kb.path().canonicalize().unwrap();

    syn()
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .args(["vault", "add", "solo"])
        .arg(&kb_path)
        .assert()
        .success();
    syn()
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .args(["vault", "default", "solo"])
        .assert()
        .success();

    syn()
        .env("SYN_GLOBAL_CONFIG", &cfg_path)
        .args(["vault", "clean", "solo"])
        .assert()
        .success();

    let raw = std::fs::read_to_string(&cfg_path).unwrap();
    assert!(!raw.contains("solo"), "solo should be unregistered");
    assert!(
        !raw.contains("default_vault = \"solo\""),
        "default should not still point at removed vault"
    );
}
