use std::path::Path;

/// Return the last `n` log entries (blocks starting with `## [`).
pub fn read_recent(log_path: &Path, n: usize) -> String {
    let content = std::fs::read_to_string(log_path).unwrap_or_default();
    let lines: Vec<&str> = content.lines().collect();

    let header_positions: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("## ["))
        .map(|(i, _)| i)
        .collect();

    if header_positions.is_empty() {
        return String::new();
    }

    let start = header_positions[header_positions.len().saturating_sub(n)];
    lines[start..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn reads_last_two_entries() {
        let dir = TempDir::new().unwrap();
        let log = dir.path().join("log.md");
        std::fs::write(
            &log,
            "# Log\n\n## [2026-01-01] ingest | A\n\ncontent a\n\n## [2026-01-02] ingest | B\n\ncontent b\n",
        )
        .unwrap();
        let recent = read_recent(&log, 1);
        assert!(recent.contains("ingest | B"));
        assert!(!recent.contains("ingest | A"));
    }

    #[test]
    fn returns_empty_when_no_entries() {
        let dir = TempDir::new().unwrap();
        let log = dir.path().join("log.md");
        std::fs::write(&log, "# Log\n").unwrap();
        assert_eq!(read_recent(&log, 5), "");
    }

    #[test]
    fn handles_missing_file() {
        let dir = TempDir::new().unwrap();
        let log = dir.path().join("missing.md");
        assert_eq!(read_recent(&log, 5), "");
    }
}
