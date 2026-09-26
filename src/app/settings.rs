//! `SettingsPage` port — General / Export / Trash tabs.
//! Export is UI-only (logic is M8); Trash rows use the Engine trash API
//! (`LoadTrash`/`RestoreEntry`/`DeleteForever`) with Vue confirm semantics.
use gpui::{div, prelude::*, px, Context, SharedString};
use gpui_component::scroll::ScrollableElement;

use memoria_gpui::routes::{Route, SettingsTab};

use super::settings_widgets::{section_header, settings_button, toggle_row};
use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;

impl Memoria {
    /// Ensure trash is loaded when the tab opens (Vue `onMounted(loadTrash)`).
    pub(crate) fn route_changed_settings(&mut self, cx: &mut Context<Self>) {
        if matches!(self.route, Route::Settings(SettingsTab::Trash)) && !self.trash_loaded {
            self.trash_loaded = true;
            self.send(memoria_gpui::store::Command::LoadTrash, cx);
        }
    }

    /// Settings shell: tab strip + active panel.
    pub(crate) fn render_settings(
        &mut self,
        tab: SettingsTab,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tabs = [
            (SettingsTab::General, "Общие", "settings-tab-general"),
            (SettingsTab::Export, "Экспорт", "settings-tab-export"),
            (SettingsTab::Trash, "Корзина", "settings-tab-trash"),
        ];
        let mut strip = div().id("settings-tabs").flex().gap_1();
        for (t, label, sel) in tabs {
            let active = t == tab;
            let weak = cx.weak_entity();
            strip = strip.child(
                div()
                    .id(SharedString::from(format!("settings-tab-{}", t.as_str())))
                    .debug_selector(move || sel.to_string())
                    .a11y_button(label)
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .text_size(px(12.))
                    .cursor_pointer()
                    .bg(if active {
                        rgba(FG(), 0.10)
                    } else {
                        rgba(FG(), 0.)
                    })
                    .text_color(if active { c(FG()) } else { c(MUTED_FG()) })
                    .hover(|s| s.bg(rgba(FG(), 0.08)))
                    .on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| this.navigate(Route::Settings(t), cx));
                    })
                    .child(label),
            );
        }

        let body: gpui::AnyElement = match tab {
            SettingsTab::General => self.render_general_settings(cx).into_any_element(),
            SettingsTab::Export => self.render_export_settings(cx).into_any_element(),
            SettingsTab::Trash => self.render_trash_settings(cx).into_any_element(),
        };

        div()
            .id("settings-page")
            .debug_selector(|| "settings-page".into())
            .size_full()
            .overflow_y_scrollbar()
            .p(px(36.))
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(c(FG()))
                    .child("Настройки"),
            )
            .child(strip)
            .child(body)
    }

    /// `GeneralSettings` — «Редактор» toggles + «Отображаемые типы» list.
    fn render_general_settings(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let spell = self.prefs.preferences.spellcheck_enabled;
        let reader = self.prefs.preferences.reader_mode_enabled;
        let all_ids: Vec<String> = self.note_types.iter().map(|t| t.id.clone()).collect();
        let visible = memoria_gpui::local_state::visible_type_set(
            &self.prefs.visible_object_type_ids,
            &all_ids,
        );
        let mut type_rows = Vec::new();
        for nt in self.note_types.clone() {
            let on = visible.contains(&nt.id);
            let tid = nt.id.clone();
            type_rows.push(toggle_row(
                format!("pref-type-{}", nt.id),
                nt.name.clone(),
                nt.id.clone(),
                on,
                cx.weak_entity(),
                move |this, v| this.set_object_type_visible(&tid, v),
            ));
        }

        let mut list = div().id("settings-general").flex().flex_col().gap_1();
        list = list.child(section_header("Редактор"));
        list = list.child(toggle_row(
            "pref-spellcheck".to_string(),
            "Проверка орфографии".to_string(),
            "Подчёркивает слова с возможными опечатками.".to_string(),
            spell,
            cx.weak_entity(),
            |this, v| this.prefs.preferences.spellcheck_enabled = v,
        ));
        list = list.child(toggle_row(
            "pref-reader-mode".to_string(),
            "Режим чтения".to_string(),
            "Открывать объекты в режиме чтения.".to_string(),
            reader,
            cx.weak_entity(),
            |this, v| this.prefs.preferences.reader_mode_enabled = v,
        ));
        list = list.child(section_header("Отображаемые типы"));
        list = list.children(type_rows);
        list.child(
            div()
                .text_size(px(11.))
                .text_color(c(MUTED_FG()))
                .px_2()
                .pt_1()
                .child("Если включены все типы, Memoria загружает все объекты."),
        )
    }

    /// `setObjectTypeVisible` — normalized: all-visible ⇒ stored list = [].
    pub(crate) fn set_object_type_visible(&mut self, type_id: &str, visible: bool) {
        let all: Vec<String> = self.note_types.iter().map(|t| t.id.clone()).collect();
        let mut set =
            memoria_gpui::local_state::visible_type_set(&self.prefs.visible_object_type_ids, &all);
        if visible {
            set.insert(type_id.to_string());
        } else {
            set.remove(type_id);
        }
        let next: Vec<String> = all.iter().filter(|id| set.contains(*id)).cloned().collect();
        self.prefs.visible_object_type_ids = if next.len() == all.len() {
            Vec::new()
        } else {
            next
        };
        self.persist_prefs();
    }

    /// `ExportSettings` — UI only; buttons surface «M8» toast (no logic).
    fn render_export_settings(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div().id("settings-export").flex().flex_col().gap_1();
        list = list.child(section_header("Обмен с Markdown"));
        for (id, title, desc) in [
            (
                "export-md",
                "Экспорт текущего объекта",
                "YAML frontmatter + Markdown.",
            ),
            (
                "export-vault",
                "Экспорт всех объектов",
                "Obsidian-compatible vault.",
            ),
            (
                "import-vault",
                "Импорт Obsidian vault",
                "Импорт Markdown-файлов из папки.",
            ),
        ] {
            let weak = cx.weak_entity();
            list = list.child(
                div()
                    .id(SharedString::from(format!("settings-{id}")))
                    .debug_selector(move || format!("eden-{id}"))
                    .a11y_button(title)
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_2()
                    .py_2()
                    .rounded_md()
                    .hover(|s| s.bg(rgba(FG(), 0.05)))
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
                    .child(settings_button(
                        id,
                        "Открыть",
                        false,
                        move |_, _, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.toast("Экспорт/импорт появится в M8", cx);
                            });
                        },
                    )),
            );
        }
        list
    }
}
