use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

const K1: f64 = 1.2;
const B: f64 = 0.75;

const STOPWORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
    "from", "is", "was", "are", "were", "be", "been", "has", "have", "had", "do", "does", "did",
    "will", "would", "could", "should", "may", "might", "this", "that", "these", "those", "it",
    "its", "as", "not", "no", "so", "if", "then", "than", "when", "where", "who", "which",
    "what", "how", "all", "each", "also", "into", "can", "just", "about",
];

pub struct SearchResult {
    pub path: String,
    pub title: String,
    pub score: f64,
    pub snippet: String,
}

struct Doc {
    path: String,
    title: String,
    tf: HashMap<String, f64>,
    len: usize,
    snippet: String,
}

pub struct BM25Index {
    docs: Vec<Doc>,
    df: HashMap<String, usize>,
    avgdl: f64,
}

impl BM25Index {
    pub fn build(wiki_dir: &Path) -> Result<Self> {
        let mut paths: Vec<PathBuf> = Vec::new();
        if wiki_dir.exists() {
            walk_markdown(wiki_dir, &mut paths)?;
        }

        let mut docs: Vec<Doc> = Vec::new();
        for path in &paths {
            let content = std::fs::read_to_string(path)?;
            let (body, fm) = strip_frontmatter(&content);

            let title = fm
                .as_deref()
                .and_then(title_from_frontmatter)
                .or_else(|| extract_h1(body))
                .unwrap_or_else(|| {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("untitled")
                        .to_string()
                });

            let tokens = tokenize(body);
            let len = tokens.len();
            let tf = term_frequencies(&tokens);
            let snippet = make_snippet(body, 180);

            // path relative to wiki_dir's parent (kb root), e.g. "wiki/sources/foo.md"
            let rel = path
                .strip_prefix(wiki_dir.parent().unwrap_or(wiki_dir))
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();

            docs.push(Doc { path: rel, title, tf, len, snippet });
        }

        let mut df: HashMap<String, usize> = HashMap::new();
        for doc in &docs {
            for term in doc.tf.keys() {
                *df.entry(term.clone()).or_default() += 1;
            }
        }

        let avgdl = if docs.is_empty() {
            1.0
        } else {
            docs.iter().map(|d| d.len as f64).sum::<f64>() / docs.len() as f64
        };

        Ok(Self { docs, df, avgdl })
    }

    pub fn search(&self, query: &str, top_k: usize) -> Vec<SearchResult> {
        let q_tokens = tokenize(query);
        if q_tokens.is_empty() {
            return Vec::new();
        }

        let n = self.docs.len() as f64;

        let mut scores: Vec<(usize, f64)> = self
            .docs
            .iter()
            .enumerate()
            .map(|(i, doc)| {
                let score: f64 = q_tokens
                    .iter()
                    .map(|t| {
                        let df = *self.df.get(t).unwrap_or(&0) as f64;
                        let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
                        let tf = *doc.tf.get(t).unwrap_or(&0.0);
                        let dl = doc.len as f64;
                        let tf_norm = tf * (K1 + 1.0)
                            / (tf + K1 * (1.0 - B + B * dl / self.avgdl));
                        idf * tf_norm
                    })
                    .sum();
                (i, score)
            })
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scores
            .into_iter()
            .filter(|(_, s)| *s > 0.0)
            .take(top_k)
            .map(|(i, score)| {
                let doc = &self.docs[i];
                SearchResult {
                    path: doc.path.clone(),
                    title: doc.title.clone(),
                    score,
                    snippet: doc.snippet.clone(),
                }
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    /// Read the full content of a page by its relative path (from kb root).
    pub fn page_content(kb_root: &Path, rel_path: &str) -> Option<String> {
        std::fs::read_to_string(kb_root.join(rel_path)).ok()
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn walk_markdown(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_markdown(&path, out)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
            // Skip .gitkeep-style empty marker files
            if path.file_name().and_then(|s| s.to_str()) != Some(".gitkeep") {
                out.push(path);
            }
        }
    }
    Ok(())
}

pub fn strip_frontmatter_pub(content: &str) -> (&str, Option<String>) {
    strip_frontmatter(content)
}

fn strip_frontmatter(content: &str) -> (&str, Option<String>) {
    if !content.starts_with("---") {
        return (content, None);
    }
    let rest = content[3..].trim_start_matches('\r').trim_start_matches('\n');
    if let Some(end) = rest.find("\n---") {
        let fm = rest[..end].to_string();
        let body = rest[end + 4..].trim_start_matches('\n');
        (body, Some(fm))
    } else {
        (content, None)
    }
}

fn title_from_frontmatter(fm: &str) -> Option<String> {
    fm.lines()
        .find(|l| l.trim_start().starts_with("title:"))
        .map(|l| {
            l.splitn(2, ':')
                .nth(1)
                .unwrap_or("")
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string()
        })
        .filter(|s| !s.is_empty())
}

fn extract_h1(text: &str) -> Option<String> {
    text.lines()
        .find(|l| l.starts_with("# "))
        .map(|l| l[2..].trim().to_string())
}

fn tokenize(text: &str) -> Vec<String> {
    let stopwords: std::collections::HashSet<&str> = STOPWORDS.iter().copied().collect();
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() > 2)
        .map(|t| t.to_lowercase())
        .filter(|t| !stopwords.contains(t.as_str()))
        .collect()
}

fn term_frequencies(tokens: &[String]) -> HashMap<String, f64> {
    let mut tf: HashMap<String, f64> = HashMap::new();
    for t in tokens {
        *tf.entry(t.clone()).or_default() += 1.0;
    }
    tf
}

fn make_snippet(text: &str, max_chars: usize) -> String {
    let clean: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if clean.len() <= max_chars {
        clean
    } else {
        format!("{}…", &clean[..max_chars])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_page(dir: &Path, name: &str, content: &str) {
        std::fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn finds_relevant_doc() {
        let dir = TempDir::new().unwrap();
        write_page(dir.path(), "a.md", "# Rust Programming\n\nRust is a systems language focused on safety.");
        write_page(dir.path(), "b.md", "# Python Basics\n\nPython is great for scripting.");

        let idx = BM25Index::build(dir.path()).unwrap();
        let results = idx.search("rust systems safety", 5);
        assert!(!results.is_empty());
        assert!(results[0].title.contains("Rust"));
    }

    #[test]
    fn empty_wiki_returns_no_results() {
        let dir = TempDir::new().unwrap();
        let idx = BM25Index::build(dir.path()).unwrap();
        assert!(idx.search("anything", 5).is_empty());
    }

    #[test]
    fn strips_frontmatter() {
        let content = "---\ntitle: \"My Page\"\n---\n\nBody text here.";
        let (body, fm) = strip_frontmatter(content);
        assert_eq!(body, "Body text here.");
        assert!(fm.is_some());
        assert_eq!(title_from_frontmatter(fm.as_deref().unwrap()), Some("My Page".to_string()));
    }

    #[test]
    fn no_frontmatter_passthrough() {
        let content = "# Hello\n\nJust content.";
        let (body, fm) = strip_frontmatter(content);
        assert_eq!(body, content);
        assert!(fm.is_none());
    }
}
