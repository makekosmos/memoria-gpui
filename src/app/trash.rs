//! `TrashSettings` — trash list + restore/delete/empty actions.
use gpui::{div, prelude::*, px, Context, SharedString};

use memoria_model::dates::trash_time_ago_label;
use memoria_model::model::Entry;
use memoria_model::object_views::entry_display_title;
use memoria_model::sidebar_model::IconId;

use super::settings_widgets::{section_header, settings_button, settings_button_danger};
use super::{icon, Memoria};
use crate::theme::*;

impl Memoria {
    /// `TrashSettings` — list + restore/delete/empty.
    pub(crate) fn render_trash_settings(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div().id("settings-trash").flex().flex_col().gap_1();
        list = list.child(section_header("Корзина"));
        if self.trash.is_empty() {
            return list.child(
                div()
                    .id("trash-empty")
                    .debug_selector(|| "trash-empty".into())
                    .px_2()
                    .py_4()
                    .text_size(px(13.))
                    .text_color(c(MUTED_FG()))
                    .child("Корзина пуста"),
            );
        }
        for e in self.trash.clone() {
            list = list.child(self.trash_row(&e, cx));
        }
        let weak = cx.weak_entity();
        list.child(div().pt_3().child(settings_button(
            "trash-empty-all",
            "Очистить корзину",
            true,
            move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.confirm_empty_trash(cx));
            },
        )))
    }

    fn trash_row(&mut self, e: &Entry, cx: &mut Context<Self>) -> impl IntoElement {
        let eid = e.id.clone();
        let eid2 = e.id.clone();
        let weak = cx.weak_entity();
        let weak2 = cx.weak_entity();
        div()
            .id(SharedString::from(format!("trash-item-{}", e.id)))
            .debug_selector(move || format!("trash-item-{}", e.id))
            .flex()
            .items_center()
            .gap_3()
            .px_2()
            .py_2()
            .rounded_md()
            .border_1()
            .border_color(c(BORDER()))
            .child(icon(IconId::FileText, 15., rgba(FG(), 0.6)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(c(FG()))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(entry_display_title(e)),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(c(MUTED_FG()))
                            .child(format!(
                                "Удалено {}",
                                trash_time_ago_label(
                                    e.deleted_at.unwrap_or(e.updated_at),
                                    memoria_model::time::now_millis(),
                                )
                            )),
                    ),
            )
            .child(settings_button(
                "trash-restore",
                "Восстановить",
                false,
                move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| this.restore_trash(eid.clone(), cx));
                },
            ))
            .child(settings_button_danger(
                "trash-delete",
                "Удалить",
                move |_, _, cx| {
                    let _ =
                        weak2.update(cx, |this, cx| this.confirm_delete_forever(eid2.clone(), cx));
                },
            ))
    }

    pub(crate) fn restore_trash(&mut self, id: String, cx: &mut Context<Self>) {
        self.send(memoria_model::store::Command::RestoreEntry(id), cx);
    }
}
