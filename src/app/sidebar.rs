//! Sidebar — sections (nav/pinned/types/collections) + footer.
use gpui::{div, prelude::*, px, Context, SharedString, WindowControlArea};
use imago_gpui::chrome;

use memoria_model::routes::{Route, SettingsTab};
use memoria_model::sidebar_model::{build_sidebar, IconId, SidebarRow};

use super::{icon, icon_name, Memoria};
use crate::a11y::A11y;
use crate::theme::*;

impl Memoria {
    /// Sidebar: nav + pinned + types + collections + footer (settings/trash).
    pub(crate) fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let collapsed = self.prefs.sidebar_collapsed;
        let sections = build_sidebar(&self.note_types, &self.list, &self.prefs.pinned_entry_ids);
        let mut shell = chrome::sidebar()
            .id("memoria-sidebar")
            .debug_selector(|| "eden-sidebar".into())
            .child(
                chrome::sidebar_titlebar()
                    .window_control_area(WindowControlArea::Drag)
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(c(FG()))
                            .child("Memoria"),
                    ),
            );
        if !collapsed {
            let mut body = chrome::sidebar_body().px_2();
            for section in &sections {
                if let Some(title) = section.title {
                    body = body.child(
                        div()
                            .px_2()
                            .pt_3()
                            .pb_1()
                            .text_size(px(11.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(c(MUTED_FG()))
                            .child(title),
                    );
                }
                for row in &section.rows {
                    body = body.child(self.sidebar_row(row, cx));
                }
            }
            shell = shell.child(body).child(
                chrome::sidebar_footer()
                    .gap_1()
                    .child(self.sidebar_footer_row(
                        "sb-trash",
                        IconId::Trash,
                        "Корзина",
                        Route::Settings(SettingsTab::Trash),
                        cx,
                    ))
                    .child(self.sidebar_footer_row(
                        "sb-settings",
                        IconId::Settings,
                        "Настройки",
                        Route::Settings(SettingsTab::General),
                        cx,
                    )),
            );
        }
        shell
    }

    fn sidebar_row(&mut self, row: &SidebarRow, cx: &mut Context<Self>) -> impl IntoElement {
        let active = match (&self.route, &row.route) {
            (Route::Everything, Route::Everything) => true,
            (Route::Diary, Route::Diary) => true,
            (Route::Collection(a), Route::Collection(b)) => a == b,
            (Route::Entry(a), Route::Entry(b)) => a == b,
            _ => false,
        };
        let route = row.route.clone();
        let weak = cx.weak_entity();
        let mut el = imago_gpui::chrome::SidebarItem::new(SharedString::from(row.id.clone()))
            .icon(icon_name(row.icon))
            .label(row.label.clone())
            .active(active)
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.navigate(route.clone(), cx));
            });
        let _ = &mut el;
        div()
            .id(SharedString::from(format!("{}-wrap", row.id)))
            .debug_selector(move || row.id.clone())
            .a11y_button(row.label.clone())
            .flex()
            .items_center()
            .child(div().flex_1().min_w_0().child(el))
            .when_some(row.count, |d, n| {
                d.child(
                    div()
                        .text_size(px(11.))
                        .text_color(c(MUTED_FG()))
                        .pr_2()
                        .child(n.to_string()),
                )
            })
    }

    fn sidebar_footer_row(
        &mut self,
        id: &'static str,
        icon_id: IconId,
        label: &'static str,
        route: Route,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let weak = cx.weak_entity();
        div()
            .id(id)
            .debug_selector(move || id.to_string())
            .a11y_button(label)
            .flex_1()
            .h(px(30.))
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .rounded_md()
            .text_size(px(12.))
            .text_color(c(MUTED_FG()))
            .cursor_pointer()
            .hover(|s| s.bg(rgba(FG(), 0.08)).text_color(c(FG())))
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.navigate(route.clone(), cx));
            })
            .child(icon(icon_id, 15., rgba(FG(), 0.7)))
            .child(label)
    }
}
