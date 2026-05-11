//! Shared helpers for integration tests (each `tests/*.rs` binary must `mod common;`).

use std::path::PathBuf;
use tempfile::TempDir;

/// Empty file → no registered vaults; avoids picking up `~/.config/syn/config.toml` on dev machines.
pub fn empty_global_config_path(dir: &TempDir) -> PathBuf {
    let p = dir.path().join("_syn_test_global.toml");
    std::fs::write(&p, "").expect("write empty global config stub");
    p
}
