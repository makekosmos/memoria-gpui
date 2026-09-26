//! Sticker window — `StickerNoteView`/`useDockedWidget` port.
//! A `WindowKind::Floating` GPUI window per entry id renders the note
//! read-only (M7 owns the full sticker/dock UX; this proves the standalone
//! window path works without Host).
use std::time::Duration;

use gpui::{div, prelude::*, px, Context, Render, Window};
use gpui_component::scroll::ScrollableElement;
use serde_json::Value;

use memoria_gpui::model::Entry;
use memoria_gpui::object_views::entry_display_title;
use memoria_gpui::preview::markdown_plain_text;
use memoria_gpui::sidebar_model::IconId;
use memoria_gpui::sticker_route::sticker_route_for;
use memoria_gpui::store::{Command, Reply};

use super::backend::Backend;
use super::{icon, DemoStore};
use crate::a11y::A11y;
use crate::theme::*;

/// Floating read-only note window — `Memoria` in miniature (own backend).
pub struct StickerView {
    backend: Backend,
    entry_id: String,
    entry: Option<Entry>,
    pinned: bool,
    _poll: Option<gpui::Task<()>>,
}

impl StickerView {
    pub fn new(entry_id: String, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let backend = if std::env::var("MEMORIA_DEMO").as_deref() == Ok("1") || cfg!(test) {
            Backend::Demo(DemoStore::seeded())
        } else {
            Backend::Engine(memoria_gpui::store::Worker::start())
        };
        let mut this = Self {
            backend,
            entry_id,
            entry: None,
            pinned: true,
            _poll: None,
        };
        for reply in this.backend.send(Command::LoadEntry {
            id: this.entry_id.clone(),
            content_only: false,
        }) {
            this.on_reply(reply);
        }
        this._poll = Some(cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            if this
                .update(cx, |this, cx| {
                    for reply in this.backend.drain() {
                        this.on_reply(reply);
                    }
                    cx.notify();
                })
                .is_err()
            {
                break;
            }
        }));
        this
    }

    fn on_reply(&mut self, reply: Reply) {
        if let Reply::Entry { id, result } = reply {
            if id == self.entry_id {
                self.entry = result.ok().flatten();
            }
        }
    }
}

impl Render for StickerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self
            .entry
            .as_ref()
            .map(entry_display_title)
            .unwrap_or_else(|| "Стикер".into());
        let body = self
            .entry
            .as_ref()
            .map(|e| {
                markdown_plain_text(&memoria_gpui::content::read_entry_markdown(
                    &serde_json::from_str(&e.content_json).unwrap_or(Value::Null),
                ))
            })
            .unwrap_or_default();
        let _route = sticker_route_for(&self.entry_id);
        let weak = cx.weak_entity();
        let pinned = self.pinned;
        div()
            .id("sticker-view")
            .debug_selector(|| "sticker-view".into())
            .size_full()
            .flex()
            .flex_col()
            .bg(c(CARD()))
            .child(
                div()
                    .id("sticker-titlebar")
                    .h(px(36.))
                    .flex()
                    .items_center()
                    .px_3()
                    .gap_2()
                    .window_control_area(gpui::WindowControlArea::Drag)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(13.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(c(FG()))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(title),
                    )
                    .child(
                        div()
                            .id("sticker-pin")
                            .debug_selector(|| "sticker-pin".into())
                            .a11y_button(if pinned {
                                "Открепить"
                            } else {
                                "Закрепить"
                            })
                            .w(px(24.))
                            .h(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(FG(), 0.08)))
                            .on_click(move |_, _, cx| {
                                let _ = weak.update(cx, |this, cx| {
                                    this.pinned = !this.pinned;
                                    cx.notify();
                                });
                            })
                            .child(icon(
                                IconId::Pin,
                                13.,
                                if pinned { c(ACCENT()) } else { rgba(FG(), 0.6) },
                            )),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .p_4()
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .text_color(c(FG()))
                    .child(if body.is_empty() {
                        "Загрузка…".to_string()
                    } else {
                        body
                    }),
            )
    }
}
