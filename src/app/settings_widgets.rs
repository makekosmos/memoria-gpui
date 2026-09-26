//! Settings widgets — section headers, toggle rows, outline/danger buttons.
use gpui::{div, prelude::*, px, SharedString};

use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;

pub(crate) fn section_header(text: &'static str) -> gpui::Div {
    div()
        .pt_4()
        .pb_1()
        .px_2()
        .text_size(px(11.))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(c(MUTED_FG()))
        .child(text)
}

pub(crate) fn toggle_row(
    id: String,
    title: String,
    desc: String,
    on: bool,
    weak: gpui::WeakEntity<Memoria>,
    apply: impl Fn(&mut Memoria, bool) + 'static,
) -> impl IntoElement {
    div()
        .id(SharedString::from(id.clone()))
        .debug_selector(move || id.clone())
        .a11y_switch(title.clone(), on)
        .flex()
        .items_center()
        .gap_3()
        .px_2()
        .py_2()
        .rounded_md()
        .cursor_pointer()
        .hover(|s| s.bg(rgba(FG(), 0.05)))
        .on_click(move |_, _, cx| {
            let _ = weak.update(cx, |this, cx| {
                apply(this, !on);
                this.persist_prefs();
                cx.notify();
            });
        })
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .child(div().text_size(px(13.)).text_color(c(FG())).child(title))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(c(MUTED_FG()))
                        .child(desc),
                ),
        )
        .child(toggle(on))
}

pub(crate) fn toggle(on: bool) -> gpui::Div {
    div()
        .w(px(36.))
        .h(px(20.))
        .rounded_full()
        .bg(if on { c(ACCENT()) } else { rgba(FG(), 0.2) })
        .flex()
        .items_center()
        .px(px(2.))
        .child(
            div()
                .size(px(16.))
                .rounded_full()
                .bg(c(FG()))
                .when(on, |d| d.ml(px(16.))),
        )
}

pub(crate) fn settings_button(
    id: &'static str,
    label: &'static str,
    danger: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    button_el(id, label, danger, on_click)
}

pub(crate) fn settings_button_danger(
    id: &'static str,
    label: &'static str,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    button_el(id, label, true, on_click)
}

pub(crate) fn button_el(
    id: &'static str,
    label: &'static str,
    danger: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(id.to_string()))
        .debug_selector(move || format!("settings-btn-{id}"))
        .a11y_button(label)
        .px_3()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(if danger {
            c(DESTRUCTIVE())
        } else {
            c(BORDER())
        })
        .text_size(px(12.))
        .text_color(if danger { c(DESTRUCTIVE()) } else { c(FG()) })
        .cursor_pointer()
        .hover(move |s| s.bg(rgba(if danger { DESTRUCTIVE() } else { FG() }, 0.08)))
        .on_click(on_click)
        .child(label)
}
