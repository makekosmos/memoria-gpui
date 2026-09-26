//! Paint helpers for `EditorElement` — code panels, per-visual-line
//! selection rects, and the empty-doc placeholder.

use gpui::{point, px, quad, size, Bounds, Pixels, TextAlign, Window};

use crate::editor::{MemoriaEditor, PLACEHOLDER};
use crate::style;

/// Rounded code panel behind a run of code rows.
pub fn code_panel(
    _bounds: Bounds<Pixels>,
    origin_x: Pixels,
    top: Pixels,
    height: Pixels,
    window: &mut Window,
) {
    // Panel spans the full text column.
    let w = px(style::wrap_width(f32::from(_bounds.size.width)));
    window.paint_quad(quad(
        Bounds::new(point(origin_x, top), size(w, height)),
        gpui::Corners::all(px(style::CODE_RADIUS)),
        style::code_bg(),
        gpui::Edges::default(),
        gpui::transparent_black(),
        gpui::BorderStyle::default(),
    ));
}

/// Per-visual-line rects of `sel` (row-local byte range) inside a wrapped row.
pub fn selection_rects(
    line: &gpui::WrappedLine,
    lo: usize,
    hi: usize,
    line_height: Pixels,
    wrap_width: Pixels,
) -> Vec<Bounds<Pixels>> {
    let len = line.len();
    // Visual line boundaries: byte indices where the row wraps.
    let mut starts: Vec<usize> = vec![0];
    for b in line.wrap_boundaries() {
        let run = &line.runs()[b.run_ix];
        starts.push(run.glyphs[b.glyph_ix].index);
    }
    let mut ends = starts[1..].to_vec();
    ends.push(len);
    let mut rects = Vec::new();
    for (k, (&s, &e_ix)) in starts.iter().zip(ends.iter()).enumerate() {
        let a = lo.max(s).min(e_ix);
        let b = hi.max(s).min(e_ix);
        if a >= b {
            continue;
        }
        let y = line_height * k as f32;
        let x0 = line
            .position_for_index(a, line_height)
            .map(|p| p.x)
            .unwrap_or(px(0.));
        let x1 = if b == e_ix && k + 1 < starts.len() {
            wrap_width
        } else {
            line.position_for_index(b, line_height)
                .map(|p| p.x)
                .unwrap_or(wrap_width)
        };
        if x1 > x0 {
            rects.push(Bounds::new(point(x0, y), size(x1 - x0, line_height)));
        }
    }
    rects
}

/// «Начните писать...» — drawn only when the doc is fully empty.
pub fn placeholder(
    e: &mut MemoriaEditor,
    placeholder_only: bool,
    bounds: Bounds<Pixels>,
    origin_x: Pixels,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    if !placeholder_only {
        return;
    }
    let mut st = window.text_style();
    st.color = style::placeholder_color();
    st.font_size = style::block_style(&memoria_editor_core::project::BlockTag::Paragraph, e.zoom)
        .size
        .into();
    let line = window.text_system().shape_line(
        PLACEHOLDER.into(),
        px(style::BODY_SIZE * e.zoom.0),
        &[gpui::TextRun {
            len: PLACEHOLDER.len(),
            font: st.font(),
            color: st.color,
            ..Default::default()
        }],
        None,
    );
    line.paint(
        point(origin_x, bounds.origin.y),
        px(style::BODY_SIZE * style::BODY_LH * e.zoom.0),
        TextAlign::Left,
        None,
        window,
        cx,
    )
    .ok();
}
