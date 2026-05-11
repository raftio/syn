use anyhow::{bail, Result};
use clap::Args;
use std::path::Path;

use crate::config::{Config, VaultConfig, WikilinkStyle};
use crate::ui;

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Re-initialise even if a knowledge base already exists
    #[arg(long)]
    pub force: bool,

    /// Initialise inside an existing Obsidian vault (uses syn/ + syn-sources/ dirs)
    #[arg(long)]
    pub vault: bool,

    /// Register this KB under NAME in the global syn config after a successful init
    #[arg(long, value_name = "NAME")]
    pub register: Option<String>,
}

pub fn run(args: &InitArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    if args.vault {
        init_vault(&cwd, args.force)?;
    } else {
        init_plain(&cwd, args.force)?;
    }
    if let Some(name) = &args.register {
        super::vault::register(name, &cwd)?;
    }
    Ok(())
}

pub fn init_plain(root: &Path, force: bool) -> Result<()> {
    let wai_dir = root.join(".syn");
    let config_path = wai_dir.join("config.toml");

    if config_path.exists() && !force {
        bail!(
            "knowledge base already exists at {}. Use --force to re-initialise.",
            root.display()
        );
    }

    let dirs: Vec<std::path::PathBuf> = vec![
        wai_dir.clone(),
        wai_dir.join("cache"),
        root.join("raw"),
        root.join("wiki"),
        root.join("wiki").join("entities"),
        root.join("wiki").join("concepts"),
        root.join("wiki").join("sources"),
        root.join("wiki").join("synthesis"),
    ];

    for dir in &dirs {
        std::fs::create_dir_all(dir)?;
    }

    for subdir in ["raw", "wiki/entities", "wiki/concepts", "wiki/sources", "wiki/synthesis"] {
        let keep = root.join(subdir).join(".gitkeep");
        if !keep.exists() {
            std::fs::write(&keep, "")?;
        }
    }

    let config = Config::default();
    config.save(root)?;

    std::fs::write(
        root.join("CLAUDE.md"),
        include_str!("../../templates/CLAUDE.md.tmpl"),
    )?;
    std::fs::write(
        root.join("index.md"),
        include_str!("../../templates/index.md.tmpl"),
    )?;
    std::fs::write(
        root.join("log.md"),
        include_str!("../../templates/log.md.tmpl"),
    )?;

    ui::init_banner::print(&root.display().to_string(), false);
    ui::init_banner::print_next_steps(false);

    Ok(())
}

pub fn init_vault(root: &Path, force: bool) -> Result<()> {
    let wai_dir = root.join(".syn");
    let config_path = wai_dir.join("config.toml");

    if config_path.exists() && !force {
        bail!(
            "knowledge base already exists at {}. Use --force to re-initialise.",
            root.display()
        );
    }

    if !detect_obsidian_vault(root) {
        eprintln!(
            "Warning: no .obsidian/ directory found at {}. \
             Proceeding anyway — make sure this is an Obsidian vault root.",
            root.display()
        );
    }

    let dirs: Vec<std::path::PathBuf> = vec![
        wai_dir.clone(),
        wai_dir.join("cache"),
        root.join("syn-sources"),
        root.join("syn"),
        root.join("syn").join("entities"),
        root.join("syn").join("concepts"),
        root.join("syn").join("sources"),
        root.join("syn").join("synthesis"),
    ];

    for dir in &dirs {
        std::fs::create_dir_all(dir)?;
    }

    for subdir in ["syn-sources", "syn/entities", "syn/concepts", "syn/sources", "syn/synthesis"] {
        let keep = root.join(subdir).join(".gitkeep");
        if !keep.exists() {
            std::fs::write(&keep, "")?;
        }
    }

    let mut config = Config::default();
    config.paths.wiki = "syn".to_string();
    config.paths.raw = "syn-sources".to_string();
    config.vault = Some(VaultConfig {
        obsidian: true,
        wiki_dir: "syn".to_string(),
        raw_dir: "syn-sources".to_string(),
        wikilink_style: WikilinkStyle::Obsidian,
    });
    config.save(root)?;

    std::fs::write(
        root.join("CLAUDE.md"),
        include_str!("../../templates/CLAUDE.md.obsidian.tmpl"),
    )?;
    std::fs::write(
        root.join("index.md"),
        include_str!("../../templates/index.md.obsidian.tmpl"),
    )?;
    std::fs::write(
        root.join("log.md"),
        include_str!("../../templates/log.md.tmpl"),
    )?;

    ui::init_banner::print(&root.display().to_string(), true);
    ui::init_banner::print_next_steps(true);

    Ok(())
}

pub fn detect_obsidian_vault(root: &Path) -> bool {
    root.join(".obsidian").is_dir()
}
