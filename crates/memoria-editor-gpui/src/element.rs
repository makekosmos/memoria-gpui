//! `EditorElement` — the custom canvas element: prepaint shapes the visible
//! row window (virtualization), paint draws code panels, text, selection,
//! caret, rules and images, and registers the `ElementInputHandler` so the
//! platform IME/keyboard machinery talks to `MemoriaEditor`.

use gpui::{
    fill, point, px, size, App, Bounds, Element, ElementId, ElementInputHandler, Entity,
    GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, TextAlign, Window,
};

use crate::editor::MemoriaEditor;
use crate::layout;
use crate::rows::{self, RowKind};
use crate::style;

pub struct EditorElement {
    view: Entity<MemoriaEditor>,
}

impl EditorElement {
    pub fn new(view: Entity<MemoriaEditor>) -> Self {
        Self { view }
    }
}

impl IntoElement for EditorElement {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

/// Rows shaped+clipped for this frame, plus the selection/caret in row-local
/// coordinates (computed in prepaint so paint stays read-only).
#[derive(Default)]
pub struct EditorPrepaint {
    /// `(row_ix, top, height)` for every painted row.
    geometry: Vec<(usize, Pixels, Pixels)>,
    /// Caret quad (element coords) when focused & blinking on.
    caret: Option<Bounds<Pixels>>,
}

impl Element for EditorElement {
    type RequestLayoutState = ();
    type PrepaintState = EditorPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    /// Fill the parent — scroll is self-managed inside the element.
    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut s = gpui::Style::default();
        s.size.width = gpui::relative(1.).into();
        s.size.height = gpui::relative(1.).into();
        (window.request_layout(s, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _rl: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.view.update(cx, |e, cx| {
            e.bounds = bounds;
            let wrap = px(style::wrap_width(f32::from(bounds.size.width)));
            if e.layout.wrap_width != Some(wrap) {
                e.layout.wrap_width = Some(wrap);
                e.shaped.clear();
            }
            e.ensure_rows();
            // Shape the visible window; measuring can change heights → up to
            // two passes to converge.
            let mut visible = e
                .layout
                .visible(e.scroll_y, e.scroll_y + bounds.size.height);
            for _ in 0..2 {
                let mut changed = false;
                for i in visible.clone() {
                    changed |= e.shape_row(i, window);
                }
                if !changed {
                    break;
                }
                e.relayout();
                visible = e
                    .layout
                    .visible(e.scroll_y, e.scroll_y + bounds.size.height);
            }
            e.clamp_scroll();
            if e.follow_caret {
                e.scroll_to_caret(window, cx);
            }
            let mut prepaint = EditorPrepaint {
                geometry: Vec::with_capacity(visible.len()),
                caret: None,
            };
            for i in visible.clone() {
                prepaint
                    .geometry
                    .push((i, e.layout.offsets[i], e.layout.heights[i]));
            }
            if e.focus.is_focused(window) && e.cursor_visible {
                prepaint.caret = e.caret_bounds(bounds, window, cx);
            }
            prepaint
        })
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _rl: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.view.read(cx).focus.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.view.clone()),
            cx,
        );
        self.view.update(cx, |e, cx| {
            let text_x = px(style::text_origin_x(f32::from(bounds.size.width)));
            let origin_x = bounds.origin.x + text_x;
            let top = bounds.origin.y - e.scroll_y;

            // -- code panels behind contiguous code-row runs ----------------------
            let mut run_start: Option<(usize, Pixels)> = None;
            for &(i, y, _h) in &prepaint.geometry {
                let code = rows::is_code_row(&e.rows[i]);
                match (run_start, code) {
                    (None, true) => run_start = Some((e.rows[i].block_ix, y)),
                    (Some((b, y0)), false) => {
                        crate::paint::code_panel(bounds, origin_x, top + y0, y - y0, window);
                        run_start = None;
                        let _ = b;
                    }
                    _ => {}
                }
            }
            if let Some((_, y0)) = run_start {
                let last = prepaint.geometry.last().map(|g| g.1 + g.2).unwrap_or(y0);
                crate::paint::code_panel(bounds, origin_x, top + y0, last - y0, window);
            }

            // -- rows --------------------------------------------------------------
            let sel_vis = e.visible_selection();
            let placeholder_only =
                e.rows.len() == 1 && e.rows[0].vis.is_empty() && e.core.is_empty();
            for &(i, y, h) in &prepaint.geometry {
                let row = &e.rows[i];
                let row_top = top + y;
                match &row.kind {
                    RowKind::Rule => {
                        let cy = row_top + h / 2.;
                        window.paint_quad(fill(
                            Bounds::new(
                                point(origin_x, cy - px(0.5)),
                                size(px(style::wrap_width(f32::from(bounds.size.width))), px(1.)),
                            ),
                            style::marker_color(),
                        ));
                        continue;
                    }
                    RowKind::StandaloneImage { src } => {
                        crate::images::paint_image_row(
                            e,
                            src,
                            point(origin_x, row_top),
                            h,
                            window,
                            cx,
                        );
                        continue;
                    }
                    RowKind::Text => {}
                }
                let Some(slot) = e.shaped.get(&i) else {
                    continue;
                };
                let code = rows::is_code_row(row);
                let pad_x = if code {
                    px(style::CODE_PAD_X * e.zoom.0)
                } else {
                    px(0.)
                };
                let pad_y = if code && row.first_in_block {
                    layout::row_pad(row, e.zoom)
                } else {
                    px(0.)
                };
                let origin = point(origin_x + pad_x, row_top + pad_y);
                let st = layout::row_style(row, e.zoom);
                // selection underlay
                if let Some(sel) = &sel_vis {
                    let lo = sel.start.saturating_sub(row.vis.start);
                    let hi = sel.end.saturating_sub(row.vis.start).min(row.vis.len());
                    if sel.start < row.vis.end && sel.end > row.vis.start {
                        for r in crate::paint::selection_rects(
                            &slot.line,
                            lo,
                            hi,
                            st.line_height,
                            slot.width,
                        ) {
                            window.paint_quad(fill(
                                Bounds::new(origin + r.origin, r.size),
                                style::selection_bg(),
                            ));
                        }
                    }
                }
                slot.line
                    .paint(origin, st.line_height, TextAlign::Left, None, window, cx)
                    .ok();
            }

            crate::paint::placeholder(e, placeholder_only, bounds, origin_x, window, cx);
        });
        if let Some(c) = prepaint.caret.take() {
            window.paint_quad(fill(c, style::accent()));
        }
    }
}
