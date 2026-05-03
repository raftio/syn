use iocraft::prelude::*;

pub fn print(path: &str, vault_mode: bool) {
    let mode = if vault_mode { " (Obsidian vault)" } else { "" };
    let heading = format!("✓  Knowledge base initialised{mode}");

    element! {
        View(
            border_style: BorderStyle::Round,
            border_color: Color::Green,
            flex_direction: FlexDirection::Column,
            padding_left: 2,
            padding_right: 2,
            padding_top: 1,
            padding_bottom: 1,
            margin_bottom: 1,
        ) {
            Text(content: heading, color: Color::Green, weight: Weight::Bold)
            Text(content: path.to_string(), color: Color::DarkGrey)
        }
    }
    .print();
}

pub fn print_next_steps(vault_mode: bool) {
    if vault_mode {
        element! {
            View(flex_direction: FlexDirection::Column, gap: 1) {
                Text(content: "  Wiki pages will be written to syn/")
                Text(content: "  Source files go in syn-sources/")
                Text(content: "  Wikilinks use Obsidian [[Note Name]] syntax.")
                View(margin_top: 1) {
                    Text(content: "  Edit ", color: Color::DarkGrey)
                    Text(content: "CLAUDE.md", weight: Weight::Bold)
                    Text(content: " to customise the wiki schema for your domain.", color: Color::DarkGrey)
                }
                View {
                    Text(content: "  Set ", color: Color::DarkGrey)
                    Text(content: "ANTHROPIC_API_KEY", weight: Weight::Bold)
                    Text(content: ", then run:", color: Color::DarkGrey)
                }
                View(padding_left: 4) {
                    Text(content: "syn ingest <path-to-source>", color: Color::Cyan)
                }
            }
        }
        .print();
    } else {
        element! {
            View(flex_direction: FlexDirection::Column) {
                View {
                    Text(content: "  Edit ", color: Color::DarkGrey)
                    Text(content: "CLAUDE.md", weight: Weight::Bold)
                    Text(content: " to customise the wiki schema for your domain.", color: Color::DarkGrey)
                }
                View {
                    Text(content: "  Set ", color: Color::DarkGrey)
                    Text(content: "ANTHROPIC_API_KEY", weight: Weight::Bold)
                    Text(content: ", then run:", color: Color::DarkGrey)
                }
                View(padding_left: 4, margin_top: 1) {
                    Text(content: "syn ingest <path-to-source>", color: Color::Cyan)
                }
            }
        }
        .print();
    }
}
