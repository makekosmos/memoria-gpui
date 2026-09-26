//! IME contract shaped like `gpui::EntityInputHandler` — text-only subset
//! (bounds/point queries belong to the UI layer, M3). All public offsets here
//! are **UTF-16 code units**, matching GPUI's `EntityInputHandler` exactly.

use std::ops::Range;

use crate::cursor::Selection;
use crate::editor::{Editor, Tx};
use crate::history::EditKind;

impl Editor {
    /// `EntityInputHandler::text_for_range` — source text in the UTF-16 range
    /// plus the same range clamped to the document bounds.
    pub fn text_for_range(&mut self, range_utf16: Range<usize>) -> Option<(String, Range<usize>)> {
        let len16 = self.buf.byte_to_utf16(self.buf.len_bytes());
        if range_utf16.start > len16 {
            return None;
        }
        let clamped = range_utf16.start..range_utf16.end.min(len16);
        let s = self.buf.utf16_to_byte(clamped.start);
        let e = self.buf.utf16_to_byte(clamped.end);
        Some((self.buf.slice(s, e), clamped))
    }

    /// `EntityInputHandler::selected_text_range` — selection in UTF-16.
    pub fn selected_text_range_utf16(&self) -> Range<usize> {
        let s = self.buf.byte_to_utf16(self.sel.start());
        let e = self.buf.byte_to_utf16(self.sel.end());
        s..e
    }

    /// `EntityInputHandler::marked_text_range` — preedit range in UTF-16.
    pub fn marked_text_range_utf16(&self) -> Option<Range<usize>> {
        self.marked_range()
            .map(|r| self.buf.byte_to_utf16(r.start)..self.buf.byte_to_utf16(r.end))
    }

    /// `EntityInputHandler::unmark_text` — commit the preedit as plain text.
    pub fn unmark_text(&mut self) {
        self.set_marked(None);
    }

    /// `EntityInputHandler::replace_text_in_range` — replace the UTF-16 range
    /// (None = current selection) with `text`, clearing any mark.
    pub fn replace_text_in_range(&mut self, range_utf16: Option<Range<usize>>, text: &str) {
        let r = self.byte_range(range_utf16);
        let mut tx = Tx::new();
        tx.replace(r.start, r.end, text);
        tx.selection(Selection::caret(r.start + text.len()));
        self.apply(tx, EditKind::Insert);
    }

    /// `EntityInputHandler::replace_and_mark_text_in_range` — replace with
    /// `new_text` and mark `new_selected_range_utf16` (relative to the start
    /// of `new_text`, in UTF-16 units).
    pub fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
    ) {
        let r = self.byte_range(range_utf16);
        let mut tx = Tx::new();
        tx.replace(r.start, r.end, new_text);
        tx.selection(Selection::caret(r.start + new_text.len()));
        self.apply(tx, EditKind::Ime);
        if let Some(mark16) = new_selected_range_utf16 {
            let ms = r.start + utf16_prefix_bytes(new_text, mark16.start);
            let me = r.start + utf16_prefix_bytes(new_text, mark16.end);
            self.set_marked(Some(ms..me));
        } else {
            self.set_marked(None);
        }
    }

    /// Convenience: replace the current selection with marked (preedit) text.
    pub fn set_marked_text(&mut self, text: &str) {
        let r = self.selection().range();
        let r16 = self.buf.byte_to_utf16(r.start)..self.buf.byte_to_utf16(r.end);
        self.replace_and_mark_text_in_range(Some(r16), text, Some(0..text.encode_utf16().count()));
    }

    /// Current marked range in source bytes (for renderers).
    pub fn marked_source_range(&self) -> Option<Range<usize>> {
        self.marked_range()
    }

    fn byte_range(&self, utf16: Option<Range<usize>>) -> Range<usize> {
        match utf16 {
            Some(r) => self.buf.utf16_to_byte(r.start)..self.buf.utf16_to_byte(r.end),
            // No range: a live preedit is the replacement target (GPUI
            // semantics for `replace_and_mark_text_in_range`).
            None => self.marked.clone().unwrap_or_else(|| self.sel.range()),
        }
    }
}

/// Byte length of the `text` prefix covering `units` UTF-16 code units.
fn utf16_prefix_bytes(text: &str, units: usize) -> usize {
    let mut acc = 0usize;
    for (i, c) in text.char_indices() {
        if acc >= units {
            return i;
        }
        acc += c.len_utf16();
    }
    text.len()
}
