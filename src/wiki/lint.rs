use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::config::WikilinkStyle;

#[derive(Debug, Default)]
pub struct LintReport {
    pub orphan_pages: Vec<String>,
    pub broken_links: Vec<(String, String)>,
    pub missing_from_index: Vec<String>,
    pub index_dead_links: Vec<String>,
    pub no_frontmatter: Vec<String>,
    /// (from_page, wikilink_text, matching_paths) — Obsidian mode only
    pub ambiguous_wikilinks: Vec<(String, String, Vec<String>)>,
}

impl LintReport {
    pub fn is_clean(&self) -> bool {
        self.orphan_pages.is_empty()
            && self.broken_links.is_empty()
            && self.missing_from_index.is_empty()
            && self.index_dead_links.is_empty()
            && self.no_frontmatter.is_empty()
            && self.ambiguous_wikilinks.is_empty()
    }

    pub fn total_issues(&self) -> usize {
        self.orphan_pages.len()
            + self.broken_links.len()
            + self.missing_from_index.len()
            + self.index_dead_links.len()
            + self.no_frontmatter.len()
            + self.ambiguous_wikilinks.len()
    }

    pub fn print(&self) {
        println!("=== Static Analysis ===\n");

        section("Orphan pages (no inbound wikilinks)", &self.orphan_pages);

        if self.broken_links.is_empty() {
            println!("Broken wikilinks: none\n");
        } else {
            println!("Broken wikilinks: {}", self.broken_links.len());
            for (from, to) in &self.broken_links {
                println!("  {from} → [[{to}]] (target missing)");
            }
            println!();
        }

        if !self.ambiguous_wikilinks.is_empty() {
            println!("Ambiguous wikilinks: {}", self.ambiguous_wikilinks.len());
            for (from, link, matches) in &self.ambiguous_wikilinks {
                println!("  {from} → [[{link}]] matches: {}", matches.join(", "));
            }
            println!();
        }

        section("Pages not listed in index.md", &self.missing_from_index);
        section("index.md entries with dead links", &self.index_dead_links);
        section("Pages without frontmatter", &self.no_frontmatter);

        if self.is_clean() {
            println!("✓ No static issues found.");
        } else {
            println!("{} issue(s) total.", self.total_issues());
        }
    }
}

fn section(title: &str, items: &[String]) {
    if items.is_empty() {
        println!("{title}: none\n");
    } else {
        println!("{title}: {}", items.len());
        for item in items {
            println!("  - {item}");
        }
        println!();
    }
}

/// Run static analysis on the wiki directory.
pub fn analyze(
    wiki_dir: &Path,
    kb_root: &Path,
    index_path: &Path,
    style: WikilinkStyle,
) -> Result<LintReport> {
    let mut paths: Vec<PathBuf> = Vec::new();
    if wiki_dir.exists() {
        walk_md(wiki_dir, &mut paths)?;
    }

    let all_pages: Vec<String> = paths
        .iter()
        .filter_map(|p| {
            p.strip_prefix(kb_root)
                .ok()
                .map(|r| r.to_string_lossy().replace('\\', "/"))
        })
        .collect();

    let mut no_frontmatter: Vec<String> = Vec::new();

    // Raw wikilink strings per page (meaning varies by style)
    let mut wikilinks: HashMap<String, Vec<String>> = HashMap::new();

    for (path, rel) in paths.iter().zip(all_pages.iter()) {
        let content = std::fs::read_to_string(path)?;
        if !content.trim_start().starts_with("---") {
            no_frontmatter.push(rel.clone());
        }
        let links = match style {
            WikilinkStyle::Syn => extract_wikilinks(&content),
            WikilinkStyle::Obsidian => extract_obsidian_wikilinks(&content),
        };
        wikilinks.insert(rel.clone(), links);
    }

    let index_content = std::fs::read_to_string(index_path).unwrap_or_default();

    match style {
        WikilinkStyle::Syn => analyze_syn(
            all_pages,
            wikilinks,
            no_frontmatter,
            kb_root,
            &index_content,
        ),
        WikilinkStyle::Obsidian => analyze_obsidian(
            all_pages,
            wikilinks,
            no_frontmatter,
            kb_root,
            &index_content,
        ),
    }
}

fn analyze_syn(
    all_pages: Vec<String>,
    wikilinks: HashMap<String, Vec<String>>,
    no_frontmatter: Vec<String>,
    kb_root: &Path,
    index_content: &str,
) -> Result<LintReport> {
    let mut inbound: HashMap<String, usize> =
        all_pages.iter().map(|p| (p.clone(), 0)).collect();
    for targets in wikilinks.values() {
        for t in targets {
            *inbound.entry(t.clone()).or_default() += 1;
        }
    }

    let orphan_pages: Vec<String> = all_pages
        .iter()
        .filter(|p| *inbound.get(*p).unwrap_or(&0) == 0)
        .cloned()
        .collect();

    let all_set: HashSet<_> = all_pages.iter().cloned().collect();
    let mut broken_links: Vec<(String, String)> = Vec::new();
    for (page, targets) in &wikilinks {
        for t in targets {
            if !all_set.contains(t) && !kb_root.join(t).exists() {
                broken_links.push((page.clone(), t.clone()));
            }
        }
    }

    let missing_from_index: Vec<String> = all_pages
        .iter()
        .filter(|p| !index_content.contains(p.as_str()))
        .cloned()
        .collect();

    let index_dead_links: Vec<String> = extract_md_link_targets(index_content)
        .into_iter()
        .filter(|t| t.starts_with("wiki/") && !kb_root.join(t).exists())
        .collect();

    Ok(LintReport {
        orphan_pages,
        broken_links,
        missing_from_index,
        index_dead_links,
        no_frontmatter,
        ambiguous_wikilinks: vec![],
    })
}

fn analyze_obsidian(
    all_pages: Vec<String>,
    wikilinks: HashMap<String, Vec<String>>,
    no_frontmatter: Vec<String>,
    kb_root: &Path,
    index_content: &str,
) -> Result<LintReport> {
    // Build lowercase-stem → paths map for case-insensitive resolution (Obsidian behaviour)
    let mut basename_map: HashMap<String, Vec<String>> = HashMap::new();
    for page in &all_pages {
        if let Some(stem) = Path::new(page).file_stem().and_then(|s| s.to_str()) {
            basename_map.entry(stem.to_lowercase()).or_default().push(page.clone());
        }
    }

    // Inbound counts based on unambiguous resolutions
    let mut inbound: HashMap<String, usize> =
        all_pages.iter().map(|p| (p.clone(), 0)).collect();

    let mut broken_links: Vec<(String, String)> = Vec::new();
    let mut ambiguous_wikilinks: Vec<(String, String, Vec<String>)> = Vec::new();

    for (page, targets) in &wikilinks {
        for t in targets {
            let key = t.to_lowercase();
            match basename_map.get(&key) {
                None => broken_links.push((page.clone(), t.clone())),
                Some(matches) if matches.len() > 1 => {
                    ambiguous_wikilinks.push((page.clone(), t.clone(), matches.clone()));
                }
                Some(matches) => {
                    *inbound.entry(matches[0].clone()).or_default() += 1;
                }
            }
        }
    }

    let orphan_pages: Vec<String> = all_pages
        .iter()
        .filter(|p| *inbound.get(*p).unwrap_or(&0) == 0)
        .cloned()
        .collect();

    let index_lower = index_content.to_lowercase();

    // missing_from_index: check if stem appears as [[stem]] (case-insensitive) or path string
    let missing_from_index: Vec<String> = all_pages
        .iter()
        .filter(|p| {
            let stem = Path::new(p).file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();
            !index_lower.contains(&format!("[[{stem}]]"))
                && !index_content.contains(p.as_str())
        })
        .cloned()
        .collect();

    // index_dead_links: [[Name]] in index.md that resolve to nothing
    let index_dead_links: Vec<String> = extract_obsidian_wikilinks(index_content)
        .into_iter()
        .filter(|name| {
            let key = name.to_lowercase();
            basename_map.get(&key).map(|v| v.is_empty()).unwrap_or(true)
                && !kb_root.join(name).with_extension("md").exists()
        })
        .map(|name| format!("[[{name}]]"))
        .collect();

    Ok(LintReport {
        orphan_pages,
        broken_links,
        missing_from_index,
        index_dead_links,
        no_frontmatter,
        ambiguous_wikilinks,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn walk_md(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
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

/// Extract `[[wiki/...]]` wikilinks (wai style).
fn extract_wikilinks(content: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut rest = content;
    while let Some(start) = rest.find("[[") {
        rest = &rest[start + 2..];
        if let Some(end) = rest.find("]]") {
            let target = rest[..end].trim();
            if target.starts_with("wiki/") {
                links.push(target.to_string());
            }
            rest = &rest[end + 2..];
        } else {
            break;
        }
    }
    links
}

/// Extract `[[Note Name]]` wikilinks (Obsidian style, no path prefix).
fn extract_obsidian_wikilinks(content: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut rest = content;
    while let Some(start) = rest.find("[[") {
        rest = &rest[start + 2..];
        if let Some(end) = rest.find("]]") {
            let inner = rest[..end].trim();
            // Handle [[Note Name|Alias]] — take only the target part
            let target = inner.split('|').next().unwrap_or(inner).trim();
            // Only pure note names (no path separators)
            if !target.is_empty() && !target.contains('/') && !target.contains('\\') {
                links.push(target.to_string());
            }
            rest = &rest[end + 2..];
        } else {
            break;
        }
    }
    links
}

/// Extract markdown link targets `(wiki/...)` from index.md.
fn extract_md_link_targets(content: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut rest = content;
    while let Some(start) = rest.find("](") {
        rest = &rest[start + 2..];
        if let Some(end) = rest.find(')') {
            let target = rest[..end].trim();
            if target.starts_with("wiki/") && !target.contains(' ') {
                links.push(target.to_string());
            }
            rest = &rest[end + 1..];
        } else {
            break;
        }
    }
    links
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn kb(dir: &TempDir) -> PathBuf {
        dir.path().to_path_buf()
    }

    fn write(dir: &TempDir, rel: &str, content: &str) {
        let p = dir.path().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    #[test]
    fn detects_orphan() {
        let dir = TempDir::new().unwrap();
        write(&dir, "wiki/concepts/foo.md", "---\ntitle: Foo\n---\n\nContent.");
        write(&dir, "index.md", "# Index\n");
        let wiki = dir.path().join("wiki");
        let report = analyze(&wiki, &kb(&dir), &dir.path().join("index.md"), WikilinkStyle::Syn).unwrap();
        assert!(report.orphan_pages.contains(&"wiki/concepts/foo.md".to_string()));
    }

    #[test]
    fn detects_broken_wikilink() {
        let dir = TempDir::new().unwrap();
        write(&dir, "wiki/a.md", "---\ntitle: A\n---\n\nSee [[wiki/missing.md]].");
        write(&dir, "index.md", "");
        let wiki = dir.path().join("wiki");
        let report = analyze(&wiki, &kb(&dir), &dir.path().join("index.md"), WikilinkStyle::Syn).unwrap();
        assert!(report.broken_links.iter().any(|(_, t)| t == "wiki/missing.md"));
    }

    #[test]
    fn detects_page_missing_from_index() {
        let dir = TempDir::new().unwrap();
        write(&dir, "wiki/concepts/bar.md", "---\ntitle: Bar\n---\n\nHello.");
        write(&dir, "index.md", "# Index\n\n(empty)\n");
        let wiki = dir.path().join("wiki");
        let report = analyze(&wiki, &kb(&dir), &dir.path().join("index.md"), WikilinkStyle::Syn).unwrap();
        assert!(report.missing_from_index.contains(&"wiki/concepts/bar.md".to_string()));
    }

    #[test]
    fn detects_no_frontmatter() {
        let dir = TempDir::new().unwrap();
        write(&dir, "wiki/raw.md", "# Raw page\n\nNo frontmatter here.");
        write(&dir, "index.md", "");
        let wiki = dir.path().join("wiki");
        let report = analyze(&wiki, &kb(&dir), &dir.path().join("index.md"), WikilinkStyle::Syn).unwrap();
        assert!(report.no_frontmatter.contains(&"wiki/raw.md".to_string()));
    }

    #[test]
    fn clean_wiki_is_clean() {
        let dir = TempDir::new().unwrap();
        write(&dir, "wiki/a.md", "---\ntitle: A\n---\n\nLinks to [[wiki/b.md]].");
        write(&dir, "wiki/b.md", "---\ntitle: B\n---\n\nLinks to [[wiki/a.md]].");
        write(&dir, "index.md", "- [A](wiki/a.md)\n- [B](wiki/b.md)\n");
        let wiki = dir.path().join("wiki");
        let report = analyze(&wiki, &kb(&dir), &dir.path().join("index.md"), WikilinkStyle::Syn).unwrap();
        assert!(report.broken_links.is_empty());
        assert!(report.index_dead_links.is_empty());
        assert!(report.no_frontmatter.is_empty());
    }

    #[test]
    fn extract_wikilinks_finds_targets() {
        let content = "See [[wiki/foo.md]] and [[wiki/bar/baz.md]] also [[other.md]].";
        let links = extract_wikilinks(content);
        assert_eq!(links, vec!["wiki/foo.md", "wiki/bar/baz.md"]);
    }

    #[test]
    fn obsidian_detects_ambiguous_wikilink() {
        let dir = TempDir::new().unwrap();
        write(&dir, "syn/concepts/foo.md", "---\ntitle: Foo\n---\n\nSee [[Foo]].");
        write(&dir, "syn/entities/foo.md", "---\ntitle: Foo Entity\n---\n\nContent.");
        write(&dir, "index.md", "[[Foo]]\n[[Foo]]\n");
        let wiki = dir.path().join("syn");
        let report = analyze(&wiki, &kb(&dir), &dir.path().join("index.md"), WikilinkStyle::Obsidian).unwrap();
        assert!(!report.ambiguous_wikilinks.is_empty());
        assert_eq!(report.ambiguous_wikilinks[0].1, "Foo");
        assert_eq!(report.ambiguous_wikilinks[0].2.len(), 2);
    }

    #[test]
    fn obsidian_detects_broken_wikilink() {
        let dir = TempDir::new().unwrap();
        write(&dir, "syn/a.md", "---\ntitle: A\n---\n\nSee [[Missing Note]].");
        write(&dir, "index.md", "[[A]]\n");
        let wiki = dir.path().join("syn");
        let report = analyze(&wiki, &kb(&dir), &dir.path().join("index.md"), WikilinkStyle::Obsidian).unwrap();
        assert!(report.broken_links.iter().any(|(_, t)| t == "Missing Note"));
    }

    #[test]
    fn extract_obsidian_wikilinks_with_alias() {
        let content = "See [[Foo Bar|alias]] and [[Baz]] but not [[wiki/path.md]].";
        let links = extract_obsidian_wikilinks(content);
        assert_eq!(links, vec!["Foo Bar", "Baz"]);
    }
}
