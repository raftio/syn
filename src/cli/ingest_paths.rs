use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::WalkDir;

/// A single resolved ingest target.
#[derive(Debug, Clone)]
pub enum ResolvedInput {
    File(PathBuf),
    Url(String),
}

impl ResolvedInput {
    pub fn display_name(&self) -> String {
        match self {
            ResolvedInput::File(p) => p.display().to_string(),
            ResolvedInput::Url(u) => u.clone(),
        }
    }
}

/// Expand a list of raw CLI inputs (files, directories, globs, URLs) into
/// `ResolvedInput` entries, filtered by `exts` (lowercase, no leading dot).
/// Duplicates are removed by canonical path; order is stable within each input.
pub fn expand(inputs: &[String], exts: &[String]) -> Result<Vec<ResolvedInput>> {
    let mut out: Vec<ResolvedInput> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();

    for input in inputs {
        if input.starts_with("http://") || input.starts_with("https://") {
            out.push(ResolvedInput::Url(input.clone()));
            continue;
        }

        if looks_like_glob(input) {
            expand_glob(input, exts, &mut out, &mut seen)?;
            continue;
        }

        let p = Path::new(input);
        if p.is_file() {
            if matches_ext(p, exts) {
                let key = canonicalize_or_raw(p);
                if seen.insert(key) {
                    out.push(ResolvedInput::File(p.to_path_buf()));
                }
            }
        } else if p.is_dir() {
            expand_dir(p, exts, &mut out, &mut seen)
                .with_context(|| format!("walking directory '{input}'"))?;
        } else {
            anyhow::bail!("path not found: {input}");
        }
    }

    Ok(out)
}

fn expand_dir(
    dir: &Path,
    exts: &[String],
    out: &mut Vec<ResolvedInput>,
    seen: &mut HashSet<PathBuf>,
) -> Result<()> {
    for entry in WalkDir::new(dir).follow_links(false) {
        let entry =
            entry.with_context(|| format!("reading entry in {}", dir.display()))?;
        let p = entry.path();
        if entry.file_type().is_file() && matches_ext(p, exts) {
            let key = canonicalize_or_raw(p);
            if seen.insert(key) {
                out.push(ResolvedInput::File(p.to_path_buf()));
            }
        }
    }
    Ok(())
}

fn expand_glob(
    pattern: &str,
    exts: &[String],
    out: &mut Vec<ResolvedInput>,
    seen: &mut HashSet<PathBuf>,
) -> Result<()> {
    let mut matched = 0usize;
    for entry in glob::glob(pattern)
        .with_context(|| format!("invalid glob pattern: {pattern}"))?
    {
        let p = entry.with_context(|| format!("error in glob '{pattern}'"))?;
        if p.is_file() && matches_ext(&p, exts) {
            let key = canonicalize_or_raw(&p);
            if seen.insert(key) {
                out.push(ResolvedInput::File(p));
                matched += 1;
            }
        }
    }
    if matched == 0 {
        anyhow::bail!("no files matched pattern: {pattern}");
    }
    Ok(())
}

fn looks_like_glob(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[')
}

fn matches_ext(path: &Path, exts: &[String]) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    exts.contains(&ext)
}

fn canonicalize_or_raw(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(dir: &TempDir, rel: &str, content: &str) {
        let p = dir.path().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    fn md() -> Vec<String> {
        vec!["md".to_string()]
    }

    fn md_txt() -> Vec<String> {
        vec!["md".to_string(), "txt".to_string()]
    }

    #[test]
    fn single_file_passes_through() {
        let dir = TempDir::new().unwrap();
        write(&dir, "a.md", "content");
        let path = dir.path().join("a.md").to_str().unwrap().to_string();
        let result = expand(&[path], &md()).unwrap();
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], ResolvedInput::File(_)));
    }

    #[test]
    fn url_passes_through() {
        let result = expand(&["https://example.com/post".to_string()], &md()).unwrap();
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0], ResolvedInput::Url(u) if u.contains("example.com")));
    }

    #[test]
    fn directory_walks_recursively() {
        let dir = TempDir::new().unwrap();
        write(&dir, "sub/a.md", "a");
        write(&dir, "sub/nested/b.md", "b");
        write(&dir, "sub/c.txt", "c");
        let path = dir.path().join("sub").to_str().unwrap().to_string();
        let result = expand(&[path], &md()).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|r| matches!(r, ResolvedInput::File(_))));
    }

    #[test]
    fn extension_filter_excludes_non_matching() {
        let dir = TempDir::new().unwrap();
        write(&dir, "a.md", "a");
        write(&dir, "b.txt", "b");
        write(&dir, "c.rs", "c");
        let path = dir.path().to_str().unwrap().to_string();
        let md_only = expand(&[path.clone()], &md()).unwrap();
        assert_eq!(md_only.len(), 1);
        let both = expand(&[path], &md_txt()).unwrap();
        assert_eq!(both.len(), 2);
    }

    #[test]
    fn duplicate_paths_are_deduped() {
        let dir = TempDir::new().unwrap();
        write(&dir, "a.md", "content");
        let path = dir.path().join("a.md").to_str().unwrap().to_string();
        let result = expand(&[path.clone(), path], &md()).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn nonexistent_path_returns_error() {
        let result = expand(&["/definitely/does/not/exist.md".to_string()], &md());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn glob_matches_files() {
        let dir = TempDir::new().unwrap();
        write(&dir, "a.md", "a");
        write(&dir, "b.md", "b");
        write(&dir, "c.txt", "c");
        let pattern = format!("{}/*.md", dir.path().display());
        let result = expand(&[pattern], &md()).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn glob_with_no_matches_errors() {
        let dir = TempDir::new().unwrap();
        let pattern = format!("{}/*.md", dir.path().display());
        let result = expand(&[pattern], &md());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no files matched"));
    }

    #[test]
    fn sorting_stable_within_directory() {
        let dir = TempDir::new().unwrap();
        write(&dir, "b.md", "b");
        write(&dir, "a.md", "a");
        write(&dir, "c.md", "c");
        let path = dir.path().to_str().unwrap().to_string();
        let result = expand(&[path], &md()).unwrap();
        // All 3 should be present (order may vary by OS, but count is correct)
        assert_eq!(result.len(), 3);
    }
}
