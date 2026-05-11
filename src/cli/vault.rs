use anyhow::{bail, Context, Result};
use clap::Subcommand;
use std::path::{Path, PathBuf};

use crate::config::global::{self, VaultEntry, load_global_config, save_global_config};

use super::init::{detect_obsidian_vault, init_plain, init_vault};

#[derive(Debug, Subcommand)]
pub enum VaultCommand {
    /// List registered vaults and the global config path
    List,
    /// Register a knowledge base by name; runs `syn init` here if `.syn/config.toml` is missing
    Add {
        name: String,
        /// Absolute or relative path to the KB root
        root: PathBuf,
    },
    /// Set the default vault for when CWD is not inside a KB
    Default { name: String },
    /// Remove this vault from the global registry and delete its `.syn/` directory (wiki/raw content is kept)
    Clean { name: String },
}

pub fn run(action: &VaultCommand) -> Result<()> {
    match action {
        VaultCommand::List => list(),
        VaultCommand::Add { name, root } => add(name, root),
        VaultCommand::Default { name } => set_default(name),
        VaultCommand::Clean { name } => clean(name),
    }
}

fn list() -> Result<()> {
    let path = global::global_config_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(no config directory)".to_string());
    println!("Global config: {path}");

    let cfg = load_global_config()?;
    if cfg.vaults.is_empty() {
        println!("No vaults registered. Use: syn vault add NAME /path/to/kb");
        println!("Tip: export SYN_VAULT=NAME or register one vault to use syn from any directory.");
        return Ok(());
    }

    let def = cfg.default_vault.as_deref();
    let mut names: Vec<_> = cfg.vaults.keys().collect();
    names.sort();
    for n in names {
        let entry = cfg.vaults.get(n).unwrap();
        let mark = if def == Some(n.as_str()) { " (default)" } else { "" };
        println!("  {n}{mark} → {}", entry.root);
    }
    Ok(())
}

fn add(name: &str, root: &Path) -> Result<()> {
    let root = root
        .canonicalize()
        .with_context(|| format!("resolving path {}", root.display()))?;
    if !global::validate_kb_root(&root) {
        if detect_obsidian_vault(&root) {
            eprintln!(
                "No syn config at {} — initialising Obsidian-style layout (`syn/` + `syn-sources/`).",
                root.display()
            );
            init_vault(&root, false)?;
        } else {
            eprintln!(
                "No syn config at {} — initialising default wiki layout (`wiki/` + `raw/`).",
                root.display()
            );
            init_plain(&root, false)?;
        }
        if !global::validate_kb_root(&root) {
            bail!(
                "syn initialisation did not produce .syn/config.toml at {}",
                root.display()
            );
        }
    }

    let mut cfg = load_global_config()?;
    cfg.vaults.insert(
        name.to_string(),
        VaultEntry {
            root: root.display().to_string(),
        },
    );
    save_global_config(&cfg)?;
    eprintln!("Registered vault '{name}' → {}", root.display());
    Ok(())
}

fn set_default(name: &str) -> Result<()> {
    let mut cfg = load_global_config()?;
    if !cfg.vaults.contains_key(name) {
        bail!("unknown vault '{name}' — run `syn vault list`");
    }
    cfg.default_vault = Some(name.to_string());
    save_global_config(&cfg)?;
    eprintln!("Default vault set to '{name}'");
    Ok(())
}

fn clean(name: &str) -> Result<()> {
    let mut cfg = load_global_config()?;
    let Some(entry) = cfg.vaults.get(name) else {
        bail!("unknown vault '{name}' — run `syn vault list`");
    };
    let root = PathBuf::from(&entry.root);
    let syn_dir = root.join(".syn");

    if root.exists() && syn_dir.exists() {
        std::fs::remove_dir_all(&syn_dir)
            .with_context(|| format!("removing {}", syn_dir.display()))?;
    }

    cfg.vaults.remove(name);
    if cfg.default_vault.as_deref() == Some(name) {
        cfg.default_vault = None;
    }
    save_global_config(&cfg)?;
    eprintln!("Cleaned vault '{name}' (removed .syn/ and global registration)");
    Ok(())
}

/// Append or update a vault entry after `syn init` (uses absolute `root`).
pub fn register(name: &str, root: &std::path::Path) -> Result<()> {
    let root = root
        .canonicalize()
        .with_context(|| format!("resolving path {}", root.display()))?;
    let mut cfg = load_global_config()?;
    cfg.vaults.insert(
        name.to_string(),
        VaultEntry {
            root: root.display().to_string(),
        },
    );
    save_global_config(&cfg)?;
    eprintln!("Registered vault '{name}' → {}", root.display());
    Ok(())
}
