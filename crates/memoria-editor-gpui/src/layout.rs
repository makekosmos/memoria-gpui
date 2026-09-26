//! Row geometry: cumulative row heights (virtualized — unmeasured rows use
//! one line-height), wrap-aware hit-testing and caret placement via
//! `WrappedLineLayout`. Rows are shaped lazily for the visible window only;
//! heights are corrected once a row is measured, which converges in one
//! frame (GPUI re-runs layout after `cx.notify`).

use crate::rows::Row;
use crate::style::{self, BlockStyle, EditorScale};
use gpui::{px, Pixels, Point, WrappedLine};
use std::ops::Range;

/// Extra vertical padding rows get inside a code panel (top/bottom edge).
pub fn row_pad(row: &Row, z: EditorScale) -> Pixels {
    if crate::rows::is_code_row(row) {
        px(style::CODE_PAD_Y * z.0)
    } else {
        px(0.)
    }
}

/// Gap inserted between two rows of different top-level blocks.
pub fn inter_block_gap(prev: &Row, next: &Row, z: EditorScale) -> Pixels {
    if prev.block_ix != next.block_ix {
        px(style::BLOCK_GAP * z.0 * gap_factor(next))
    } else {
        px(0.)
    }
}

/// Vue scales block margins down for tight elements (list items); code panels
/// already carry their own padding.
fn gap_factor(next: &Row) -> f32 {
    match next.tag {
        memoria_editor_core::project::BlockTag::Item { .. } => 0.2,
        memoria_editor_core::project::BlockTag::CodeBlock { .. } => 0.6,
        _ => 1.0,
    }
}

/// Whole-document layout snapshot: rows + per-row heights + y offsets.
#[derive(Default)]
pub struct DocLayout {
    pub heights: Vec<Pixels>,
    pub offsets: Vec<Pixels>,
    pub content_h: Pixels,
    /// Measured wrap width (None until first prepaint).
    pub wrap_width: Option<Pixels>,
}

impl DocLayout {
    /// Recompute offsets/heights for `rows`; `measured(i)` gives the real
    /// height for shaped rows.
    pub fn rebuild<F>(&mut self, rows: &[Row], z: EditorScale, measured: F)
    where
        F: Fn(usize) -> Option<Pixels>,
    {
        self.heights.clear();
        self.offsets.clear();
        let mut y = Pixels::ZERO;
        let mut prev: Option<&Row> = None;
        for (i, row) in rows.iter().enumerate() {
            if let Some(p) = prev {
                y += inter_block_gap(p, row, z);
            }
            let mut h = measured(i).unwrap_or_else(|| est_height(row, z));
            if crate::rows::is_code_row(row) && (row.first_in_block || row.last_in_block) {
                h += row_pad(row, z);
            }
            self.offsets.push(y);
            self.heights.push(h);
            y += h;
            prev = Some(row);
        }
        self.content_h = y;
    }

    /// Index of the row at content y (scroll offset space), or the last row.
    pub fn row_at_y(&self, y: Pixels) -> usize {
        let i = self.offsets.partition_point(|o| *o <= y);
        i.saturating_sub(1)
    }

    /// Rows intersecting `[y0, y1)` plus their y offsets.
    pub fn visible(&self, y0: Pixels, y1: Pixels) -> Range<usize> {
        let start = self.row_at_y(y0);
        let mut end = start;
        while end < self.offsets.len()
            && self.offsets[end] + self.heights[end] <= y1.max(y0 + px(1.))
        {
            end += 1;
        }
        start..(end + 1).min(self.offsets.len())
    }
}

/// Estimated height before shaping: one visual line (+code pad).
pub fn est_height(row: &Row, z: EditorScale) -> Pixels {
    let st = style::block_style(&row.tag, z);
    match row.kind {
        crate::rows::RowKind::Rule => px(10.) + px(1.),
        crate::rows::RowKind::StandaloneImage { .. } => px(crate::images::IMAGE_ROW_H * z.0),
        crate::rows::RowKind::Text => st.line_height,
    }
}

/// Point (row-local) → byte index (row-local) — for mouse hit-testing.
pub fn index_for_point(line: &WrappedLine, position: Point<Pixels>, line_height: Pixels) -> usize {
    line.closest_index_for_position(position, line_height)
        .unwrap_or_else(|i| i)
}

/// Block style for a row.
pub fn row_style(row: &Row, z: EditorScale) -> BlockStyle {
    style::block_style(&row.tag, z)
}

// ---------------------------------------------------------------------------
// `MemoriaEditor` row-shaping — moved here (file-size gate) since every
// function is a layout concern.
// ---------------------------------------------------------------------------

use std::rc::Rc;
use std::sync::Arc;

use crate::editor::MemoriaEditor;
use gpui::Window;

/// Pixels shaved off a row's wrap width (code rows are inset by panel padding).
fn row_wrap_delta(row: &Row, z: EditorScale) -> Pixels {
    if crate::rows::is_code_row(row) {
        px(2. * style::CODE_PAD_X * z.0)
    } else {
        px(0.)
    }
}

/// Cached shaped line plus the wrap width it was shaped at.
pub(crate) struct WrappedSlot {
    pub line: gpui::WrappedLine,
    pub width: Pixels,
}

impl MemoriaEditor {
    /// Text-column X origin — compact mode drops the centered 760px column
    /// and fills the container (the composer card carries its own padding).
    pub(crate) fn text_origin_x(&self, bounds_w: f32) -> f32 {
        if self.compact {
            0.0
        } else {
            style::text_origin_x(bounds_w)
        }
    }

    /// Text-column wrap width — full container width in compact mode.
    pub(crate) fn text_wrap_width(&self, bounds_w: f32) -> f32 {
        if self.compact {
            bounds_w
        } else {
            style::wrap_width(bounds_w)
        }
    }

    // ---- projection / rows -------------------------------------------------

    /// Rebuild projection + rows when the buffer or selection changed.
    pub(crate) fn ensure_rows(&mut self) {
        let (rev, sel) = (self.core.revision(), self.core.selection());
        if self.built_rev == rev && self.built_sel == sel && self.proj.is_some() {
            return;
        }
        let (proj, doc) = self.core.project_and_doc();
        let proj = Arc::new(proj.clone());
        self.rows = Rc::new(crate::rows::build_rows(&proj, doc));
        self.proj = Some(proj);
        self.shaped.clear();
        self.built_rev = rev;
        self.built_sel = sel;
        self.relayout();
    }

    pub(crate) fn relayout(&mut self) {
        let z = self.zoom;
        let shaped = &self.shaped;
        let global = self.layout.wrap_width;
        let rows = self.rows.clone();
        self.layout.rebuild(&rows, z, |i| {
            let slot = shaped.get(&i)?;
            let row = &rows[i];
            let want = global? - row_wrap_delta(row, z);
            (slot.width == want).then(|| {
                slot.line
                    .size(style::block_style(&row.tag, z).line_height)
                    .height
            })
        });
    }

    /// Ensure `row` is shaped for `wrap_width`; returns whether its measured
    /// height changed (caller re-runs layout if so).
    pub(crate) fn shape_row(&mut self, i: usize, window: &mut Window) -> bool {
        let Some(global_wrap) = self.layout.wrap_width else {
            return false;
        };
        let old_h = self.layout.heights.get(i).copied();
        let row = self.rows.get(i).cloned().unwrap_or_else(|| Row {
            vis: 0..0,
            block_ix: 0,
            tag: memoria_editor_core::project::BlockTag::Paragraph,
            spans: vec![],
            kind: crate::rows::RowKind::Text,
            first_in_block: true,
            last_in_block: true,
            code_origin: 0,
        });
        let z = self.zoom;
        let st = style::block_style(&row.tag, z);
        // Code rows are inset by the panel padding on both sides.
        let wrap = global_wrap - row_wrap_delta(&row, z);
        if let Some(slot) = self.shaped.get(&i) {
            if slot.width == wrap {
                return false;
            }
        }
        let runs = self.runs_for_row(i, &row);
        // NB: H5's CSS `text-transform: uppercase` is NOT reproduced —
        // uppercasing here would desync shaped-glyph indices for
        // case-expanding chars (ß→SS). Documented GAP in PARITY.md.
        let text = self
            .proj
            .as_ref()
            .map(|p| gpui::SharedString::from(p.text[row.vis.clone()].to_string()))
            .unwrap_or_default();
        let ts = window.text_system();
        let line = ts
            .shape_text(text, st.size, &runs, Some(wrap), None)
            .unwrap_or_default()
            .into_iter()
            .next()
            .unwrap_or_default();
        let h = line.size(st.line_height).height
            + if crate::rows::is_code_row(&row) && (row.first_in_block || row.last_in_block) {
                crate::layout::row_pad(&row, z)
            } else {
                px(0.)
            };
        self.shaped.insert(i, WrappedSlot { line, width: wrap });
        Some(h) != old_h
    }
}
