pub mod config_cmd;
pub mod init;
pub mod ingest;
pub mod ingest_paths;
pub mod lint;
pub mod log_cmd;
pub mod query;
pub mod search;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "syn", about = "Personal knowledge base — LLM-maintained wiki")]
#[command(version, propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialise a new knowledge base in the current directory
    Init(init::InitArgs),

    /// Ingest a source document or URL into the wiki
    Ingest(ingest::IngestArgs),

    /// Query the wiki
    Query(query::QueryArgs),

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
