//! `window.confirm` equivalent — centered modal for destructive actions.
use gpui::{div, prelude::*, px, Context, MouseButton};
use memoria_model::store::Command;

use super::types::{Confirm, ViewAction};
use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;

impl Memoria {
    /// `window.confirm` equivalent — centered modal with «Отмена»/«Подтвердить».
    pub(crate) fn render_confirm(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let confirm = self.confirm.clone()?;
        let (title, danger_label) = match &confirm {
            Confirm::DeleteEntry(_) => ("Удалить заметку? Она попадёт в корзину.", "Удалить"),
            Confirm::DeleteForever(_) => (
                "Удалить заметку навсегда? Это действие нельзя отменить.",
                "Удалить навсегда",
            ),
            Confirm::EmptyTrash => (
                "Очистить корзину? Все заметки будут удалены навсегда.",
                "Очистить",
            ),
        };

        let btn = |id: &'static str,
                   label: &'static str,
                   danger: bool,
                   f: ViewAction,
                   cx: &mut Context<Self>| {
            let weak = cx.weak_entity();
            let mut el = div()
                .id(id)
                .debug_selector(move || id.to_string())
                .a11y_menu_item(label)
                .px_4()
                .h(px(30.))
                .flex()
                .items_center()
                .rounded_md()
                .text_size(px(13.))
                .cursor_pointer();
            el = if danger {
                el.text_color(c(BG())).bg(c(DESTRUCTIVE()))
            } else {
                el.text_color(c(FG()))
                    .border_1()
                    .border_color(c(BORDER()))
                    .hover(|s| s.bg(rgba(FG(), 0.08)))
            };
            el.on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.confirm = None;
                    f(this, cx);
                });
            })
            .child(label)
        };

        Some(
            div()
                .id("confirm-backdrop")
                .debug_selector(|| "confirm-backdrop".into())
                .absolute()
                .inset_0()
                .occlude()
                .bg(rgba(0x000000, 0.4))
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.confirm = None;
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .id("confirm-dialog")
                        .debug_selector(|| "confirm-dialog".into())
                        .w(px(360.))
                        .bg(c(BG()))
                        .border_1()
                        .border_color(c(BORDER()))
                        .rounded_lg()
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .on_mouse_down(MouseButton::Left, |_, _, _| {})
                        .child(div().text_size(px(13.)).text_color(c(FG())).child(title))
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap_2()
                                .child(btn(
                                    "confirm-cancel",
                                    "Отмена",
                                    false,
                                    Box::new(|_, _| {}),
                                    cx,
                                ))
                                .child(btn(
                                    "confirm-ok",
                                    danger_label,
                                    true,
                                    Box::new(move |this, cx| match &confirm {
                                        Confirm::DeleteEntry(id) => {
                                            this.delete_entry(id.clone(), cx)
                                        }
                                        Confirm::DeleteForever(id) => {
                                            this.send(Command::DeleteForever(id.clone()), cx)
                                        }
                                        Confirm::EmptyTrash => {
                                            let ids: Vec<String> =
                                                this.trash.iter().map(|e| e.id.clone()).collect();
                                            for id in ids {
                                                this.send(Command::DeleteForever(id), cx);
                                            }
                                        }
                                    }),
                                    cx,
                                )),
                        ),
                ),
        )
    }
}
