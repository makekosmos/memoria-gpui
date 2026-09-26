//! Geometry helpers on `MemoriaEditor`: source↔visible↔row mapping, caret
//! placement, hit-testing (mouse + IME), and scroll-to-caret.

use gpui::{px, Bounds, Context, Pixels, Point, TextRun};
use std::ops::Range;

use crate::editor::MemoriaEditor;
use crate::layout;
use crate::rows::{self, Row};
use crate::style;

impl MemoriaEditor {
    /// `TextRun`s for row `i` (also feeds `runs.rs` the syntax overlay +
    /// IME marked range).
    pub(crate) fn runs_for_row(&mut self, _i: usize, row: &Row) -> Vec<TextRun> {
        let proj_text = self
            .proj
            .as_ref()
            .map(|p| p.text.clone())
            .unwrap_or_default();
        let code_styles = if rows::is_code_row(row) {
            let lang = rows::code_lang(&row.tag).unwrap_or("").to_string();
            let text = rows::code_block_text(&self.rows, row.block_ix, &proj_text);
            self.high.styles_for(&lang, &text).cloned()
        } else {
            None
        };
        let marked = self.core.marked_source_range().map(|r| {
            let p = self.proj.as_ref().unwrap();
            p.to_visible(r.start)..p.to_visible(r.end).max(p.to_visible(r.start))
        });
        crate::runs::runs_for_row(
            row,
            &proj_text,
            code_styles.as_deref(),
            row.code_origin,
            marked,
            self.zoom,
        )
    }

    /// First row whose `vis.end >= v` (row containing visible index `v`;
    /// end-of-row positions map to that row's tail).
    pub(crate) fn row_at_vis(&self, v: usize) -> Option<usize> {
        if self.rows.is_empty() {
            return None;
        }
        let i = self
            .rows
            .partition_point(|r| r.vis.end < v)
            .min(self.rows.len() - 1);
        Some(i)
    }

    /// Ensure the row containing visible index `v` is shaped; returns its index.
    pub(crate) fn shape_row_at_vis(
        &mut self,
        v: usize,
        window: &mut gpui::Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let i = self.row_at_vis(v)?;
        self.shape_row(i, window);
        Some(i)
    }

    /// Caret quad in **element** coordinates (add `bounds.origin` for window).
    pub(crate) fn caret_bounds(
        &mut self,
        element_bounds: Bounds<Pixels>,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let head_src = self.core.selection().head;
        self.ensure_rows();
        let proj = self.proj.clone()?;
        let vis = proj.to_visible(head_src);
        let i = self.shape_row_at_vis(vis, window, cx)?;
        let row = self.rows.get(i)?.clone();
        if !matches!(row.kind, rows::RowKind::Text) {
            return None;
        }
        let slot = self.shaped.get(&i)?;
        let st = layout::row_style(&row, self.zoom);
        let local = vis.saturating_sub(row.vis.start).min(row.vis.len());
        let p = slot
            .line
            .position_for_index(local, st.line_height)
            .unwrap_or_default();
        let pad_x = if rows::is_code_row(&row) {
            px(style::CODE_PAD_X * self.zoom.0)
        } else {
            px(0.)
        };
        let pad_y = if rows::is_code_row(&row) && row.first_in_block {
            layout::row_pad(&row, self.zoom)
        } else {
            px(0.)
        };
        Some(Bounds::new(
            Point::new(
                element_bounds.origin.x
                    + px(self.text_origin_x(f32::from(element_bounds.size.width)))
                    + pad_x
                    + p.x,
                element_bounds.origin.y + self.layout.offsets[i] + pad_y - self.scroll_y + p.y,
            ),
            gpui::size(px(style::CARET_W), st.line_height),
        ))
    }

    /// Selection in visible-projection coords (`None` when it's a caret).
    pub(crate) fn visible_selection(&self) -> Option<Range<usize>> {
        let sel = self.core.selection();
        if sel.is_empty() {
            return None;
        }
        let proj = self.proj.as_ref()?;
        let (a, b) = if sel.anchor <= sel.head {
            (sel.anchor, sel.head)
        } else {
            (sel.head, sel.anchor)
        };
        let lo = proj.to_visible(a);
        let hi = proj.to_visible(b).max(lo);
        (hi > lo).then_some(lo..hi)
    }

    /// Keep the caret inside the viewport (call after `relayout`/`clamp`).
    pub(crate) fn scroll_to_caret(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let head = self.core.selection().head;
        let Some(proj) = self.proj.clone() else {
            return;
        };
        let vis = proj.to_visible(head);
        let Some(i) = self.shape_row_at_vis(vis, window, cx) else {
            return;
        };
        let y0 = self.layout.offsets[i];
        let y1 = y0 + self.layout.heights[i];
        let vh = self.bounds.size.height;
        if y0 < self.scroll_y {
            self.scroll_y = y0;
        } else if y1 > self.scroll_y + vh {
            self.scroll_y = (y1 - vh).max(px(0.));
        }
        self.clamp_scroll();
    }

    /// Window-space point → visible index (`None` when geometry isn't ready).
    /// Used by `character_index_for_point` (IME) and mouse hit-testing.
    pub(crate) fn vis_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut gpui::Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        self.ensure_rows();
        let local = point - self.bounds.origin;
        let content_x = local.x - px(self.text_origin_x(f32::from(self.bounds.size.width)));
        let content_y = local.y + self.scroll_y;
        let i = self
            .layout
            .row_at_y(content_y)
            .min(self.rows.len().saturating_sub(1));
        self.shape_row(i, window);
        let row = self.rows.get(i)?.clone();
        let slot = self.shaped.get(&i)?;
        let pad_x = if rows::is_code_row(&row) {
            px(style::CODE_PAD_X * self.zoom.0)
        } else {
            px(0.)
        };
        let pad_y = if rows::is_code_row(&row) && row.first_in_block {
            layout::row_pad(&row, self.zoom)
        } else {
            px(0.)
        };
        let in_row = Point::new(
            content_x - pad_x,
            content_y - self.layout.offsets[i] - pad_y,
        );
        let st = layout::row_style(&row, self.zoom);
        let idx = layout::index_for_point(&slot.line, in_row, st.line_height);
        Some(row.vis.start + idx.min(row.vis.len()))
    }
}

/// Horizontal scroll-by (wheel) → clamped scroll offset change.
pub(crate) fn apply_scroll(e: &mut MemoriaEditor, delta: Pixels) {
    e.scroll_y += delta;
    e.clamp_scroll();
    e.follow_caret = false;
}
