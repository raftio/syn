use std::path::Path;

use crate::config::{Config, WikilinkStyle};
use crate::llm::messages::{Message, MessageRequest, Role, wiki_edit_tool};
use crate::wiki::lint::LintReport;

const MAX_PAGE_CHARS: usize = 400;
const MAX_PAGES_IN_PROMPT: usize = 20;

pub fn build(report: &LintReport, kb_root: &Path, config: &Config, fix: bool) -> MessageRequest {
    let schema = if config.ingest.include_schema_in_prompt {
        std::fs::read_to_string(config.schema_path(kb_root)).ok()
    } else {
        None
    };

    let index_content =
        std::fs::read_to_string(config.index_path(kb_root)).unwrap_or_default();

    let mut msg = String::new();

    // Static findings summary
    msg.push_str("## Static Analysis Findings\n\n");
    if report.is_clean() {
        msg.push_str("No static issues detected.\n\n");
    } else {
        if !report.orphan_pages.is_empty() {
            msg.push_str(&format!(
                "**Orphan pages** (no inbound links): {}\n",
                report.orphan_pages.len()
            ));
            for p in &report.orphan_pages {
                msg.push_str(&format!("  - {p}\n"));
            }
            msg.push('\n');
        }
        if !report.broken_links.is_empty() {
            msg.push_str(&format!(
                "**Broken wikilinks**: {}\n",
                report.broken_links.len()
            ));
            for (from, to) in &report.broken_links {
                msg.push_str(&format!("  - {from} → [[{to}]]\n"));
            }
            msg.push('\n');
        }
        if !report.missing_from_index.is_empty() {
            msg.push_str(&format!(
                "**Pages not in index.md**: {}\n",
                report.missing_from_index.len()
            ));
            for p in &report.missing_from_index {
                msg.push_str(&format!("  - {p}\n"));
            }
            msg.push('\n');
        }
    }

    // Wiki index
    msg.push_str("## index.md\n\n");
    msg.push_str(&index_content);
    msg.push_str("\n\n");

    // Sampled wiki pages
    let wiki_dir = config.wiki_dir(kb_root);
    let pages = sample_pages(&wiki_dir, kb_root, report, MAX_PAGES_IN_PROMPT);
    if !pages.is_empty() {
        msg.push_str("## Wiki Pages (sample)\n\n");
        for (rel, content) in &pages {
            let truncated = if content.len() > MAX_PAGE_CHARS {
                format!("{}…", &content[..MAX_PAGE_CHARS])
            } else {
                content.clone()
            };
            msg.push_str(&format!("### `{rel}`\n\n{truncated}\n\n---\n\n"));
        }
    }

    // Instructions
    msg.push_str(
        "## Your task\n\n\
         Review the wiki for:\n\
         1. **Contradictions** — claims in different pages that conflict\n\
         2. **Stale claims** — statements superseded by newer sources\n\
         3. **Missing pages** — concepts frequently referenced but lacking their own page\n\
         4. **Weak cross-references** — pages that should link to each other but don't\n\
         5. **Suggested questions** — gaps in knowledge worth investigating\n\n",
    );

    let wikilink_note = match config.wikilink_style() {
        WikilinkStyle::Syn => format!(
            "Use `[[{}/path.md]]` wikilinks when referencing pages.",
            config.wiki_dir_name()
        ),
        WikilinkStyle::Obsidian => {
            "Use `[[Note Name]]` wikilinks (Obsidian-style, note title only) when referencing pages.".to_string()
        }
    };
    msg.push_str(&format!("{wikilink_note}\n\n"));

    if fix {
        msg.push_str(
            "For each issue you can fix automatically, use the `wiki_edit` tool.\n\
             Focus on adding missing cross-references and creating stub pages for \
             frequently-referenced concepts that lack pages.\n",
        );
    } else {
        msg.push_str(
            "Report your findings as a structured list. \
             Do not attempt to fix anything — just identify issues and suggestions.\n",
        );
    }

    let tools = if fix { vec![wiki_edit_tool()] } else { vec![] };

    MessageRequest {
        system: schema,
        messages: vec![Message { role: Role::User, content: msg }],
        tools,
        model: config.llm.model.clone(),
        max_tokens: config.llm.max_tokens,
    }
}

/// Return up to `max` pages, prioritising orphans and pages with broken links.
fn sample_pages(wiki_dir: &Path, kb_root: &Path, report: &LintReport, max: usize) -> Vec<(String, String)> {
    let mut priority: Vec<String> = report
        .orphan_pages
        .iter()
        .chain(report.broken_links.iter().map(|(f, _)| f))
        .cloned()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Fill remaining slots with anything in the wiki dir
    if priority.len() < max {
        if let Ok(mut extra) = collect_all_wiki_pages(wiki_dir, kb_root) {
            extra.retain(|p| !priority.contains(p));
            priority.extend(extra.into_iter().take(max - priority.len()));
        }
    }

    priority.truncate(max);

    priority
        .into_iter()
        .filter_map(|rel| {
            let content = std::fs::read_to_string(kb_root.join(&rel)).ok()?;
            Some((rel, content))
        })
        .collect()
}

fn collect_all_wiki_pages(wiki_dir: &Path, kb_root: &Path) -> anyhow::Result<Vec<String>> {
    let mut paths = Vec::new();
    walk_md(wiki_dir, &mut paths)?;
    Ok(paths
        .iter()
        .filter_map(|p| {
            p.strip_prefix(kb_root)
                .ok()
                .map(|r| r.to_string_lossy().replace('\\', "/"))
        })
        .collect())
}

fn walk_md(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> anyhow::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_md(&path, out)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
            out.push(path);
        }
    }
    Ok(())
}
