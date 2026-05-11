pub mod chat;
pub mod config_cmd;
pub mod init;
pub mod ingest;
pub mod ingest_paths;
pub mod lint;
pub mod log_cmd;
pub mod query;
pub mod search;
pub mod vault;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "syn", about = "Personal knowledge base — LLM-maintained wiki")]
#[command(version, propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Knowledge base root (directory containing `.syn/`); overrides SYN_KB and discovery
    #[arg(long, global = true, value_name = "PATH")]
    pub kb_root: Option<PathBuf>,

    /// Use a KB registered in the global config file (`syn vault list`; not `init --vault`)
    #[arg(short = 'w', long = "use-vault", global = true, value_name = "NAME")]
    pub use_vault: Option<String>,

    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,
}

impl Cli {
    pub fn kb_resolve_opts(&self) -> crate::config::paths::KbResolveOpts {
        crate::config::paths::KbResolveOpts {
            kb_root: self.kb_root.clone(),
            vault: self.use_vault.clone(),
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialise a new knowledge base in the current directory
    Init(init::InitArgs),

    /// Ingest a source document or URL into the wiki
    Ingest(ingest::IngestArgs),

    /// Query the wiki
    Query(query::QueryArgs),

    /// Interactive multi-turn wiki chat (stdin)
    Chat(chat::ChatArgs),

    /// Search wiki pages locally (BM25)
    Search(search::SearchArgs),

    /// Show recent log entries
    Log(log_cmd::LogArgs),

    /// View or edit configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Health-check the wiki for issues and suggest improvements
    Lint(lint::LintArgs),

    /// List or update registered knowledge bases (machine-wide config)
    Vault {
        #[command(subcommand)]
        action: vault::VaultCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Print all configuration
    Show,
    /// Get a config value (e.g. llm.model)
    Get { key: String },
    /// Set a config value (e.g. llm.model gpt-4o)
    Set { key: String, value: String },
}
