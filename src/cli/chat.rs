use anyhow::Result;
use clap::Args;
use std::io::Write;
use tokio::io::AsyncBufReadExt;

use crate::config::{Config, paths::resolve_kb_root};
use crate::llm::messages::{Message, MessageRequest, Role};
use crate::llm::LlmProvider;
use crate::search::BM25Index;

use super::Cli;

#[derive(Debug, Args)]
pub struct ChatArgs {
    /// Override model for this session
    #[arg(short, long)]
    pub model: Option<String>,
}

pub async fn run(args: &ChatArgs, cli: &Cli) -> Result<()> {
    let kb_root = resolve_kb_root(&cli.kb_resolve_opts())?;
    let mut config = Config::load(&kb_root)?;

    if let Some(m) = &args.model {
        config.llm.model = m.clone();
    }

    let wiki_dir = config.wiki_dir(&kb_root);
    let index = BM25Index::build(&wiki_dir)?;

    if index.is_empty() {
        eprintln!("Wiki is empty — ingest some sources first with `syn ingest`.");
    }

    let system = crate::prompts::chat::build_system(&kb_root, &config);

    eprintln!("syn chat — type `exit` or `quit` (or Ctrl-D) to leave.");
    eprintln!("Using model {}.", config.llm.model);

    let mut messages: Vec<Message> = Vec::new();
    let provider = LlmProvider::from_config(&config.llm)?;
    let stdin = tokio::io::stdin();
    let mut reader = tokio::io::BufReader::new(stdin);
    let mut line_buf = String::new();

    loop {
        eprint!("chat> ");
        let _ = std::io::stderr().flush();

        line_buf.clear();
        let n = reader.read_line(&mut line_buf).await?;
        if n == 0 {
            eprintln!();
            break;
        }

        let line = line_buf.trim();
        if line.is_empty() {
            continue;
        }
        if line.eq_ignore_ascii_case("exit") || line.eq_ignore_ascii_case("quit") {
            break;
        }

        let results = index.search(line, config.search.top_k);
        let user_content = crate::prompts::chat::build_user_turn(line, &kb_root, &config, &results);
        messages.push(Message {
            role: Role::User,
            content: user_content,
        });

        let req = MessageRequest {
            system: system.clone(),
            messages: messages.clone(),
            tools: vec![],
            model: config.llm.model.clone(),
            max_tokens: config.llm.max_tokens,
        };

        let mut answer = String::new();
        provider
            .stream_message(&req, |text| {
                print!("{text}");
                answer.push_str(text);
                let _ = std::io::stdout().flush();
            })
            .await?;
        println!();

        messages.push(Message {
            role: Role::Assistant,
            content: answer,
        });
    }

    Ok(())
}
