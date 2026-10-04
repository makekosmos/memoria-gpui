//! `ImageObjectView` port — gallery tiles + full-size view.
use gpui::{div, img, prelude::*, px, Context, SharedString};
use gpui_component::scroll::ScrollableElement;

use memoria_model::dates::format_russian_date_ms;
use memoria_model::model::Entry;
use memoria_model::object_views::entry_display_title;
use memoria_model::routes::Route;

use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;

impl Memoria {
    /// `ImageObjectView` tile — thumbnail via data-uri/remote source.
    pub(crate) fn image_tile(&mut self, e: &Entry, cx: &mut Context<Self>) -> impl IntoElement {
        let eid = e.id.clone();
        let sel = format!("image-tile-{eid}");
        let weak = cx.weak_entity();
        let src = memoria_model::image_src::entry_image_src(e);
        div()
            .id(SharedString::from(format!("img-{eid}")))
            .debug_selector(move || sel.clone())
            .a11y_button(entry_display_title(e))
            .w(px(200.))
            .rounded_lg()
            .overflow_hidden()
            .border_1()
            .border_color(c(BORDER()))
            .bg(c(CARD()))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(FG(), 0.05)))
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.navigate(Route::Entry(eid.clone()), cx));
            })
            .when_some(src, |d, s| {
                d.child(
                    img(SharedString::from(s))
                        .w_full()
                        .h(px(140.))
                        .object_fit(gpui::ObjectFit::Cover),
                )
            })
            .child(
                div()
                    .p_2()
                    .text_size(px(12.))
                    .text_color(c(FG()))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(entry_display_title(e)),
            )
    }

    /// `ImageObjectView` — centered large image + meta.
    pub(crate) fn render_image_object(
        &mut self,
        e: &Entry,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let src = memoria_model::image_src::entry_image_src(e);
        div()
            .id("image-object-view")
            .debug_selector(|| "image-object-view".into())
            .size_full()
            .overflow_y_scrollbar()
            .p(px(36.))
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(c(FG()))
                    .child(entry_display_title(e)),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .when_some(src.clone(), |d, s| {
                        d.child(
                            img(SharedString::from(s))
                                .max_w(px(720.))
                                .object_fit(gpui::ObjectFit::Contain),
                        )
                    })
                    .when(src.is_none(), |d| {
                        d.child(
                            div()
                                .text_color(c(MUTED_FG()))
                                .text_size(px(13.))
                                .child("Нет изображения"),
                        )
                    }),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(c(MUTED_FG()))
                    .child(format_russian_date_ms(e.updated_at)),
            )
    }
}
