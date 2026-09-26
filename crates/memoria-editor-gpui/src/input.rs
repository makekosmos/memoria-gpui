//! `EntityInputHandler` — GPUI's IME contract over the core's UTF-16 surface.
//! Ranges are UTF-16 code units into the **source** markdown (the platform's
//! view of the document). `bounds_for_range` maps source → visible → screen
//! so the candidate window lands on the caret; `character_index_for_point`
//! goes the other way for mouse-driven composition positioning.

use gpui::{
    px, Bounds, ClipboardItem, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window,
};
use std::ops::Range;

use crate::editor::MemoriaEditor;

impl EntityInputHandler for MemoriaEditor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let (text, clamped) = self.core.text_for_range(range)?;
        *adjusted_range = Some(clamped);
        Some(text)
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let range = self.core.selected_text_range_utf16();
        let sel = self.core.selection();
        Some(UTF16Selection {
            range,
            reversed: sel.head < sel.anchor,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.core.marked_text_range_utf16()
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.core.unmark_text();
        // The preedit underline is baked into shaped runs — drop the cache
        // so the next frame reshapes without it.
        self.shaped.clear();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.core.replace_text_in_range(range, text);
        self.after_edit(true, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.core
            .replace_and_mark_text_in_range(range, new_text, new_selected_range);
        self.after_edit(true, cx);
    }

    /// IME candidate window: bounds of the marked (or caret) range in window
    /// coordinates — `element_bounds` is the painted element's frame.
    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // UTF-16 → source bytes → visible → content coords → screen.
        let src = self.core.buffer().utf16_to_byte(range_utf16.start);
        let src_end = self.core.buffer().utf16_to_byte(range_utf16.end);
        self.ensure_rows();
        let proj = self.proj.clone()?;
        let v0 = proj.to_visible(src);
        let v1 = proj.to_visible(src_end).max(v0);
        let i = self.shape_row_at_vis(v0, window, cx)?;
        let row = self.rows.get(i)?.clone();
        let slot = self.shaped.get(&i)?;
        let st = crate::layout::row_style(&row, self.zoom);
        let pad_x = if crate::rows::is_code_row(&row) {
            px(crate::style::CODE_PAD_X * self.zoom.0)
        } else {
            px(0.)
        };
        let pad_y = if crate::rows::is_code_row(&row) && row.first_in_block {
            crate::layout::row_pad(&row, self.zoom)
        } else {
            px(0.)
        };
        let p0 = slot
            .line
            .position_for_index(
                v0.saturating_sub(row.vis.start).min(row.vis.len()),
                st.line_height,
            )
            .unwrap_or_default();
        let x0 = element_bounds.origin.x
            + px(crate::style::text_origin_x(f32::from(
                element_bounds.size.width,
            )))
            + pad_x;
        let mut bounds = Bounds::new(
            Point::new(
                x0 + p0.x,
                element_bounds.origin.y + self.layout.offsets[i] + pad_y - self.scroll_y + p0.y,
            ),
            gpui::size(px(crate::style::CARET_W), st.line_height),
        );
        if v1 > v0 {
            // Same-visual-line range → widen to cover it (candidate window).
            let p1 = slot
                .line
                .position_for_index(
                    v1.saturating_sub(row.vis.start).min(row.vis.len()),
                    st.line_height,
                )
                .unwrap_or_default();
            if p1.y == p0.y {
                bounds.size.width = (p1.x - p0.x).max(bounds.size.width);
            }
        }
        Some(bounds)
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        let vis = self.vis_index_for_point(point, window, cx)?;
        let proj = self.proj.clone()?;
        let src = proj.to_source(vis);
        Some(self.core.buffer().byte_to_utf16(src))
    }

    fn text_length_utf16(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.core.buffer().byte_to_utf16(self.core.len()))
    }

    /// Vue paste semantics: HTML wins (converted via html2md), else plain.
    fn paste(&mut self, item: ClipboardItem, _window: &mut Window, cx: &mut Context<Self>) {
        let mut plain = None;
        let mut html = None;
        for e in item.entries() {
            if let gpui::ClipboardEntry::String(s) = e {
                let is_html = s.metadata.as_deref().is_some_and(|m| m.contains("html"));
                if is_html {
                    html = html.or(Some(s.text.clone()));
                } else {
                    plain = plain.or(Some(s.text.clone()));
                }
            }
        }
        // Some platforms ship a bare `<tag>` string without a mime marker —
        // `looks_like_html` inside `core::paste` catches that too.
        let plain = plain.or_else(|| item.text());
        self.core.paste(plain.as_deref(), html.as_deref());
        self.after_edit(true, cx);
    }
}
