use anyhow::Result;
use clap::Args;

use crate::config::{Config, paths::resolve_kb_root};
use crate::search::BM25Index;
use crate::ui;

#[derive(Debug, Args)]
pub struct SearchArgs {
    /// Search query
    pub query: String,

    /// Number of results to return
    #[arg(short = 'n', long, default_value = "10")]
    pub top: usize,
}

pub fn run(args: &SearchArgs) -> Result<()> {
    let kb_root = resolve_kb_root()?;
    let config = Config::load(&kb_root)?;
    let wiki_dir = config.wiki_dir(&kb_root);

    let index = BM25Index::build(&wiki_dir)?;

    if index.is_empty() {
        eprintln!("Wiki is empty — nothing to search yet.");
        return Ok(());
    }

    let results = index.search(&args.query, args.top);

    if results.is_empty() {
        eprintln!("No results for \"{}\".", args.query);
        return Ok(());
    }

    ui::search_results::print(&results);

    Ok(())
}
