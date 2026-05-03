use anyhow::{Context, Result};

use crate::sources::Source;

pub async fn load(url: &str) -> Result<Source> {
    let resp = reqwest::get(url)
        .await
        .with_context(|| format!("fetching {url}"))?;

    if !resp.status().is_success() {
        anyhow::bail!("HTTP {} fetching {url}", resp.status());
    }

    let final_url = resp.url().to_string();
    let html = resp.text().await.context("reading response body")?;

    let title = extract_html_title(&html).unwrap_or_else(|| "Untitled".to_string());
    let body = htmd::convert(&html).unwrap_or_else(|_| strip_tags(&html));
    let slug = slug_from_url(&final_url)
        .unwrap_or_else(|| crate::sources::to_slug(&title));

    Ok(Source { title, slug, body })
}

fn extract_html_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title>")? + "<title>".len();
    let end = lower.find("</title>")?;
    if start < end {
        Some(
            html[start..end]
                .trim()
                .replace("&amp;", "&")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&#39;", "'"),
        )
    } else {
        None
    }
}

/// Take the last non-empty path segment of the URL as the slug.
fn slug_from_url(url: &str) -> Option<String> {
    let path = url.split('?').next().unwrap_or(url); // strip query string
    let segment = path
        .trim_end_matches('/')
        .rsplit('/')
        .find(|s| !s.is_empty())?;

    // Skip if it looks like a domain (no dashes/underscores and short)
    if !segment.contains('-') && !segment.contains('_') && segment.len() < 4 {
        return None;
    }

    let slug = crate::sources::to_slug(segment);
    if slug.is_empty() { None } else { Some(slug) }
}

/// Fallback: strip HTML tags and return plain text.
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_title_from_html() {
        let html = "<html><head><title>My Blog Post</title></head><body></body></html>";
        assert_eq!(extract_html_title(html), Some("My Blog Post".to_string()));
    }

    #[test]
    fn returns_none_when_no_title() {
        assert_eq!(extract_html_title("<html><body>hi</body></html>"), None);
    }

    #[test]
    fn slug_from_url_last_segment() {
        assert_eq!(
            slug_from_url("https://example.com/blog/my-cool-post"),
            Some("my-cool-post".to_string())
        );
        assert_eq!(
            slug_from_url("https://example.com/blog/my-cool-post/"),
            Some("my-cool-post".to_string())
        );
    }

    #[test]
    fn strip_tags_removes_html() {
        assert_eq!(strip_tags("<p>Hello <b>world</b></p>"), "Hello world");
    }
}
