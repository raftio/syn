use crate::errors::WaiError;
use std::path::{Path, PathBuf};

/// Walk up from `start` looking for a `.syn/config.toml` marker file.
/// Returns the KB root directory if found.
pub fn find_kb_root(start: &Path) -> Result<PathBuf, WaiError> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(".syn").join("config.toml").exists() {
            return Ok(current);
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return Err(WaiError::NotAKnowledgeBase),
        }
    }
}

/// Resolve the KB root starting from the current working directory.
pub fn resolve_kb_root() -> Result<PathBuf, WaiError> {
    let cwd = std::env::current_dir().map_err(WaiError::Io)?;
    find_kb_root(&cwd)
}
