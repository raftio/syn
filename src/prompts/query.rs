use std::path::Path;

use crate::config::{Config, WikilinkStyle};
use crate::llm::messages::{Message, MessageRequest, Role};
use crate::search::{BM25Index, SearchResult};

const MAX_PAGE_CHARS: usize = 3000;

pub fn build(
    question: &str,
    kb_root: &Path,
    config: &Config,
    results: &[SearchResult],
) -> MessageRequest {
    let schema = if config.ingest.include_schema_in_prompt {
        std::fs::read_to_string(config.schema_path(kb_root)).ok()
    } else {
        None
    };

    let index_content =
        std::fs::read_to_string(config.index_path(kb_root)).unwrap_or_default();

    let mut user_msg = format!("## Question\n\n{question}\n\n---\n\n## Wiki Index\n\n{index_content}\n");

    if results.is_empty() {
        user_msg.push_str("\n*(No relevant wiki pages found — answer based on the index alone.)*\n");
    } else {
        user_msg.push_str("\n## Relevant Wiki Pages\n");
        for r in results {
            let content = BM25Index::page_content(kb_root, &r.path)
                .unwrap_or_default();
            let (body, _) = crate::search::strip_frontmatter_pub(&content);
            let truncated = if body.len() > MAX_PAGE_CHARS {
                format!("{}…\n*(truncated)*", &body[..MAX_PAGE_CHARS])
            } else {
                body.to_string()
            };
            user_msg.push_str(&format!("\n### {} (`{}`)\n\n{}\n", r.title, r.path, truncated));
        }
    }

    let cite_instruction = match config.wikilink_style() {
        WikilinkStyle::Syn => {
            format!("Cite pages with `[[{}/path.md]]` wikilinks.", config.wiki_dir_name())
        }
        WikilinkStyle::Obsidian => {
            "Cite pages with `[[Note Name]]` wikilinks (Obsidian-style, note title only).".to_string()
        }
    };
    user_msg.push_str(&format!(
        "\n---\n\nAnswer the question using the wiki content above.\n\
         {cite_instruction}\n\
         If the wiki lacks enough information, say so clearly.\n",
    ));

    MessageRequest {
        system: schema,
        messages: vec![Message { role: Role::User, content: user_msg }],
        tools: vec![],
        model: config.llm.model.clone(),
        max_tokens: config.llm.max_tokens,
    }
}
