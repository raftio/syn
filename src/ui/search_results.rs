use iocraft::prelude::*;

use crate::search::SearchResult;

pub fn print(results: &[SearchResult]) {
    for (i, r) in results.iter().enumerate() {
        let score_color = if r.score >= 1.0 {
            Color::Green
        } else if r.score >= 0.5 {
            Color::Yellow
        } else {
            Color::DarkGrey
        };

        let rank = format!("{}", i + 1);
        let score = format!("{:.2}", r.score);

        element! {
            View(flex_direction: FlexDirection::Column, margin_bottom: 1) {
                View(flex_direction: FlexDirection::Row, gap: 2) {
                    Text(content: rank, color: Color::DarkGrey)
                    Text(content: r.title.clone(), weight: Weight::Bold)
                    Text(content: score, color: score_color)
                }
                View(padding_left: 3) {
                    Text(content: r.path.clone(), color: Color::DarkGrey, italic: true)
                }
                View(padding_left: 3) {
                    Text(content: r.snippet.clone(), color: Color::Grey)
                }
            }
        }
        .print();
    }
}
