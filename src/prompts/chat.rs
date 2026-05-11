use std::path::Path;

use crate::config::{Config, WikilinkStyle};
use crate::search::{BM25Index, SearchResult};

const MAX_PAGE_CHARS: usize = 3000;

fn cite_instruction(config: &Config) -> String {
    match config.wikilink_style() {
        WikilinkStyle::Syn => {
            format!(
                "Cite pages with `[[{}/path.md]]` wikilinks.",
                config.wiki_dir_name()
            )
        }
        WikilinkStyle::Obsidian => {
            "Cite pages with `[[Note Name]]` wikilinks (Obsidian-style, note title only).".to_string()
        }
    }
}

/// System prompt for multi-turn chat: schema (optional), wiki index once, citation rules.
pub fn build_system(kb_root: &Path, config: &Config) -> Option<String> {
    let mut out = String::new();

    if config.ingest.include_schema_in_prompt {
        if let Ok(schema) = std::fs::read_to_string(config.schema_path(kb_root)) {
            if !schema.is_empty() {
                out.push_str(&schema);
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str("\n---\n\n");
            }
        }
    }

    let index_content = std::fs::read_to_string(config.index_path(kb_root)).unwrap_or_default();
    out.push_str("## Wiki Index\n\n");
    out.push_str(&index_content);
    if !out.ends_with('\n') {
        out.push('\n');
    }

    let cite = cite_instruction(config);
    out.push_str(&format!(
        "\n---\n\nYou are helping the user in a multi-turn conversation about their personal wiki.\n\
         Each user message may include a **Relevant Wiki Pages** section from retrieval — prefer those pages when present.\n\
         {cite}\n\
         If the wiki lacks enough information for a question, say so clearly.\n",
    ));

    Some(out)
}

/// One chat turn: user line plus BM25-backed page excerpts (index lives in the system prompt).
pub fn build_user_turn(
    line: &str,
    kb_root: &Path,
    _config: &Config,
    results: &[SearchResult],
) -> String {
    let mut user_msg = format!("## Message\n\n{line}\n");

    if results.is_empty() {
        user_msg.push_str(
            "\n*(No relevant wiki pages found for this message — use the wiki index in your instructions.)*\n",
        );
    } else {
        user_msg.push_str("\n## Relevant Wiki Pages\n");
        for r in results {
            let content = BM25Index::page_content(kb_root, &r.path).unwrap_or_default();
            let (body, _) = crate::search::strip_frontmatter_pub(&content);
            let truncated = if body.len() > MAX_PAGE_CHARS {
                format!("{}…\n*(truncated)*", &body[..MAX_PAGE_CHARS])
            } else {
                body.to_string()
            };
            user_msg.push_str(&format!(
                "\n### {} (`{}`)\n\n{}\n",
                r.title, r.path, truncated
            ));
        }
    }

    user_msg
}
