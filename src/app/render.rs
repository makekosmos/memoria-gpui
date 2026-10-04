//! Root render — `DesktopChrome`-equivalent layout: sidebar + content column
//! (titlebar + routed view), then overlays (conflict banner, search, context
//! menu, confirm dialog, toasts). Note routes host the M3 `MemoriaEditor`.
//! Vue `<Transition>` wrappers are a GAP — GPUI has no element transition
//! primitives (see PARITY.md).
use gpui::{div, prelude::*, px, Context, IntoElement, ParentElement, Render, Styled, Window};

use memoria_model::routes::Route;

use super::Memoria;
use crate::theme::*;

impl Render for Memoria {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused_once {
            self.focused_once = true;
            self.root_focus.focus(window, cx);
        }

        // Sync the shared doc for the current entry — creates the doc's
        // editor + this window's title binding on first paint and pushes the
        // canonical title into the binding (`set_value` needs a `Window`).
        if let Some(entry) = self.pending_fill.take() {
            let markdown = memoria_model::content::read_entry_markdown(
                &serde_json::from_str(&entry.content_json).unwrap_or(serde_json::Value::Null),
            );
            let doc = self.ensure_doc(&entry.id, window, cx);
            doc.update(cx, |doc, cx| doc.apply_fill(&entry, &markdown, cx));
            self.dirty = false;
        }
        self.sync_editor_entities(window, cx);

        // `composerEditor.clearContent()` + reply reset deferred from bubble
        // replies. An open *edit* session keeps its text (Vue `editText` is
        // component-local and survives a sibling bubble create).
        if self.pending_diary_reset {
            self.pending_diary_reset = false;
            if let Some(composer) = &self.diary_composer {
                composer.update(cx, |e, cx| e.set_markdown("", cx));
            }
            if let Some(input) = &self.reply_input {
                input.update(cx, |s, cx| s.set_value("", window, cx));
            }
        }

        let body: gpui::AnyElement = match self.route.clone() {
            Route::Everything => self.render_everything(cx).into_any_element(),
            Route::Diary => self.render_diary(window, cx).into_any_element(),
            Route::Collection(id) => self.render_collection(&id, cx).into_any_element(),
            Route::Entry(id) => match self.current.clone() {
                Some(e) if e.id == id => self.render_entry_view(&e, window, cx).into_any_element(),
                Some(_) | None if self.loading_entry.as_deref() == Some(id.as_str()) => {
                    self.missing_view("Загрузка…").into_any_element()
                }
                _ => self.missing_view("Объект не найден").into_any_element(),
            },
            Route::Settings(tab) => self.render_settings(tab, cx).into_any_element(),
        };

        let content = div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .min_w_0()
            .bg(c(BG()))
            .child(self.render_titlebar(window, cx))
            .children(self.render_conflict_banner(cx))
            .child(div().flex_1().min_h_0().relative().child(body));

        div()
            .id("memoria-root")
            .debug_selector(|| "memoria-root".into())
            .size_full()
            .flex()
            .bg(c(BG()))
            .text_color(c(FG()))
            .font_family("Manrope")
            .track_focus(&self.root_focus)
            .on_key_down(cx.listener(Self::on_key))
            .when(!self.zen, |d| d.child(self.render_sidebar(cx)))
            .child(content)
            .children(self.render_search_overlay(window, cx))
            .children(self.render_ctx_menu(window, cx))
            .children(self.render_kind_menu(cx))
            .children(self.render_prop_picker(window, cx))
            .children(self.render_cover_modal(cx))
            .children(self.render_metadata_modal(cx))
            .children(self.render_confirm(cx))
            .child(self.render_toasts(cx))
    }
}

impl Memoria {
    /// Empty/loading state shared by missing routes.
    fn missing_view(&self, text: &str) -> impl IntoElement {
        div()
            .id("missing-view")
            .debug_selector(|| "empty-editor-state".into())
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(c(MUTED_FG()))
                    .child(text.to_string()),
            )
    }
}
