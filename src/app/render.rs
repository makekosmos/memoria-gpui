//! Root render — `DesktopChrome`-equivalent layout: sidebar + content column
//! (titlebar + routed view), then overlays (conflict banner, search, context
//! menu, confirm dialog, toasts). Note routes host the M3 `MemoriaEditor`.
//! Vue `<Transition>` wrappers are a GAP — GPUI has no element transition
//! primitives (see PARITY.md).
use gpui::{div, prelude::*, px, Context, IntoElement, ParentElement, Render, Styled, Window};

use memoria_gpui::routes::Route;

use super::Memoria;
use crate::theme::*;

impl Render for Memoria {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused_once {
            self.focused_once = true;
            self.root_focus.focus(window, cx);
        }

        // Ensure editor/title entities exist and apply any pending fill from
        // Engine replies (`set_value` / `set_markdown` need a `Window`).
        let title_state = self.title_state(window, cx);
        let editor = self.editor_state(window, cx);
        if let Some((title, markdown)) = self.pending_fill.take() {
            title_state.update(cx, |s, cx| s.set_value(title, window, cx));
            // `set_markdown` emits no `Edited` event and does not mark the
            // editor dirty — a programmatic fill isn't a user edit.
            editor.update(cx, |e, cx| e.set_markdown(&markdown, cx));
            self.dirty = false;
        }

        let body: gpui::AnyElement = match self.route.clone() {
            Route::Everything => self.render_everything(cx).into_any_element(),
            Route::Diary => self.render_diary_placeholder().into_any_element(),
            Route::Collection(id) => self.render_collection(&id, cx).into_any_element(),
            Route::Entry(id) => match self.current.clone() {
                Some(e) if e.id == id => {
                    self.render_entry_view(&e, window, cx).into_any_element()
                }
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
            .children(self.render_ctx_menu(cx))
            .children(self.render_confirm(cx))
            .child(self.render_toasts(cx))
    }
}

impl Memoria {
    /// Diary — M6 milestone; App.vue routes it to `BubbleDiaryView`.
    fn render_diary_placeholder(&self) -> impl IntoElement {
        self.missing_view("Дневник появится в M6")
    }

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
