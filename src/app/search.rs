//! `SearchOverlay` port — Ctrl+K command palette over Engine `search_objects`
//! (`Command::Search`). Debounce 300ms like `useSearch.ts`; ↑↓/Enter/Esc
//! navigation like `CommandPalette`.

use gpui::{div, prelude::*, px, Context, Entity, MouseButton, SharedString, Window};
use gpui_component::input::{Input, InputEvent, InputState};

use memoria_model::object_views::entry_display_title;
use memoria_model::routes::Route;
use memoria_model::store::Command;

use super::types::SEARCH_DEBOUNCE;
use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;

impl Memoria {
    pub(crate) fn search_query(&self) -> String {
        // Mirror of the InputState value — `Reply::Search` replies compare
        // against this snapshot to drop stale results.
        self.search_query_cache.clone()
    }

    pub(crate) fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_open = true;
        self.search_query_cache.clear();
        self.search_results.clear();
        self.search_selected = 0;
        let input = self.ensure_search_input(window, cx);
        input.update(cx, |s, cx| {
            s.set_value("", window, cx);
            s.focus(window, cx);
        });
        cx.notify();
    }

    pub(crate) fn close_search(&mut self, cx: &mut Context<Self>) {
        self.search_open = false;
        self.search_results.clear();
        self.search_selected = 0;
        cx.notify();
    }

    fn ensure_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(state) = &self.search_input {
            return state.clone();
        }
        let state = cx.new(|cx| InputState::new(window, cx).placeholder("Поиск..."));
        self._subs
            .push(cx.subscribe(&state, |this, _, ev: &InputEvent, cx| {
                match ev {
                    InputEvent::Change => this.on_search_query_change(cx),
                    // Enter arrives as PressEnter while the input owns focus —
                    // the root key handler does not see it.
                    InputEvent::PressEnter { .. } => {
                        if let Some(hit) = this.search_results.get(this.search_selected) {
                            let id = hit.entry_id.clone();
                            this.close_search(cx);
                            this.navigate(Route::Entry(id), cx);
                        }
                    }
                    _ => {}
                }
            }));
        self.search_input = Some(state.clone());
        state
    }

    /// `useSearch` debounce — pending query commits after 300ms idle. Demo
    /// backend runs synchronously and skips the wait.
    fn on_search_query_change(&mut self, cx: &mut Context<Self>) {
        let query = self
            .search_input
            .as_ref()
            .map(|s| s.read(cx).value().to_string())
            .unwrap_or_default();
        self.search_query_cache = query.clone();
        self.search_gen += 1;
        let gen = self.search_gen;
        if query.trim().is_empty() {
            self.search_results.clear();
            cx.notify();
            return;
        }
        if self.backend.is_demo() {
            self.send(Command::Search(query), cx);
            return;
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SEARCH_DEBOUNCE).await;
            let _ = this.update(cx, |this, cx| {
                if this.search_gen == gen && this.search_open {
                    this.send(Command::Search(query), cx);
                }
            });
        })
        .detach();
    }

    /// Search overlay key handling — ↑↓ select, Enter opens, Esc closes.
    pub(crate) fn search_key(&mut self, ev: &gpui::KeyDownEvent, cx: &mut Context<Self>) -> bool {
        if !self.search_open {
            return false;
        }
        match ev.keystroke.key.as_str() {
            "escape" => {
                self.close_search(cx);
                true
            }
            "up" => {
                self.search_selected = self.search_selected.saturating_sub(1);
                cx.notify();
                true
            }
            "down" => {
                if self.search_selected + 1 < self.search_results.len() {
                    self.search_selected += 1;
                }
                cx.notify();
                true
            }
            "enter" => {
                if let Some(hit) = self.search_results.get(self.search_selected) {
                    let id = hit.entry_id.clone();
                    self.close_search(cx);
                    self.navigate(Route::Entry(id), cx);
                }
                true
            }
            _ => false,
        }
    }

    /// `SearchOverlay`/`CommandPalette` — fixed top panel + result rows.
    pub(crate) fn render_search_overlay(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        if !self.search_open {
            return None;
        }
        let input = self.ensure_search_input(_window, cx);
        let query = self.search_query();
        let results = self.search_results.clone();
        let has_query = !query.trim().is_empty();

        let mut rows = div().flex().flex_col().py_1();
        if has_query && results.is_empty() {
            rows = rows.child(
                div()
                    .p_4()
                    .text_size(px(13.))
                    .text_color(c(MUTED_FG()))
                    .child("Ничего не найдено"),
            );
        } else if has_query {
            for (i, hit) in results.iter().enumerate() {
                let title = self
                    .list
                    .iter()
                    .find(|e| e.id == hit.entry_id)
                    .map(entry_display_title)
                    .unwrap_or_else(|| "Без названия".into());
                let entry_id = hit.entry_id.clone();
                let weak = cx.weak_entity();
                let selected = i == self.search_selected;
                rows = rows.child(
                    div()
                        .id(SharedString::from(format!("search-hit-{i}")))
                        .debug_selector(move || format!("eden-search-result-item-{i}"))
                        .a11y_button(title.clone())
                        .mx_2()
                        .px_3()
                        .py_2()
                        .rounded_lg()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .cursor_pointer()
                        .when(selected, |d| d.bg(rgba(FG(), 0.08)))
                        .hover(|s| s.bg(rgba(FG(), 0.08)))
                        .on_click(move |_, _, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.close_search(cx);
                                this.navigate(Route::Entry(entry_id.clone()), cx);
                            });
                        })
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(c(FG()))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(title),
                        )
                        .child(crate::app::highlight::highlighted(&hit.text, &query)),
                );
            }
        } else {
            rows = rows.child(
                div()
                    .p_4()
                    .text_size(px(13.))
                    .text_color(c(MUTED_FG()))
                    .child("Начните вводить для поиска"),
            );
        }

        Some(
            div()
                .id("search-backdrop")
                .debug_selector(|| "eden-search-palette".into())
                .absolute()
                .inset_0()
                .occlude()
                .bg(rgba(0x000000, 0.35))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.close_search(cx);
                    }),
                )
                .child(
                    div()
                        .id("search-panel")
                        .occlude()
                        .w(px(560.))
                        .mx_auto()
                        .mt(px(120.))
                        .bg(c(BG()))
                        .border_1()
                        .border_color(c(BORDER()))
                        .rounded_lg()
                        .shadow_lg()
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .on_mouse_down(MouseButton::Left, |_, _, _| {})
                        .child(
                            div()
                                .px_4()
                                .py_1()
                                .border_b_1()
                                .border_color(c(BORDER()))
                                .child(Input::new(&input)),
                        )
                        .child(rows)
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap_4()
                                .border_t_1()
                                .border_color(c(BORDER()))
                                .px_4()
                                .py_3()
                                .text_size(px(11.))
                                .text_color(c(MUTED_FG()))
                                .child("↑↓ навигация")
                                .child("Enter открыть")
                                .child("Esc закрыть"),
                        ),
                ),
        )
    }
}
