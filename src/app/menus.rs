//! Context menu + confirm dialog — the Vue `@click.stop` context menu on
//! Everything cards and the `window.confirm` equivalents for destructive
//! trash actions.
use gpui::{div, prelude::*, px, Context, MouseButton, Window};

use super::types::{Confirm, CtxMenu};
use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;

impl Memoria {
    /// Open the card/row context menu at the mouse position.
    pub(crate) fn open_ctx_menu(
        &mut self,
        entry_id: String,
        x: f32,
        y: f32,
        cx: &mut Context<Self>,
    ) {
        let pinned = self.prefs.pinned_entry_ids.contains(&entry_id);
        self.ctx_menu = Some(CtxMenu {
            x,
            y,
            entry_id,
            pinned,
        });
        cx.notify();
    }

    pub(crate) fn toggle_pin(&mut self, entry_id: &str, cx: &mut Context<Self>) {
        if let Some(pos) = self
            .prefs
            .pinned_entry_ids
            .iter()
            .position(|p| p == entry_id)
        {
            self.prefs.pinned_entry_ids.remove(pos);
            self.toast("Откреплено", cx);
        } else {
            self.prefs.pinned_entry_ids.push(entry_id.to_string());
            self.toast("Закреплено", cx);
        }
        self.persist_prefs();
        self.ctx_menu = None;
        cx.notify();
    }

    /// `openStickerWindow` — one floating GPUI window per note key; reopening
    /// focuses the existing window (`menus.rs` keeps the key→window registry
    /// — `kepler.window.open` keyed semantics). `canOpenInSticker` guards:
    /// stickers only render note/book types.
    pub(crate) fn open_sticker(
        &mut self,
        entry_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let type_id = self
            .list
            .iter()
            .find(|e| e.id == entry_id)
            .and_then(|e| e.type_id.clone())
            .or_else(|| {
                self.current
                    .as_ref()
                    .filter(|e| e.id == entry_id)
                    .and_then(|e| e.type_id.clone())
            });
        if !memoria_gpui::sticker_route::can_open_in_sticker(type_id.as_deref()) {
            self.toast("Стикер доступен только для заметок и книг", cx);
            return;
        }
        if !super::sticker::open_sticker_window(self, &entry_id, window, cx) {
            self.toast("Не удалось открыть стикер", cx);
        }
    }

    /// `handlePermanentDelete` — confirm first (Vue `window.confirm`).
    pub(crate) fn confirm_delete_forever(&mut self, id: String, cx: &mut Context<Self>) {
        self.confirm = Some(Confirm::DeleteForever(id));
        cx.notify();
    }

    /// `handleEmptyTrash` — confirm then per-entry permanent delete.
    pub(crate) fn confirm_empty_trash(&mut self, cx: &mut Context<Self>) {
        self.confirm = Some(Confirm::EmptyTrash);
        cx.notify();
    }

    /// Context menu overlay: «Закрепить/Открепить», «Открыть стикером»,
    /// «Удалить». Backdrop click dismisses.
    pub(crate) fn render_ctx_menu(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let menu = self.ctx_menu.clone()?;
        let entry_id = menu.entry_id.clone();

        let item = |id: &'static str,
                    label: &'static str,
                    f: Box<dyn Fn(&mut Memoria, &mut Window, &mut Context<Memoria>)>,
                    cx: &mut Context<Self>| {
            let weak = cx.weak_entity();
            div()
                .id(id)
                .debug_selector(move || id.to_string())
                .a11y_menu_item(label)
                .px_3()
                .h(px(30.))
                .flex()
                .items_center()
                .rounded_md()
                .text_size(px(13.))
                .text_color(c(FG()))
                .cursor_pointer()
                .hover(|s| s.bg(rgba(FG(), 0.08)))
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.ctx_menu = None;
                        f(this, window, cx);
                    });
                })
                .child(label)
        };

        let id1 = entry_id.clone();
        let id2 = entry_id.clone();
        let id3 = entry_id.clone();
        Some(
            div()
                .id("ctx-backdrop")
                .debug_selector(|| "ctx-backdrop".into())
                .absolute()
                .inset_0()
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.ctx_menu = None;
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .id("ctx-menu")
                        .debug_selector(|| "ctx-menu".into())
                        .absolute()
                        .occlude()
                        .left(px(menu.x))
                        .top(px(menu.y))
                        .min_w(px(180.))
                        .bg(c(SIDEBAR_BG()))
                        .border_1()
                        .border_color(c(BORDER()))
                        .rounded_lg()
                        .p_1()
                        .flex()
                        .flex_col()
                        .shadow_lg()
                        .child(item(
                            "ctx-pin",
                            if menu.pinned {
                                "Открепить"
                            } else {
                                "Закрепить"
                            },
                            Box::new(move |this, _window, cx| this.toggle_pin(&id1, cx)),
                            cx,
                        ))
                        .child(item(
                            "ctx-sticker",
                            "Открыть стикером",
                            Box::new(move |this, window, cx| {
                                this.open_sticker(id2.clone(), window, cx)
                            }),
                            cx,
                        ))
                        .child(item(
                            "ctx-delete",
                            "Удалить",
                            Box::new(move |this, _window, cx| {
                                this.confirm = Some(Confirm::DeleteEntry(id3.clone()));
                                cx.notify();
                            }),
                            cx,
                        )),
                ),
        )
    }
}
