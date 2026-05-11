use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

use crate::errors::WaiError;

#[derive(Debug, Clone)]
pub struct Edit {
    pub op: EditOp,
    /// Relative path from KB root (e.g. "wiki/sources/article.md")
    pub path: String,
    pub content: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditOp {
    Create,
    Update,
    Append,
    Delete,
}

impl std::fmt::Display for EditOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditOp::Create => write!(f, "create"),
            EditOp::Update => write!(f, "update"),
            EditOp::Append => write!(f, "append"),
            EditOp::Delete => write!(f, "delete"),
        }
    }
}

/// Parse a `wiki_edit` tool input into an `Edit`.
pub fn parse_edit(input: &serde_json::Value) -> Result<Edit, WaiError> {
    let path = input["path"]
        .as_str()
        .ok_or_else(|| WaiError::InvalidEdit("missing 'path' field".to_string()))?
        .to_string();

    let content = input["content"].as_str().map(str::to_string);

    let op = match input["op"].as_str() {
        Some("create") => EditOp::Create,
        Some("update") => EditOp::Update,
        Some("append") => EditOp::Append,
        Some("delete") => EditOp::Delete,
        Some(other) => return Err(WaiError::InvalidEdit(format!("unknown op: {other}"))),
        // Infer op when the model omits it
        None => {
            if content.is_some() {
                EditOp::Create
            } else {
                EditOp::Delete
            }
        }
    };

    Ok(Edit { op, path, content })
}

/// Apply a list of edits to the knowledge base rooted at `kb_root`.
/// All paths are validated to stay within allowed locations.
pub fn apply_edits(edits: &[Edit], kb_root: &Path) -> Result<Vec<String>> {
    let mut applied = Vec::new();
    for edit in edits {
        apply_one(edit, kb_root).with_context(|| format!("applying edit to {}", edit.path))?;
        applied.push(format!("{} {}", edit.op, edit.path));
    }
    Ok(applied)
}

fn apply_one(edit: &Edit, kb_root: &Path) -> Result<()> {
    let abs = resolve_and_validate(&edit.path, kb_root)?;

    match &edit.op {
        EditOp::Create | EditOp::Update => {
            let content = edit
                .content
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("content required for {}", edit.op))?;
            if let Some(parent) = abs.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&abs, content)?;
        }
        EditOp::Append => {
            let content = edit
                .content
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("content required for append"))?;
            if let Some(parent) = abs.parent() {
                std::fs::create_dir_all(parent)?;
            }
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&abs)?;
            writeln!(file, "{content}")?;
        }
        EditOp::Delete => {
            if abs.exists() {
                std::fs::remove_file(&abs)?;
            }
        }
    }
    Ok(())
}

/// Resolve `rel_path` relative to `kb_root` and confirm it stays within
/// allowed locations: `wiki/`, `index.md`, `log.md`.
fn resolve_and_validate(rel_path: &str, kb_root: &Path) -> Result<PathBuf> {
    // Reject obvious traversal
    if rel_path.contains("..") {
        bail!("path traversal rejected: {rel_path}");
    }

    // Must be relative
    let rel = Path::new(rel_path);
    if rel.is_absolute() {
        bail!("absolute paths not allowed: {rel_path}");
    }

    let abs = kb_root.join(rel);

    // Verify the canonical prefix stays inside kb_root
    // (use the non-canonicalized version since the file may not exist yet)
    let normalized = normalize_path(&abs);
    let normalized_root = normalize_path(kb_root);
    if !normalized.starts_with(&normalized_root) {
        bail!("path escapes KB root: {rel_path}");
    }

    // Allow-list: must start with wiki/ or wai/ (vault mode), or be index.md / log.md
    let allowed = rel_path.starts_with("wiki/")
        || rel_path.starts_with("syn/")
        || rel_path == "index.md"
        || rel_path == "log.md";
    if !allowed {
        bail!("path not in allowed locations (wiki/, syn/, index.md, log.md): {rel_path}");
    }

    Ok(abs)
}

/// Resolve `.` and `..` components without hitting the filesystem.
fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tmp() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn rejects_path_traversal() {
        let dir = tmp();
        let edit = Edit {
            op: EditOp::Create,
            path: "../outside.md".to_string(),
            content: Some("x".to_string()),
        };
        assert!(apply_edits(&[edit], dir.path()).is_err());
    }

    #[test]
    fn rejects_absolute_path() {
        let dir = tmp();
        let edit = Edit {
            op: EditOp::Create,
            path: "/etc/passwd".to_string(),
            content: Some("x".to_string()),
        };
        assert!(apply_edits(&[edit], dir.path()).is_err());
    }

    #[test]
    fn rejects_disallowed_location() {
        let dir = tmp();
        let edit = Edit {
            op: EditOp::Create,
            path: "raw/secret.md".to_string(),
            content: Some("x".to_string()),
        };
        assert!(apply_edits(&[edit], dir.path()).is_err());
    }

    #[test]
    fn creates_wiki_file() {
        let dir = tmp();
        std::fs::create_dir_all(dir.path().join("wiki/sources")).unwrap();
        let edit = Edit {
            op: EditOp::Create,
            path: "wiki/sources/test.md".to_string(),
            content: Some("# Test\n".to_string()),
        };
        apply_edits(&[edit], dir.path()).unwrap();
        assert!(dir.path().join("wiki/sources/test.md").exists());
    }

    #[test]
    fn appends_to_log() {
        let dir = tmp();
        std::fs::write(dir.path().join("log.md"), "# Log\n").unwrap();
        let edit = Edit {
            op: EditOp::Append,
            path: "log.md".to_string(),
            content: Some("## [2026-04-23] ingest | Test".to_string()),
        };
        apply_edits(&[edit], dir.path()).unwrap();
        let content = std::fs::read_to_string(dir.path().join("log.md")).unwrap();
        assert!(content.contains("## [2026-04-23] ingest | Test"));
    }

    #[test]
    fn parse_edit_from_json() {
        let v = serde_json::json!({
            "op": "create",
            "path": "wiki/sources/foo.md",
            "content": "# Foo\n"
        });
        let edit = parse_edit(&v).unwrap();
        assert_eq!(edit.op, EditOp::Create);
        assert_eq!(edit.path, "wiki/sources/foo.md");
    }
}
