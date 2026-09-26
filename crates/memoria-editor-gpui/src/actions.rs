//! Action handlers — cursor motion in **visible** space (so hidden markup
//! doesn't make arrows feel wrong: `vis ± k` → `to_source`), word jumps in
//! visible text, vertical motion via shaped-row hit-testing, and the edit /
//! mark / history / zoom commands wired to the Vue hotkey table.
//!
//! Selection semantics follow ProseMirror: a non-caret + arrow collapses to
//! the near end; `shift` extends from the anchor.

use gpui::{Context, Window};
use memoria_editor_core::{cmd::Command, Selection};
use unicode_segmentation::UnicodeSegmentation;

use crate::editor::MemoriaEditor;
use crate::layout;

/// Collapse-or-move result: where the caret lands for an arrow key.
fn arrow_collapse(sel: Selection, left: bool) -> Option<usize> {
    (!sel.is_empty()).then(|| if left { sel.start() } else { sel.end() })
}

impl MemoriaEditor {
    /// Move/collapse selection to a source position.
    pub(crate) fn set_caret(&mut self, pos: usize, extend: bool, cx: &mut Context<Self>) {
        let pos = pos.min(self.core.len());
        let sel = if extend {
            Selection::new(self.core.selection().anchor, pos)
        } else {
            Selection::caret(pos)
        };
        self.core.set_selection(sel);
        self.after_edit(false, cx);
    }

    /// Arrow key in visible space (`extend` = shift held).
    pub(crate) fn move_horiz(&mut self, left: bool, extend: bool, cx: &mut Context<Self>) {
        let sel = self.core.selection();
        if !extend {
            if let Some(pos) = arrow_collapse(sel, left) {
                self.set_caret(pos, false, cx);
                return;
            }
        }
        self.ensure_rows();
        let Some(proj) = self.proj.clone() else {
            return;
        };
        let vis = proj.to_visible(sel.head);
        let len = proj.text.len();
        let v = if left {
            vis.saturating_sub(1)
        } else {
            (vis + 1).min(len)
        };
        self.set_caret(proj.to_source(v), extend, cx);
    }

    /// Word jump — boundary search over the *visible* text so hidden markers
    /// don't count as words.
    pub(crate) fn move_word(&mut self, left: bool, extend: bool, cx: &mut Context<Self>) {
        self.ensure_rows();
        let Some(proj) = self.proj.clone() else {
            return;
        };
        let vis = proj.to_visible(self.core.selection().head);
        let text = &proj.text;
        let v = if left {
            text[..vis.min(text.len())]
                .split_word_bound_indices()
                .rev()
                .find(|(i, _)| *i < vis)
                .map(|(i, _)| i)
                .unwrap_or(0)
        } else {
            text.split_word_bound_indices()
                .find(|(i, _)| *i > vis)
                .map(|(i, _)| i)
                .unwrap_or(text.len())
        };
        self.set_caret(proj.to_source(v), extend, cx);
    }

    /// Home/End — visual line (wrap-aware) start/end inside the caret row.
    pub(crate) fn move_home_end(
        &mut self,
        end: bool,
        extend: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ensure_rows();
        let Some(proj) = self.proj.clone() else {
            return;
        };
        let vis = proj.to_visible(self.core.selection().head);
        let Some(i) = self.shape_row_at_vis(vis, window, cx) else {
            return;
        };
        let row = self.rows[i].clone();
        let local = vis.saturating_sub(row.vis.start).min(row.vis.len());
        let Some(slot) = self.shaped.get(&i) else {
            return;
        };
        // Visual line boundaries (wrap points) inside the row.
        let mut starts: Vec<usize> = vec![0];
        for b in slot.line.wrap_boundaries() {
            starts.push(slot.line.runs()[b.run_ix].glyphs[b.glyph_ix].index);
        }
        let mut ends: Vec<usize> = starts[1..].to_vec();
        ends.push(row.vis.len());
        // At a wrap boundary `local` is both `ends[k]` and `starts[k+1]`.
        // Home targets the line the caret visually sits on (k+1);
        // End targets k.
        let k = if end {
            starts
                .iter()
                .zip(ends.iter())
                .position(|(&s, &e)| local >= s && local <= e)
                .unwrap_or(0)
        } else {
            starts.iter().rposition(|&s| s <= local).unwrap_or(0)
        };
        let target_local = if end { ends[k] } else { starts[k] };
        let v = row.vis.start + target_local;
        self.set_caret(proj.to_source(v), extend, cx);
    }

    /// Up/Down — same x on the neighbour row (or the next visual line of a
    /// wrapped row).
    pub(crate) fn move_vert(
        &mut self,
        up: bool,
        extend: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ensure_rows();
        let Some(proj) = self.proj.clone() else {
            return;
        };
        let vis = proj.to_visible(self.core.selection().head);
        let Some(i) = self.shape_row_at_vis(vis, window, cx) else {
            return;
        };
        let row = self.rows[i].clone();
        let local = vis.saturating_sub(row.vis.start).min(row.vis.len());
        let Some(slot) = self.shaped.get(&i) else {
            return;
        };
        let st = layout::row_style(&row, self.zoom);
        let p = slot
            .line
            .position_for_index(local, st.line_height)
            .unwrap_or_default();
        let vlines = slot.line.wrap_boundaries().len() + 1;
        let k = (p.y / st.line_height).max(0.) as usize;
        let v = if up && k > 0 {
            // previous visual line, same row
            let target = gpui::point(p.x, st.line_height * (k - 1) as f32 + st.line_height / 2.);
            row.vis.start + layout::index_for_point(&slot.line, target, st.line_height)
        } else if !up && k + 1 < vlines {
            let target = gpui::point(p.x, st.line_height * (k + 1) as f32 + st.line_height / 2.);
            row.vis.start + layout::index_for_point(&slot.line, target, st.line_height)
        } else {
            // neighbour row
            let ni = if up { i.wrapping_sub(1) } else { i + 1 };
            let Some(nrow) = self.rows.get(ni).cloned() else {
                // above first row → doc start / below last → doc end
                let v = if up { 0 } else { proj.text.len() };
                self.set_caret(proj.to_source(v), extend, cx);
                return;
            };
            self.shape_row(ni, window);
            match self.shaped.get(&ni) {
                Some(nl) => {
                    let nst = layout::row_style(&nrow, self.zoom);
                    let target = gpui::point(p.x, nst.line_height / 2.);
                    nrow.vis.start + layout::index_for_point(&nl.line, target, nst.line_height)
                }
                None => nrow.vis.start,
            }
        };
        self.set_caret(proj.to_source(v), extend, cx);
    }

    /// Select all (source range — selection stays in source coords).
    pub(crate) fn select_all(&mut self, cx: &mut Context<Self>) {
        self.core.set_selection(Selection::new(0, self.core.len()));
        self.after_edit(false, cx);
    }

    /// Document start/end (ctrl-home/end).
    pub(crate) fn move_doc_edge(&mut self, end: bool, extend: bool, cx: &mut Context<Self>) {
        let pos = if end { self.core.len() } else { 0 };
        self.set_caret(pos, extend, cx);
    }

    // ---- editing keys -------------------------------------------------------

    pub(crate) fn key_enter(&mut self, cx: &mut Context<Self>) {
        self.core.key_enter();
        self.after_edit(true, cx);
    }
    pub(crate) fn key_shift_enter(&mut self, cx: &mut Context<Self>) {
        self.core.key_shift_enter();
        self.after_edit(true, cx);
    }
    pub(crate) fn key_backspace(&mut self, cx: &mut Context<Self>) {
        self.core.key_backspace();
        self.after_edit(true, cx);
    }
    pub(crate) fn key_delete(&mut self, cx: &mut Context<Self>) {
        self.core.key_delete();
        self.after_edit(true, cx);
    }
    pub(crate) fn key_tab(&mut self, shift: bool, cx: &mut Context<Self>) {
        if shift {
            self.core.key_shift_tab();
        } else {
            self.core.key_tab();
        }
        self.after_edit(true, cx);
    }

    // ---- commands ------------------------------------------------------------

    pub(crate) fn command(&mut self, cmd: Command, cx: &mut Context<Self>) {
        // A command can be a pure no-op (e.g. bold with nothing in scope) —
        // only treat it as an edit when it actually changed something.
        let (rev, sel) = (self.core.revision(), self.core.selection());
        self.core.command(&cmd);
        self.after_edit(
            self.core.revision() != rev || self.core.selection() != sel,
            cx,
        );
    }

    pub(crate) fn undo(&mut self, cx: &mut Context<Self>) {
        if self.core.undo() {
            self.after_edit(true, cx);
        }
    }
    pub(crate) fn redo(&mut self, cx: &mut Context<Self>) {
        if self.core.redo() {
            self.after_edit(true, cx);
        }
    }

    /// `ctrl-k` — arm the zen chord and tell the app (Vue opens search).
    pub(crate) fn ctrl_k(&mut self, cx: &mut Context<Self>) {
        self.arm_zen_chord(cx);
        cx.emit(crate::editor::EditorEvent::CtrlK);
    }
}
