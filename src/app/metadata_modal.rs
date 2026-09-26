//! `BookMetadataImportModal.vue` template port — source input row, error,
//! preview rows with per-field checkboxes, footer buttons. The load/reply/
//! apply flow lives in `metadata_apply.rs`.
use gpui::{div, prelude::*, px, Context, Window};
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::ScrollableElement;
use serde_json::Value;

use memoria_gpui::book_metadata::{book_metadata_field_value, is_empty_book_value, normalize_isbn};
use memoria_gpui::object_views::parse_entry_header_props;

use super::modal::{ghost_btn, modal_panel, modal_shell, primary_btn};
use super::types::MetadataModal;
use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;

/// Preview row order (Vue `metadataFields`) — label + field id pairs.
pub(crate) const FIELD_LABELS: &[(&str, &str)] = &[
    ("title", "Название"),
    ("author", "Автор"),
    ("cover_image", "Обложка"),
    ("isbn", "ISBN"),
    ("page_count", "Страниц"),
    ("language", "Язык"),
    ("publisher", "Издательство"),
    ("published_date", "Дата издания"),
    ("source_url", "Источник"),
];

impl Memoria {
    /// `openMetadataModal` — prefill source with the stored ISBN.
    pub(crate) fn open_metadata_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.current.clone() else {
            return;
        };
        let props = parse_entry_header_props(&entry);
        let isbn = normalize_isbn(props.get("isbn").unwrap_or(&Value::Null));
        let input = cx.new(|cx| {
            let mut s = InputState::new(window, cx)
                .placeholder("https://www.livelib.ru/book/... или 9780140328721".to_string());
            if !isbn.is_empty() {
                s.set_value(isbn, window, cx);
            }
            s
        });
        self.metadata_modal = Some(MetadataModal {
            entry_id: entry.id,
            source_input: input,
            loading: false,
            error: None,
            metadata: None,
            selected: Vec::new(),
            enrich_isbn: None,
        });
        cx.notify();
    }

    /// The modal overlay — source row, error/hint, preview rows with
    /// checkboxes, footer buttons (Vue `BookMetadataImportModal` template).
    pub(crate) fn render_metadata_modal(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let modal = self.metadata_modal.as_ref()?;
        let loading = modal.loading;
        let source_state = modal.source_input.clone();
        let error = modal.error.clone();
        let metadata = modal.metadata.clone();
        let selected = modal.selected.clone();
        let current_props = self
            .current
            .as_ref()
            .map(parse_entry_header_props)
            .unwrap_or_default();
        let current_title = self
            .current
            .as_ref()
            .map(|e| e.title.clone())
            .unwrap_or_default();

        let mut panel = modal_panel("book-metadata-modal", "Заполнить данные книги")
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(c(MUTED_FG()))
                    .child("Ссылка или ISBN"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .id("book-metadata-source")
                            .debug_selector(|| "book-metadata-source".into())
                            .flex_1()
                            .child(Input::new(&source_state)),
                    )
                    .child(primary_btn(
                        "book-metadata-find",
                        if loading {
                            "Поиск…"
                        } else {
                            "Найти"
                        },
                        !loading,
                        cx,
                        |this, cx| this.metadata_load(cx),
                    )),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(c(MUTED_FG()))
                    .child("Выбранные поля заменят текущие."),
            );
        if let Some(e) = error {
            panel = panel.child(
                div()
                    .id("book-metadata-error")
                    .debug_selector(|| "book-metadata-error".into())
                    .text_size(px(12.))
                    .text_color(c(DESTRUCTIVE()))
                    .child(e),
            );
        }

        if let Some(meta) = &metadata {
            let mut rows = div()
                .id("book-metadata-preview")
                .debug_selector(|| "book-metadata-preview".into())
                .flex()
                .flex_col()
                .gap_1()
                .overflow_y_scrollbar()
                .max_h(px(280.));
            for (key, label) in FIELD_LABELS {
                let value = book_metadata_field_value(meta, key);
                if is_empty_book_value(&value) {
                    continue;
                }
                let already_filled = if *key == "title" {
                    !current_title.trim().is_empty()
                } else {
                    !is_empty_book_value(current_props.get(*key).unwrap_or(&Value::Null))
                };
                let checked = selected.iter().any(|s| s == *key);
                let text = match &value {
                    Value::Number(n) => n.to_string(),
                    v => v.as_str().unwrap_or_default().to_string(),
                };
                let k = (*key).to_string();
                let k_sel = k.clone();
                let weak = cx.weak_entity();
                rows = rows.child(
                    div()
                        .id(gpui::SharedString::from(format!("meta-row-{k}")))
                        .debug_selector(move || format!("meta-row-{k_sel}"))
                        .a11y_switch(format!("Применить поле «{label}»"), checked)
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(FG(), 0.06)))
                        .on_click(move |_, _, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                if let Some(m) = this.metadata_modal.as_mut() {
                                    if let Some(i) = m.selected.iter().position(|s| s == &k) {
                                        m.selected.remove(i);
                                    } else {
                                        m.selected.push(k.clone());
                                    }
                                }
                                cx.notify();
                            });
                        })
                        .child(
                            div()
                                .w(px(14.))
                                .text_size(px(11.))
                                .text_color(c(ACCENT()))
                                .child(if checked { "☑" } else { "☐" }),
                        )
                        .child(
                            div()
                                .w(px(110.))
                                .flex_shrink_0()
                                .text_size(px(12.))
                                .text_color(c(MUTED_FG()))
                                .child(*label),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(px(12.))
                                .text_color(c(FG()))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(text),
                        )
                        .when(already_filled, |d| {
                            d.child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(rgba(FG(), 0.45))
                                    .child("заменит текущее"),
                            )
                        }),
                );
            }
            panel = panel.child(rows);
        }

        let can_apply = metadata.is_some() && !selected.is_empty() && !loading;
        panel = panel.child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(ghost_btn(
                    "metadata-cancel",
                    "Отмена",
                    cx,
                    |this, _| {
                        if !this
                            .metadata_modal
                            .as_ref()
                            .map(|m| m.loading)
                            .unwrap_or(false)
                        {
                            this.metadata_modal = None;
                        }
                    },
                ))
                .child(primary_btn(
                    "metadata-apply",
                    "Применить",
                    can_apply,
                    cx,
                    |this, cx| {
                        this.metadata_apply(cx);
                    },
                )),
        );
        Some(modal_shell("book-metadata", panel))
    }
}
