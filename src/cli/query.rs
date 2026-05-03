use anyhow::Result;
use clap::Args;
use std::io::Write;

use crate::config::{Config, paths::resolve_kb_root};
use crate::llm::LlmProvider;
use crate::search::BM25Index;
use crate::sources::to_slug;
use crate::ui;

#[derive(Debug, Args)]
pub struct QueryArgs {
    /// Question to ask the wiki
    pub question: String,

    /// Save the answer as a synthesis page (provide a slug, e.g. my-analysis)
    #[arg(long, value_name = "SLUG")]
    pub save: Option<String>,

    /// Override model for this run
    #[arg(short, long)]
    pub model: Option<String>,
}

pub async fn run(args: &QueryArgs) -> Result<()> {
    let kb_root = resolve_kb_root()?;
    let mut config = Config::load(&kb_root)?;

    if let Some(m) = &args.model {
        config.llm.model = m.clone();
    }

    let wiki_dir = config.wiki_dir(&kb_root);
    let index = BM25Index::build(&wiki_dir)?;

    if index.is_empty() {
        eprintln!("Wiki is empty — ingest some sources first with `syn ingest`.");
    }

    let results = index.search(&args.question, config.search.top_k);

    if !results.is_empty() {
        ui::query_header::print(results.len(), &config.llm.model);
    }

    let req = crate::prompts::query::build(&args.question, &kb_root, &config, &results);
    let provider = LlmProvider::from_config(&config.llm)?;
    let mut answer = String::new();
    provider
        .stream_message(&req, |text| {
            print!("{text}");
            answer.push_str(text);
            let _ = std::io::stdout().flush();
        })
        .await?;
    println!();

    if let Some(slug) = &args.save {
        let slug = if slug.is_empty() {
            to_slug(&args.question.split_whitespace().take(5).collect::<Vec<_>>().join(" "))
        } else {
            slug.clone()
        };
        save_answer(&answer, &args.question, &slug, &kb_root, &config)?;
        eprintln!("\nSaved → wiki/synthesis/{slug}.md");
    }

    Ok(())
}

fn save_answer(
    answer: &str,
    question: &str,
    slug: &str,
    kb_root: &std::path::Path,
    config: &Config,
) -> Result<()> {
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let page_path = kb_root.join("wiki").join("synthesis").join(format!("{slug}.md"));

    let content = format!(
        "---\ntitle: \"{question}\"\ntags: [synthesis]\nupdated: {date}\n---\n\n# {question}\n\n{answer}\n"
    );

    if let Some(parent) = page_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&page_path, &content)?;

    // Append to index.md
    let index_path = config.index_path(kb_root);
    let entry = format!("- [{question}](wiki/synthesis/{slug}.md) — query answer\n");
    use std::io::Write as _;
    let mut f = std::fs::OpenOptions::new().append(true).open(&index_path)?;
    writeln!(f, "{entry}")?;

    // Append to log.md
    let log_path = config.log_path(kb_root);
    let log_entry = format!("## [{date}] query | {question}\n");
    let mut f = std::fs::OpenOptions::new().append(true).open(&log_path)?;
    writeln!(f, "{log_entry}")?;

    Ok(())
}
