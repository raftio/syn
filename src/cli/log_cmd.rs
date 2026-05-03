use anyhow::Result;
use clap::Args;

use crate::config::{Config, paths::resolve_kb_root};
use crate::ui;
use crate::wiki::log::read_recent;

#[derive(Debug, Args)]
pub struct LogArgs {
    /// Number of most recent entries to show
    #[arg(short = 'n', long, default_value = "10")]
    pub tail: usize,
}

pub fn run(args: &LogArgs) -> Result<()> {
    let kb_root = resolve_kb_root()?;
    let config = Config::load(&kb_root)?;
    let log_path = config.log_path(&kb_root);

    let recent = read_recent(&log_path, args.tail);
    ui::log_view::print(&recent);

    Ok(())
}
