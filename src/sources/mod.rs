pub mod url;

use anyhow::{Context, Result};
use std::path::Path;

pub struct Source {
    pub title: String,
    pub slug: String,
    pub body: String,
}

pub fn load(path: &Path) -> Result<Source> {
    let body = std::fs::read_to_string(path)
        .with_context(|| format!("reading source file {}", path.display()))?;

    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");

    let title = extract_title(&body).unwrap_or_else(|| stem.to_string());
    let slug = to_slug(stem);

    Ok(Source { title, slug, body })
}

fn extract_title(md: &str) -> Option<String> {
    md.lines()
        .find(|l| l.starts_with("# "))
        .map(|l| l.trim_start_matches("# ").trim().to_string())
}

pub fn to_slug(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_h1_title() {
        assert_eq!(
            extract_title("# My Article\n\nContent here."),
            Some("My Article".to_string())
        );
    }

    #[test]
    fn slug_from_filename() {
        assert_eq!(to_slug("My Cool Article"), "my-cool-article");
        assert_eq!(to_slug("article_2026"), "article-2026");
        assert_eq!(to_slug("foo--bar"), "foo-bar");
    }
}
