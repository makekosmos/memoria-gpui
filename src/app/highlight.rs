//! `v-memo` search highlight — shared span splitter for overlay results.
use gpui::{div, prelude::*, px, IntoElement, SharedString};

use memoria_gpui::search_model::highlight_ranges;

use crate::theme::*;

/// Highlighted snippet with the query match rendered in accent.
pub(crate) fn highlighted(text: &str, query: &str) -> impl IntoElement {
    let ranges = highlight_ranges(text, query);
    let byte_idx = |char_pos: usize| -> usize {
        text.char_indices()
            .nth(char_pos)
            .map(|(i, _)| i)
            .unwrap_or(text.len())
    };
    let total_chars = text.chars().count();
    let mut spans: Vec<(&str, bool)> = Vec::new();
    let mut cursor = 0usize;
    for (s, e) in ranges {
        let e = e.min(total_chars);
        if s > cursor {
            spans.push((&text[byte_idx(cursor)..byte_idx(s)], false));
        }
        if e > s {
            spans.push((&text[byte_idx(s)..byte_idx(e)], true));
        }
        cursor = e.max(s);
    }
    if cursor < total_chars {
        spans.push((&text[byte_idx(cursor)..], false));
    }
    let mut row = div().flex().flex_wrap().text_size(px(12.));
    for (part, hit) in spans {
        if part.is_empty() {
            continue;
        }
        row = row.child(
            div()
                .text_color(if hit { c(ACCENT()) } else { c(MUTED_FG()) })
                .when(hit, |d| d.font_weight(gpui::FontWeight::SEMIBOLD))
                .child(SharedString::from(part.to_string())),
        );
    }
    row
}
