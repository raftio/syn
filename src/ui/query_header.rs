use iocraft::prelude::*;

pub fn print(n_pages: usize, model: &str) {
    let pages_label = format!(
        "Using {} relevant page{}",
        n_pages,
        if n_pages == 1 { "" } else { "s" }
    );
    let model_label = format!("[{model}]");

    element! {
        View(flex_direction: FlexDirection::Row, gap: 2, margin_bottom: 1) {
            Text(content: pages_label, color: Color::Cyan)
            Text(content: model_label, color: Color::DarkGrey)
        }
    }
    .eprint();
}
