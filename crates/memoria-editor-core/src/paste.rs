//! Clipboard paste: plain text / markdown pass through as-is, HTML converts
//! via `html2md`. Matches Memoria's paste surface (text/plain + text/html).

use crate::cursor::Selection;
use crate::editor::{Editor, Tx};
use crate::history::EditKind;
use crate::html2md::html_to_markdown;

impl Editor {
    /// Paste clipboard content. `html` wins when present (browser clips carry
    /// both); `plain` is inserted verbatim (it already is markdown).
    /// Returns the markdown that was inserted.
    pub fn paste(&mut self, plain: Option<&str>, html: Option<&str>) -> Option<String> {
        let md = match (html, plain) {
            (Some(h), _) if looks_like_html(h) => html_to_markdown(h),
            (_, Some(p)) => p.to_string(),
            (Some(h), None) => h.to_string(),
            (None, None) => return None,
        };
        if md.is_empty() {
            return Some(md);
        }
        let r = self.sel.range();
        let mut tx = Tx::new();
        tx.replace(r.start, r.end, &md);
        tx.selection(Selection::caret(r.start + md.len()));
        self.apply(tx, EditKind::Paste);
        Some(md)
    }
}

/// Cheap heuristic: contains a tag that isn't a lone `<` in prose.
fn looks_like_html(s: &str) -> bool {
    s.contains('<') && s.contains('>') && {
        let t = s.trim_start();
        t.starts_with('<') || s.contains("</") || s.contains("<br") || s.contains("<p")
    }
}
