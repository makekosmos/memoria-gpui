//! Read-only note object view — M3 (KOS-150) owns the editable Tiptap
//! surface; until then the entry body renders as markdown text projection
//! (M1 codec `read_entry_markdown`), matching the read-only requirement.
use gpui::{div, prelude::*, px, Context};
use gpui_component::scroll::ScrollableElement;
use serde_json::Value;

use memoria_gpui::dates::format_russian_date_ms;
use memoria_gpui::model::Entry;
use memoria_gpui::object_views::entry_display_title;
use memoria_gpui::preview::markdown_plain_text;
use memoria_gpui::sidebar_model::IconId;

use super::{icon, Memoria};
use crate::a11y::A11y;
use crate::theme::*;

impl Memoria {
    /// Note body — title header + plain-text markdown projection + updated
    /// date; a banner marks it read-only until M3.
    pub(crate) fn render_note(
        &mut self,
        entry: &Entry,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let markdown = memoria_gpui::content::read_entry_markdown(
            &serde_json::from_str(&entry.content_json).unwrap_or(Value::Null),
        );
        let body = markdown_plain_text(&markdown);
        let note_type = entry
            .type_id
            .as_deref()
            .and_then(|t| self.note_types.iter().find(|nt| nt.id == t))
            .cloned();

        let mut header = div()
            .id("entry-titlebar")
            .flex()
            .items_start()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(22.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(c(FG()))
                            .child(entry_display_title(entry)),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(c(MUTED_FG()))
                            .child(format_russian_date_ms(entry.updated_at)),
                    ),
            )
            .child(
                // «⋮» — context menu (pin / sticker / delete) like card menu.
                {
                    let eid = entry.id.clone();
                    let weak = cx.weak_entity();
                    div()
                        .id("entry-menu-btn")
                        .debug_selector(|| "entry-menu-btn".into())
                        .a11y_button("Меню объекта")
                        .w(px(28.))
                        .h(px(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(FG(), 0.08)))
                        .on_click(move |ev, _, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.open_ctx_menu(
                                    eid.clone(),
                                    f32::from(ev.position().x),
                                    f32::from(ev.position().y),
                                    cx,
                                );
                            });
                        })
                        .child(icon(IconId::EllipsisVertical, 15., rgba(FG(), 0.7)))
                },
            );

        if let Some(nt) = note_type {
            header = header.child(
                div()
                    .id("entry-type-badge")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(rgba(ACCENT(), 0.12))
                    .text_size(px(11.))
                    .text_color(c(ACCENT()))
                    .child(nt.name.clone()),
            );
        }

        div()
            .id("note-view")
            .debug_selector(|| "note-view".into())
            .size_full()
            .overflow_y_scrollbar()
            .p(px(36.))
            .flex()
            .flex_col()
            .gap_4()
            .child(header)
            .child(
                // Read-only badge — M3 embeds the editor; shell-only for now.
                div()
                    .id("note-readonly-badge")
                    .debug_selector(|| "note-readonly-badge".into())
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(rgba(FG(), 0.06))
                    .text_size(px(11.))
                    .text_color(c(MUTED_FG()))
                    .w(px(200.))
                    .child("Чтение — редактирование в M3"),
            )
            .child(
                div()
                    .id("note-body")
                    .flex_1()
                    .text_size(px(14.))
                    .line_height(px(22.))
                    .text_color(c(FG()))
                    .child(if body.trim().is_empty() {
                        "Пустая заметка".to_string()
                    } else {
                        body
                    }),
            )
    }
}
