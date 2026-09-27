//! Shared modal chrome — dimmed backdrop + centered panel + footer buttons,
//! the same shape as the confirm dialog in `confirm.rs`. Used by the cover
//! and book-metadata modals.
use gpui::{div, prelude::*, px, Context, SharedString};

use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;

/// Dimmed fullscreen shell that centers `panel`.
pub(crate) fn modal_shell(id: &'static str, panel: impl IntoElement) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("{id}-backdrop")))
        .debug_selector(move || format!("{id}-backdrop"))
        .absolute()
        .inset_0()
        .occlude()
        .bg(rgba(0x000000, 0.4))
        .flex()
        .items_center()
        .justify_center()
        .child(panel)
}

/// Panel chrome — title + column layout.
pub(crate) fn modal_panel(id: &'static str, title: &str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .occlude()
        .w(px(480.))
        .max_h(px(560.))
        .bg(c(SIDEBAR_BG()))
        .border_1()
        .border_color(c(BORDER()))
        .rounded_lg()
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_size(px(15.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(c(FG()))
                .child(title.to_string()),
        )
}

/// Ghost (outline) footer button.
pub(crate) fn ghost_btn(
    id: &'static str,
    label: &'static str,
    cx: &mut Context<Memoria>,
    f: impl Fn(&mut Memoria, &mut Context<Memoria>) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let weak = cx.weak_entity();
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .a11y_button(label)
        .px_4()
        .h(px(30.))
        .flex()
        .items_center()
        .rounded_md()
        .text_size(px(13.))
        .text_color(c(FG()))
        .border_1()
        .border_color(c(BORDER()))
        .cursor_pointer()
        .hover(|s| s.bg(rgba(FG(), 0.08)))
        .on_click(move |_, _, cx| {
            let _ = weak.update(cx, |this, cx| f(this, cx));
        })
}

/// Solid primary footer button (Vue `Button` default variant); `enabled=false`
/// renders it dimmed and inert.
pub(crate) fn primary_btn(
    id: &'static str,
    label: &'static str,
    enabled: bool,
    cx: &mut Context<Memoria>,
    f: impl Fn(&mut Memoria, &mut Context<Memoria>) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let weak = cx.weak_entity();
    let mut el = div()
        .id(id)
        .debug_selector(move || id.to_string())
        .a11y_button(label)
        .px_4()
        .h(px(30.))
        .flex()
        .items_center()
        .rounded_md()
        .text_size(px(13.));
    if enabled {
        el = el
            .bg(c(FG()))
            .text_color(c(BG()))
            .cursor_pointer()
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| f(this, cx));
            });
    } else {
        el = el.bg(rgba(FG(), 0.3)).text_color(rgba(BG(), 0.6));
    }
    el
}
