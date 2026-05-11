use anyhow::Result;
use clap::Args;
use std::io::Write;

use crate::config::{Config, paths::resolve_kb_root};

use super::Cli;
use crate::llm::LlmProvider;
use crate::wiki::edits::{apply_edits, parse_edit};
use crate::wiki::lint;

#[derive(Debug, Args)]
pub struct LintArgs {
    /// Auto-apply suggested fixes via LLM
    #[arg(long)]
    pub fix: bool,

    /// Skip the LLM analysis pass (static checks only)
    #[arg(long)]
    pub static_only: bool,

    /// Override model for this run
    #[arg(short, long)]
    pub model: Option<String>,

    /// Apply fixes without interactive confirmation (requires --fix)
    #[arg(short, long)]
    pub yes: bool,
}

pub async fn run(args: &LintArgs, cli: &Cli) -> Result<()> {
    let kb_root = resolve_kb_root(&cli.kb_resolve_opts())?;
    let mut config = Config::load(&kb_root)?;

    if let Some(m) = &args.model {
        config.llm.model = m.clone();
    }

    let wiki_dir = config.wiki_dir(&kb_root);
    let index_path = config.index_path(&kb_root);

    // ── Static analysis ───────────────────────────────────────────────────────
    eprintln!("Running static analysis…\n");
    let report = lint::analyze(&wiki_dir, &kb_root, &index_path, config.wikilink_style())?;
    report.print();

    if args.static_only {
        return Ok(());
    }

    // ── LLM analysis ─────────────────────────────────────────────────────────
    eprintln!("Running LLM analysis [{}]…\n", config.llm.model);

    let req = crate::prompts::lint::build(&report, &kb_root, &config, args.fix);
    let provider = LlmProvider::from_config(&config.llm)?;

    let tool_results = provider
        .stream_message(&req, |text| {
            print!("{text}");
            let _ = std::io::stdout().flush();
        })
        .await?;
    println!();

    // ── Apply fixes if requested ──────────────────────────────────────────────
    if !args.fix || tool_results.is_empty() {
        return Ok(());
    }

    let mut edits = Vec::new();
    for r in &tool_results {
        if r.name == "wiki_edit" {
            match parse_edit(&r.input) {
                Ok(e) => edits.push(e),
                Err(err) => eprintln!("Warning: could not parse edit: {err}"),
            }
        }
    }

    if edits.is_empty() {
        eprintln!("\nNo fixes proposed by LLM.");
        return Ok(());
    }

    eprintln!("\nProposed fixes ({}):", edits.len());
    for e in &edits {
        eprintln!("  {} {}", e.op, e.path);
    }

    let apply = args.yes || confirm()?;
    if !apply {
        eprintln!("Skipped.");
        return Ok(());
    }

    let applied = apply_edits(&edits, &kb_root)?;
    eprintln!("\nApplied {} fix(es):", applied.len());
    for a in &applied {
        eprintln!("  ✓ {a}");
    }

    Ok(())
}

fn confirm() -> Result<bool> {
    eprint!("\nApply these fixes? [y/N] ");
    std::io::stderr().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(matches!(line.trim().to_lowercase().as_str(), "y" | "yes"))
}
