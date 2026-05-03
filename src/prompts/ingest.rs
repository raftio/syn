use std::path::Path;

use crate::config::{Config, WikilinkStyle};
use crate::llm::messages::{Message, MessageRequest, Role, wiki_edit_tool};
use crate::sources::Source;
use crate::wiki::log;

pub fn build(source: &Source, kb_root: &Path, config: &Config) -> MessageRequest {
    let schema = if config.ingest.include_schema_in_prompt {
        std::fs::read_to_string(config.schema_path(kb_root)).ok()
    } else {
        None
    };

    let index_content =
        std::fs::read_to_string(config.index_path(kb_root)).unwrap_or_default();
    let recent_log = log::read_recent(&config.log_path(kb_root), 5);

    let wiki_dir_name = config.wiki_dir_name();
    let wikilink_instruction = match config.wikilink_style() {
        WikilinkStyle::Syn => format!(
            "Use `[[{wiki_dir_name}/path/to/page.md]]` wikilinks for cross-references."
        ),
        WikilinkStyle::Obsidian => {
            "Use `[[Note Name]]` wikilinks (Obsidian-style, note title only, no path or extension).".to_string()
        }
    };

    let mut user_msg = format!(
        "Please ingest this source document into the wiki.\n\n\
         ## Source: {title}\n\n\
         {body}\n\n\
         ---\n\n\
         ## Current index.md\n\n\
         {index}\n",
        title = source.title,
        body = source.body,
        index = index_content,
    );

    if !recent_log.is_empty() {
        user_msg.push_str(&format!("\n---\n\n## Recent log entries\n\n{recent_log}\n"));
    }

    user_msg.push_str(&format!(
        "\n---\n\n\
         {wikilink_instruction}\n\n\
         Use the `wiki_edit` tool to create or update wiki pages. Remember to:\n\
         1. Create a summary page at `{wiki_dir_name}/sources/<slug>.md`\n\
         2. Create or update entity and concept pages as needed\n\
         3. Update `index.md` — add or refresh entries for every new/updated page\n\
         4. Append a log entry to `log.md` in the format:\n\
            `## [YYYY-MM-DD] ingest | <Source Title>`\n\n\
         Slug for this source: `"
    ));
    user_msg.push_str(&source.slug);
    user_msg.push_str("`\n");

    MessageRequest {
        system: schema,
        messages: vec![Message { role: Role::User, content: user_msg }],
        tools: vec![wiki_edit_tool()],
        model: config.llm.model.clone(),
        max_tokens: config.llm.max_tokens,
    }
}
