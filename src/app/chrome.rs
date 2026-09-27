//! Shell chrome — `DesktopChrome`/`Titlebar` pattern via imago-gpui
//! `chrome::*` primitives: sidebar shell, drag titlebar with history buttons,
//! search + settings triggers, native window controls on Linux/Windows.
use gpui::{div, prelude::*, px, ClickEvent, Context, Window, WindowControlArea};
use imago_gpui::chrome;

use memoria_gpui::routes::{Route, SettingsTab};
use memoria_gpui::sidebar_model::IconId;

use super::{icon, Memoria};
use crate::a11y::A11y;
use crate::theme::*;

fn titlebar_button(
    id: &'static str,
    icon_id: IconId,
    a11y: &'static str,
    enabled: bool,
    cx: &mut Context<Memoria>,
    f: impl Fn(&mut Memoria, &mut Window, &mut Context<Memoria>) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let weak = cx.weak_entity();
    let glyph = if enabled {
        rgba(FG(), 0.82)
    } else {
        rgba(FG(), 0.3)
    };
    let mut el = div()
        .id(id)
        .debug_selector(move || id.to_string())
        .w(px(28.))
        .h(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_md();
    if enabled {
        el = el
            .cursor_pointer()
            .a11y_button(a11y)
            .hover(|s| s.bg(rgba(FG(), 0.08)))
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |this, cx| f(this, window, cx));
            });
    }
    el.child(icon(icon_id, 15., glyph))
}

impl Memoria {
    /// Content titlebar: drag region + back/forward + search + settings.
    /// Mirrors `Titlebar.vue` (history buttons, center slot, trailing slot).
    pub(crate) fn render_titlebar(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let can_back = self.history.can_go_back();
        let can_fwd = self.history.can_go_forward();
        let unresolved =
            memoria_gpui::entry_conflicts::unresolved_entry_conflicts(&self.conflicts.conflicts)
                .len();

        let mut bar = chrome::titlebar()
            .id("titlebar")
            .debug_selector(|| "memoria-titlebar".into())
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(titlebar_button(
                        "nav-back",
                        IconId::ChevronLeft,
                        "Назад",
                        can_back,
                        cx,
                        |this, _, cx| this.go_back(cx),
                    ))
                    .child(titlebar_button(
                        "nav-forward",
                        IconId::ChevronRight,
                        "Вперёд",
                        can_fwd,
                        cx,
                        |this, _, cx| this.go_forward(cx),
                    )),
            )
            .child(
                div()
                    .id("titlebar-drag")
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .window_control_area(WindowControlArea::Drag)
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(c(MUTED_FG()))
                            .child(self.titlebar_title()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .when(unresolved > 0, |d| {
                        d.child(
                            div()
                                .id("entry-conflict-count")
                                .debug_selector(|| "entry-conflict-count".into())
                                .text_size(px(11.))
                                .text_color(c(WARN()))
                                .child(format!("Конфликты: {unresolved}")),
                        )
                    })
                    .when(matches!(self.route, Route::Diary) && !self.zen, |d| {
                        // Vue `TitlebarButton` — CalendarDays, `aria-pressed`.
                        let open = self.diary_calendar_open;
                        d.child(
                            div()
                                .id("titlebar-diary-calendar-toggle")
                                .debug_selector(|| "titlebar-diary-calendar-toggle".into())
                                .a11y_switch("Календарь", open)
                                .w(px(28.))
                                .h(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .cursor_pointer()
                                .when(open, |b| b.bg(rgba(FG(), 0.10)))
                                .hover(|s| s.bg(rgba(FG(), 0.08)))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.diary_calendar_open = !this.diary_calendar_open;
                                    cx.notify();
                                }))
                                .child(icon(IconId::Calendar, 15., rgba(FG(), 0.82))),
                        )
                    })
                    .child(titlebar_button(
                        "titlebar-search",
                        IconId::Search,
                        "Поиск",
                        true,
                        cx,
                        |this, window, cx| this.open_search(window, cx),
                    ))
                    .child(titlebar_button(
                        "titlebar-settings",
                        IconId::Settings,
                        "Настройки",
                        true,
                        cx,
                        |this, _, cx| this.navigate(Route::Settings(SettingsTab::General), cx),
                    )),
            );

        #[cfg(target_os = "linux")]
        {
            bar = bar.child(self.render_window_controls(cx));
        }
        #[cfg(target_os = "macos")]
        {
            let _ = window;
        }
        bar
    }

    /// Title shown in the drag area — Vue shows the entry/collection title.
    fn titlebar_title(&self) -> String {
        match &self.route {
            Route::Everything => "Всё".into(),
            Route::Diary => "Дневник".into(),
            Route::Collection(t) => self
                .note_types
                .iter()
                .find(|nt| &nt.id == t)
                .map(|nt| memoria_gpui::note_types::get_note_type_collection_name(Some(nt)))
                .unwrap_or_else(|| "Коллекция".into()),
            Route::Entry(_) => self
                .current
                .as_ref()
                .map(memoria_gpui::object_views::entry_display_title)
                .unwrap_or_default(),
            Route::Settings(_) => "Настройки".into(),
        }
    }

    /// Linux window caption buttons (macOS uses traffic lights instead).
    #[cfg(not(target_os = "macos"))]
    fn render_window_controls(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let specs: [(&'static str, IconId, &'static str, fn(&mut Window)); 3] = [
            ("win-min", IconId::Minus, "Свернуть", |w| {
                w.minimize_window()
            }),
            ("win-max", IconId::Maximize2, "Развернуть", |w| {
                w.zoom_window()
            }),
            ("win-close", IconId::X, "Закрыть", |w| {
                w.remove_window()
            }),
        ];
        let mut row = div().flex().flex_none().h_full();
        for (id, ic, name, action) in specs {
            let danger = id == "win-close";
            row = row.child(
                div()
                    .id(id)
                    .debug_selector(move || id.to_string())
                    .a11y_button(name)
                    .w(px(40.))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(move |s| {
                        s.bg(if danger {
                            rgba(0xe81123, 1.0)
                        } else {
                            rgba(FG(), 0.10)
                        })
                    })
                    .on_click(move |_: &ClickEvent, window, _| action(window))
                    .child(icon(ic, 14., rgba(FG(), 0.82))),
            );
        }
        row
    }

    /// Ctrl+K / Esc / ←→ history — physical keys (layout-independent).
    pub(crate) fn on_key(
        &mut self,
        ev: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.search_key(ev, cx) {
            return;
        }
        let k = &ev.keystroke;
        // Vue `Dropdown` Esc-dismiss for the kind menu.
        if k.key == "escape" && self.kind_menu_for.is_some() {
            self.kind_menu_for = None;
            cx.notify();
            return;
        }
        let ctrl = k.modifiers.control || k.modifiers.platform;
        if ctrl && k.key == "k" {
            if self.search_open {
                self.close_search(cx);
            } else {
                self.open_search(window, cx);
            }
            return;
        }
        if ctrl && k.key == "b" {
            self.prefs.sidebar_collapsed = !self.prefs.sidebar_collapsed;
            self.persist_prefs();
            cx.notify();
            return;
        }
        // History: Alt+← / Alt+→ (desktop convention; Vue binds mouse side
        // buttons + these keys through the platform bridge).
        if k.modifiers.alt && k.key == "left" {
            self.go_back(cx);
        } else if k.modifiers.alt && k.key == "right" {
            self.go_forward(cx);
        }
    }
}
