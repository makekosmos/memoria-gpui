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
