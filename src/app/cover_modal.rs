//! `BookCoverFileDropzone.vue` + cover modal port — URL save + OS file-drop
//! → `images.storeCover`. All network/filesystem work happens in the Engine
//! via app-network ops; the app only renders Engine-returned local paths.
use gpui::{div, prelude::*, px, Context, Window};
use gpui_component::input::{Input, InputState};
use serde_json::Value;

use memoria_model::object_views::parse_entry_header_props;
use memoria_model::store::Command;

use super::modal::{ghost_btn, modal_panel, modal_shell, primary_btn};
use super::types::CoverModal;
use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;

impl Memoria {
    /// `openCoverModal` — prefills the URL field when the prop already holds
    /// an http(s) ref.
    pub(crate) fn open_cover_modal(
        &mut self,
        image_field_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self.current.clone() else {
            return;
        };
        let props = parse_entry_header_props(&entry);
        let current = props
            .get(image_field_id)
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let prefill = if current.starts_with("http") {
            current
        } else {
            String::new()
        };
        let input = cx.new(|cx| {
            let mut s = InputState::new(window, cx)
                .placeholder("https://example.com/cover.jpg".to_string());
            if !prefill.is_empty() {
                s.set_value(prefill, window, cx);
            }
            s
        });
        self.cover_modal = Some(CoverModal {
            entry_id: entry.id,
            image_field_id: image_field_id.to_string(),
            url_input: input,
            saving: false,
            error: None,
        });
        cx.notify();
    }

    /// `saveCoverUrl` — the prop stores the URL; display resolves through
    /// `images.fetch` (Engine) — no direct app fetch.
    fn save_cover_url(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = self.cover_modal.take() else {
            return;
        };
        let url = modal.url_input.read(cx).value().trim().to_string();
        if !url.starts_with("http://") && !url.starts_with("https://") {
            self.cover_modal = Some(CoverModal {
                error: Some("Вставь ссылку на изображение".into()),
                ..modal
            });
            return;
        }
        self.set_header_prop(&modal.image_field_id, Value::from(url), cx);
    }

    /// Cover modal — URL input + «или» + file dropzone (GPUI `on_drop` over
    /// `ExternalPaths` → `images.storeCover` in the Engine).
    pub(crate) fn render_cover_modal(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let modal = self.cover_modal.as_ref()?;
        let saving = modal.saving;
        let eid = modal.entry_id.clone();
        let weak = cx.weak_entity();
        let dropzone = div()
            .id("cover-dropzone")
            .debug_selector(|| "cover-dropzone".into())
            .a11y_button("Загрузка обложки книги")
            .h(px(96.))
            .rounded_md()
            .border_1()
            .border_color(c(BORDER()))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.))
            .text_color(c(MUTED_FG()))
            .drag_over::<gpui::ExternalPaths>(|s, _, _, _| s.bg(rgba(ACCENT(), 0.12)))
            .on_drop::<gpui::ExternalPaths>(move |paths, _, cx| {
                let _ = weak.update(cx, |this, cx| {
                    let Some(path) = paths.paths().first().cloned() else {
                        return;
                    };
                    if let Some(m) = this.cover_modal.as_mut() {
                        m.saving = true;
                        m.error = None;
                    }
                    this.send(
                        Command::StoreCover {
                            source_path: path.to_string_lossy().to_string(),
                            entry_id: eid.clone(),
                        },
                        cx,
                    );
                    cx.notify();
                });
            })
            .child(if saving {
                "Сохранение…"
            } else {
                "Перетащите файл изображения сюда"
            });

        let url_state = modal.url_input.clone();
        let error = modal.error.clone();
        let panel = modal_panel("cover-modal", "Обложка книги")
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(c(MUTED_FG()))
                    .child("Ссылка на изображение"),
            )
            .child(
                div()
                    .id("cover-url-input")
                    .debug_selector(|| "cover-url-input".into())
                    .child(Input::new(&url_state)),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(c(MUTED_FG()))
                    .child("— или —"),
            )
            .child(dropzone)
            .when_some(error, |d, e| {
                d.child(
                    div()
                        .id("cover-error")
                        .debug_selector(|| "cover-error".into())
                        .text_size(px(12.))
                        .text_color(c(DESTRUCTIVE()))
                        .child(e),
                )
            })
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(ghost_btn("cover-cancel", "Отмена", cx, |this, _| {
                        if !this.cover_modal.as_ref().map(|m| m.saving).unwrap_or(false) {
                            this.cover_modal = None;
                        }
                    }))
                    .child(primary_btn(
                        "cover-save",
                        "Сохранить ссылку",
                        !saving,
                        cx,
                        |this, cx| {
                            this.save_cover_url(cx);
                        },
                    )),
            );
        Some(modal_shell("cover-modal", panel))
    }

    /// `images.storeCover` reply — the prop gets the stored local path.
    pub(crate) fn on_cover_stored(
        &mut self,
        _entry_id: String,
        result: Result<String, String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(path) => {
                let fid = self
                    .cover_modal
                    .as_ref()
                    .map(|m| m.image_field_id.clone())
                    .unwrap_or_else(|| "cover_image".into());
                self.cover_modal = None;
                self.set_header_prop(&fid, Value::from(path), cx);
            }
            Err(e) => {
                if let Some(m) = self.cover_modal.as_mut() {
                    m.saving = false;
                    m.error = Some(e);
                }
                cx.notify();
            }
        }
    }
}
