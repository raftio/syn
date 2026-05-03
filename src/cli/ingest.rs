use anyhow::{bail, Result};
use clap::Args;
use std::io::Write;
use std::path::Path;

use crate::config::{Config, paths::resolve_kb_root};
use crate::llm::LlmProvider;
use crate::sources::{self, Source};
use crate::ui;
use crate::wiki::edits::{apply_edits, parse_edit, Edit};
use super::ingest_paths::{self, ResolvedInput};

#[derive(Debug, Args)]
pub struct IngestArgs {
    /// Paths, URLs, directories, or glob patterns to ingest
    #[arg(num_args = 1..)]
    pub paths: Vec<String>,

    /// File extensions to include when walking dirs or expanding globs (comma-separated)
    #[arg(long, value_delimiter = ',', default_value = "md")]
    pub ext: Vec<String>,

    /// Skip sources whose wiki/sources/<slug>.md already exists
    #[arg(long)]
    pub skip_existing: bool,

    /// Override model for this run (e.g. gpt-4o, claude-opus-4-7)
    #[arg(short, long)]
    pub model: Option<String>,

    /// Show planned edits without applying them
    #[arg(long)]
    pub dry_run: bool,

    /// Apply edits without interactive confirmation
    #[arg(short, long)]
    pub yes: bool,
}

struct IngestFailure {
    input: String,
    stage: &'static str,
    error: anyhow::Error,
}

pub async fn run(args: &IngestArgs) -> Result<()> {
    let kb_root = resolve_kb_root()?;
    let mut config = Config::load(&kb_root)?;

    if let Some(m) = &args.model {
        config.llm.model = m.clone();
    }

    let provider = LlmProvider::from_config(&config.llm)?;

    // Phase 1: expand inputs → deduplicated, filtered list
    let inputs = ingest_paths::expand(&args.paths, &args.ext)?;

    if inputs.is_empty() {
        eprintln!("No matching files found.");
        return Ok(());
    }

    // Apply --skip-existing filter
    let wiki_dir = config.wiki_dir(&kb_root);
    let inputs: Vec<ResolvedInput> = if args.skip_existing {
        inputs
            .into_iter()
            .filter(|inp| {
                let slug = tentative_slug(inp);
                let skip = already_ingested(&wiki_dir, &slug);
                if skip {
                    eprintln!("Skipping (already ingested): {}", inp.display_name());
                }
                !skip
            })
            .collect()
    } else {
        inputs
    };

    if inputs.is_empty() {
        eprintln!("All sources already ingested.");
        return Ok(());
    }

    // Phase 2: load each source and plan edits via LLM
    let mut all_plans: Vec<(String, Vec<Edit>)> = Vec::new();
    let mut failures: Vec<IngestFailure> = Vec::new();
    let multi = inputs.len() > 1;

    for input in &inputs {
        let display = input.display_name();

        let source = match load_input(input).await {
            Ok(s) => s,
            Err(e) => {
                failures.push(IngestFailure { input: display, stage: "load", error: e });
                continue;
            }
        };

        ui::ingest_plan::print_source_header(
            all_plans.len() + failures.len() + 1,
            inputs.len(),
            &source.title,
            &config.llm.model,
        );
        if !multi {
            eprintln!();
        }

        let req = crate::prompts::ingest::build(&source, &kb_root, &config);

        let tool_results = match provider
            .stream_message(&req, |text| {
                if !multi {
                    print!("{text}");
                    let _ = std::io::stdout().flush();
                }
            })
            .await
        {
            Ok(r) => r,
            Err(e) => {
                failures.push(IngestFailure { input: display, stage: "llm", error: e });
                if !multi {
                    println!();
                }
                continue;
            }
        };
        if !multi {
            println!();
        }

        let mut edits: Vec<Edit> = Vec::new();
        for result in &tool_results {
            if result.name != "wiki_edit" {
                continue;
            }
            match parse_edit(&result.input) {
                Ok(edit) => edits.push(edit),
                Err(e) => {
                    eprintln!("Warning: could not parse wiki_edit call: {e}");
                }
            }
        }

        all_plans.push((display, edits));
    }

    if multi {
        ui::ingest_plan::end_progress();
    }

    // Phase 3: display aggregate plan
    let total_edits: usize = all_plans.iter().map(|(_, e)| e.len()).sum();

    if total_edits == 0 && failures.is_empty() {
        eprintln!("\nNo wiki edits proposed.");
        return Ok(());
    }

    if total_edits > 0 {
        ui::ingest_plan::print_edit_plan(&all_plans, multi);
    }

    if args.dry_run {
        eprintln!("\n[dry-run] No changes written.");
        let had_failures = !failures.is_empty();
        print_failures(&failures);
        if had_failures {
            bail!("{} source(s) failed", failures.len());
        }
        return Ok(());
    }

    if total_edits > 0 && !args.yes && !confirm()? {
        eprintln!("Aborted.");
        print_failures(&failures);
        return Ok(());
    }

    // Phase 4: apply all edits in one shot
    if total_edits > 0 {
        let all_edits: Vec<Edit> = all_plans.into_iter().flat_map(|(_, e)| e).collect();
        match apply_edits(&all_edits, &kb_root) {
            Ok(applied) => {
                ui::ingest_plan::print_applied(&applied);
            }
            Err(e) => {
                failures.push(IngestFailure {
                    input: "<apply>".to_string(),
                    stage: "apply",
                    error: e,
                });
            }
        }
    }

    print_failures(&failures);

    if !failures.is_empty() {
        bail!("{} source(s) failed to ingest", failures.len());
    }

    Ok(())
}

async fn load_input(input: &ResolvedInput) -> Result<Source> {
    match input {
        ResolvedInput::File(p) => {
            if !p.exists() {
                bail!("source file not found: {}", p.display());
            }
            sources::load(p)
        }
        ResolvedInput::Url(u) => {
            eprintln!("Fetching {u} …");
            sources::url::load(u).await
        }
    }
}

fn tentative_slug(input: &ResolvedInput) -> String {
    match input {
        ResolvedInput::File(p) => {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("untitled");
            sources::to_slug(stem)
        }
        ResolvedInput::Url(u) => {
            let no_query = u.split('?').next().unwrap_or(u);
            let segment = no_query
                .trim_end_matches('/')
                .rsplit('/')
                .find(|s| !s.is_empty())
                .unwrap_or("untitled");
            let stem = segment.split('.').next().unwrap_or(segment);
            sources::to_slug(stem)
        }
    }
}

fn already_ingested(wiki_dir: &Path, slug: &str) -> bool {
    wiki_dir.join("sources").join(format!("{slug}.md")).exists()
}

fn print_failures(failures: &[IngestFailure]) {
    if failures.is_empty() {
        return;
    }
    let triples: Vec<(&str, &str, String)> = failures
        .iter()
        .map(|f| (f.stage, f.input.as_str(), f.error.to_string()))
        .collect();
    let refs: Vec<(&str, &str, &str)> = triples
        .iter()
        .map(|(s, i, e)| (*s, *i, e.as_str()))
        .collect();
    ui::ingest_plan::print_failures(&refs);
}

fn confirm() -> Result<bool> {
    eprint!("\nApply these edits? [y/N] ");
    std::io::stderr().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(matches!(line.trim().to_lowercase().as_str(), "y" | "yes"))
}
