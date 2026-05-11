mod cli;
mod config;
mod errors;
mod llm;
mod prompts;
mod search;
mod sources;
mod ui;
mod wiki;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let level = match cli.verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("syn={level}")));
    fmt().with_env_filter(filter).without_time().init();

    match &cli.command {
        Command::Init(args) => cli::init::run(args)?,
        Command::Ingest(args) => cli::ingest::run(args, &cli).await?,
        Command::Query(args) => cli::query::run(args, &cli).await?,
        Command::Chat(args) => cli::chat::run(args, &cli).await?,
        Command::Search(args) => cli::search::run(args, &cli)?,
        Command::Log(args) => cli::log_cmd::run(args, &cli)?,
        Command::Config { action } => cli::config_cmd::run(action, &cli)?,
        Command::Lint(args) => cli::lint::run(args, &cli).await?,
        Command::Vault { action } => cli::vault::run(action)?,
    }

    Ok(())
}
