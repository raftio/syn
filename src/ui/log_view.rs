use iocraft::prelude::*;

struct LogEntry<'a> {
    date: &'a str,
    op: &'a str,
    title: &'a str,
}

fn op_color(op: &str) -> Color {
    match op {
        "ingest" => Color::Blue,
        "query" => Color::Magenta,
        "lint" => Color::Yellow,
        _ => Color::DarkGrey,
    }
}

pub fn print(raw: &str) {
    let entries: Vec<LogEntry<'_>> = raw
        .lines()
        .filter(|l| l.starts_with("## ["))
        .filter_map(|l| {
            // Format: ## [2026-01-15] op | title
            let inner = l.trim_start_matches("## [");
            let (date, rest) = inner.split_once(']')?;
            let rest = rest.trim().trim_start_matches('|').trim();
            let (op, title) = rest.split_once('|')?;
            Some(LogEntry {
                date: date.trim(),
                op: op.trim(),
                title: title.trim(),
            })
        })
        .collect();

    if entries.is_empty() {
        element! {
            Text(content: "No log entries yet. Run `syn ingest` to get started.", color: Color::DarkGrey)
        }
        .eprint();
        return;
    }

    for entry in &entries {
        let color = op_color(entry.op);
        let op_badge = format!("[{}]", entry.op);

        element! {
            View(flex_direction: FlexDirection::Row, gap: 2, margin_bottom: 0) {
                Text(content: entry.date.to_string(), color: Color::DarkGrey)
                Text(content: op_badge, color: color, weight: Weight::Bold)
                Text(content: entry.title.to_string())
            }
        }
        .print();
    }
}
