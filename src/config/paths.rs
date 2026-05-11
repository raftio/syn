use crate::errors::WaiError;
use std::path::{Path, PathBuf};

use super::global::{
    GlobalSynConfig, load_global_config, resolve_vault_named, sole_registered_vault_root,
    validate_kb_root,
};

/// Optional overrides from the CLI (`--kb-root`, `-w` / `--use-vault`).
#[derive(Debug, Default, Clone)]
pub struct KbResolveOpts {
    pub kb_root: Option<PathBuf>,
    pub vault: Option<String>,
}

/// Walk up from `start` looking for a `.syn/config.toml` marker file.
/// Returns the KB root directory if found.
pub fn find_kb_root(start: &Path) -> Result<PathBuf, WaiError> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(".syn").join("config.toml").exists() {
            return Ok(current);
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return Err(WaiError::NotAKnowledgeBase),
        }
    }
}

fn normalize_kb_candidate(path: PathBuf) -> Result<PathBuf, WaiError> {
    let path = path
        .canonicalize()
        .map_err(|e| WaiError::Config(format!("invalid KB path {}: {e}", path.display())))?;
    if !validate_kb_root(&path) {
        return Err(WaiError::Config(format!(
            "not a syn knowledge base (missing .syn/config.toml): {}",
            path.display()
        )));
    }
    Ok(path)
}

/// Resolve the KB root (all commands use this path):
/// `--kb-root`, `-w` / `--use-vault`, `SYN_VAULT`, `SYN_KB`, walk up from CWD,
/// then global `default_vault`, then the sole registered vault if there is exactly one.
pub fn resolve_kb_root(opts: &KbResolveOpts) -> Result<PathBuf, WaiError> {
    if let Some(p) = &opts.kb_root {
        return normalize_kb_candidate(p.clone());
    }

    if let Some(name) = &opts.vault {
        let global = load_global_config().map_err(|e| WaiError::Config(e.to_string()))?;
        return resolve_vault_cli_name(&global, name);
    }

    if let Ok(name) = std::env::var("SYN_VAULT") {
        let global = load_global_config().map_err(|e| WaiError::Config(e.to_string()))?;
        return resolve_vault_cli_name(&global, &name);
    }

    if let Ok(v) = std::env::var("SYN_KB") {
        let p = PathBuf::from(v);
        return normalize_kb_candidate(p);
    }

    let cwd = std::env::current_dir().map_err(WaiError::Io)?;
    if let Ok(root) = find_kb_root(&cwd) {
        return Ok(root);
    }

    let global = load_global_config().map_err(|e| WaiError::Config(e.to_string()))?;
    if let Some(name) = &global.default_vault {
        if let Some(root) = resolve_vault_named(&global, name) {
            return Ok(root);
        }
    }
    if let Some(root) = sole_registered_vault_root(&global) {
        return Ok(root);
    }

    Err(WaiError::NotAKnowledgeBase)
}

fn resolve_vault_cli_name(global: &GlobalSynConfig, name: &str) -> Result<PathBuf, WaiError> {
    resolve_vault_named(global, name).ok_or_else(|| {
        WaiError::Config(format!(
            "unknown vault '{name}' or missing .syn/config.toml — run `syn vault list`"
        ))
    })
}
