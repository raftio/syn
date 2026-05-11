//! Machine-wide syn config (`<config_dir>/syn/config.toml`): vault registry and default vault.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Path to the machine-wide syn config file.
///
/// If `SYN_GLOBAL_CONFIG` is set, it must be the full path to `config.toml` (used in tests).
/// Otherwise: `<config_dir>/syn/config.toml` via the `dirs` crate.
pub fn global_config_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SYN_GLOBAL_CONFIG") {
        return Some(PathBuf::from(p));
    }
    dirs::config_dir().map(|d| d.join("syn").join("config.toml"))
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalSynConfig {
    /// When set, used as the KB root if nothing else matches (must exist in `vaults`).
    #[serde(default)]
    pub default_vault: Option<String>,
    #[serde(default)]
    pub vaults: HashMap<String, VaultEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultEntry {
    /// Absolute path to the knowledge base root (directory containing `.syn/`).
    pub root: String,
}

pub fn load_global_config() -> Result<GlobalSynConfig> {
    let Some(path) = global_config_path() else {
        return Ok(GlobalSynConfig::default());
    };
    if !path.is_file() {
        return Ok(GlobalSynConfig::default());
    }
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(GlobalSynConfig::default());
    }
    toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))
}

pub fn save_global_config(cfg: &GlobalSynConfig) -> Result<()> {
    let path = global_config_path().context("could not resolve global config directory")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let content = toml::to_string_pretty(cfg).context("serializing global config")?;
    std::fs::write(&path, content).with_context(|| format!("writing {}", path.display()))
}

/// Resolve a registered vault name to its KB root, if present and valid.
pub fn resolve_vault_named(global: &GlobalSynConfig, name: &str) -> Option<PathBuf> {
    let entry = global.vaults.get(name)?;
    let root = PathBuf::from(&entry.root);
    if root.join(".syn").join("config.toml").is_file() {
        Some(root)
    } else {
        None
    }
}

/// When exactly one vault is registered and valid, use it without `default_vault` or flags.
pub fn sole_registered_vault_root(global: &GlobalSynConfig) -> Option<PathBuf> {
    if global.vaults.len() != 1 {
        return None;
    }
    let name = global.vaults.keys().next()?;
    resolve_vault_named(global, name.as_str())
}

pub fn validate_kb_root(root: &Path) -> bool {
    root.join(".syn").join("config.toml").is_file()
}
