use std::io::IsTerminal;

use iocraft::prelude::*;

use crate::wiki::edits::{Edit, EditOp};

/// Print the per-source planning header.
///
/// Multi-file mode: writes without a trailing newline so subsequent calls
/// overwrite the same terminal line via `\r\x1b[2K`. The caller is
/// responsible for calling `end_progress()` after the loop.
///
/// Single-file mode: uses iocraft for a styled one-shot print with newline.
pub fn print_source_header(n: usize, total: usize, title: &str, model: &str) {
    if total > 1 {
        if std::io::stderr().is_terminal() {
            // Overwrite current line in-place: CR + erase line + new content (no trailing newline)
            eprint!(
                "\r\x1b[2K\x1b[90m[{n}/{total}]\x1b[0m  Planning:  \x1b[1m{title}\x1b[0m  \x1b[90m[{model}]\x1b[0m"
            );
        } else {
            eprintln!("[{n}/{total}] Planning: {title} [{model}]");
        }
    } else {
        let model_label = format!("[{model}]");
        element! {
            View(flex_direction: FlexDirection::Row, gap: 2) {
                Text(content: "Ingesting:", color: Color::DarkGrey)
                Text(content: title.to_string(), weight: Weight::Bold)
                Text(content: model_label, color: Color::DarkGrey)
            }
        }
        .eprint();
    }
}

/// Terminate the in-place progress line after a multi-file ingest loop.
pub fn end_progress() {
    eprintln!();
}

pub fn print_edit_plan(all_plans: &[(String, Vec<Edit>)], multi: bool) {
    let total: usize = all_plans.iter().map(|(_, e)| e.len()).sum();
    if total == 0 {
        return;
    }

    let header = format!("Planned edits ({total} total):");
    element! {
        View(margin_top: 1) {
            Text(content: header, color: Color::White, weight: Weight::Bold)
        }
    }
    .eprint();

    for (src, edits) in all_plans {
        if edits.is_empty() {
            continue;
        }

        if multi {
            element! {
                View(padding_left: 2, margin_top: 1) {
                    Text(content: format!("[{src}]"), color: Color::DarkGrey, italic: true)
                }
            }
            .eprint();
        }

        for edit in edits {
            let (symbol, color) = edit_style(&edit.op);
            let indent = if multi { 4usize } else { 2usize };
            let path = edit.path.clone();

            element! {
                View(padding_left: indent as u32, flex_direction: FlexDirection::Row, gap: 1) {
                    Text(content: symbol.to_string(), color: color, weight: Weight::Bold)
                    Text(content: path)
                    Text(content: format!("({})", edit.op), color: Color::DarkGrey)
                }
            }
            .eprint();
        }
    }
}

pub fn print_applied(applied: &[String]) {
    let header = format!("\nApplied {} edit(s):", applied.len());
    element! {
        Text(content: header, color: Color::Green, weight: Weight::Bold)
    }
    .eprint();

    for path in applied {
        element! {
            View(padding_left: 2, flex_direction: FlexDirection::Row, gap: 1) {
                Text(content: "✓".to_string(), color: Color::Green, weight: Weight::Bold)
                Text(content: path.clone())
            }
        }
        .eprint();
    }
}

pub fn print_failures(failures: &[(&str, &str, &str)]) {
    if failures.is_empty() {
        return;
    }

    let header = format!("\n{} failure(s):", failures.len());
    element! {
        Text(content: header, color: Color::Red, weight: Weight::Bold)
    }
    .eprint();

    for (stage, input, error) in failures {
        element! {
            View(padding_left: 2, flex_direction: FlexDirection::Row, gap: 1) {
                Text(content: format!("[{stage}]"), color: Color::Red)
                Text(content: input.to_string(), weight: Weight::Bold)
                Text(content: "—".to_string(), color: Color::DarkGrey)
                Text(content: error.to_string(), color: Color::DarkGrey)
            }
        }
        .eprint();
    }
}

fn edit_style(op: &EditOp) -> (&'static str, Color) {
    match op {
        EditOp::Create => ("+", Color::Green),
        EditOp::Update => ("~", Color::Yellow),
        EditOp::Append => ("»", Color::Cyan),
        EditOp::Delete => ("-", Color::Red),
    }
}
