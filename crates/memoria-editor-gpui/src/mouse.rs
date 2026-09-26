//! Mouse: click → caret (through hidden markup — the hit lands on the
//! projection and `to_source` maps back to the markdown source), drag →
//! range selection, double-click → word, triple-click → block (Vue/Tiptap
//! paragraph granularity). Shift-click extends from the anchor.

use gpui::{Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Window};
use memoria_editor_core::Selection;
use unicode_segmentation::UnicodeSegmentation;

use crate::editor::MemoriaEditor;

impl MemoriaEditor {
    pub(crate) fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left {
            return;
        }
        window.focus(&self.focus, cx);
        window.prevent_default();
        self.picker = None;
        let Some(vis) = self.vis_index_for_point(event.position, window, cx) else {
            return;
        };
        let src = self
            .proj
            .as_ref()
            .map(|p| p.to_source(vis))
            .unwrap_or_default();
        match event.click_count {
            2 => {
                let (a, b) = word_at(&self.core.text(), src);
                self.core.set_selection(Selection::new(a, b));
                self.drag_anchor = None;
            }
            3 => self.select_block_at(src),
            _ => {
                let sel = if event.modifiers.shift {
                    Selection::new(self.core.selection().anchor, src)
                } else {
                    Selection::caret(src)
                };
                self.core.set_selection(sel);
                self.drag_anchor = Some(sel.anchor);
            }
        }
        self.cursor_visible = true;
        self.follow_caret = true;
        cx.emit(crate::editor::EditorEvent::SelectionChanged);
        cx.notify();
    }

    pub(crate) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !event.pressed_button.is_some_and(|b| b == MouseButton::Left) {
            return;
        }
        let Some(anchor) = self.drag_anchor else {
            return;
        };
        let Some(vis) = self.vis_index_for_point(event.position, window, cx) else {
            return;
        };
        let src = self
            .proj
            .as_ref()
            .map(|p| p.to_source(vis))
            .unwrap_or(anchor);
        self.core.set_selection(Selection::new(anchor, src));
        self.cursor_visible = true;
        cx.emit(crate::editor::EditorEvent::SelectionChanged);
        cx.notify();
    }

    pub(crate) fn on_mouse_up(
        &mut self,
        _event: &MouseUpEvent,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        self.drag_anchor = None;
    }

    /// Triple-click → select the whole top-level block (paragraph etc.).
    fn select_block_at(&mut self, src: usize) {
        self.ensure_rows();
        let vis = self
            .proj
            .as_ref()
            .map(|p| p.to_visible(src))
            .unwrap_or_default();
        let Some(i) = self.row_at_vis(vis) else {
            return;
        };
        let block = self.rows[i].block_ix;
        let (mut a, mut b) = (usize::MAX, 0usize);
        for r in self.rows.iter().filter(|r| r.block_ix == block) {
            for s in &r.spans {
                a = a.min(s.src.start);
                b = b.max(s.src.end);
            }
        }
        if a > b {
            a = src;
            b = src;
        }
        self.core.set_selection(Selection::new(a, b));
        self.drag_anchor = None;
    }
}

/// Word boundaries around `pos` in the markdown source (grapheme-safe).
fn word_at(src: &str, pos: usize) -> (usize, usize) {
    let pos = pos.min(src.len());
    for (start, word) in src.split_word_bound_indices() {
        if pos >= start && pos < start + word.len() {
            return (start, start + word.len());
        }
    }
    (pos, pos)
}
