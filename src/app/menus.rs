//! Context menu + confirm dialog — the Vue `@click.stop` context menu on
//! Everything cards and the `window.confirm` equivalents for destructive
//! trash actions.
use gpui::{div, prelude::*, px, Context, MouseButton, SharedString};

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

    /// `openStickerWindow` — a floating GPUI window per entry id.
    /// `canOpenInSticker` guards: stickers only render note/book types.
    pub(crate) fn open_sticker(&mut self, entry_id: String, cx: &mut Context<Self>) {
        let type_id = self
            .list
            .iter()
            .find(|e| e.id == entry_id)
            .and_then(|e| e.type_id.clone());
        if !memoria_gpui::sticker_route::can_open_in_sticker(type_id.as_deref()) {
            self.toast("Стикер доступен только для заметок и книг", cx);
            return;
        }
        let key = memoria_gpui::sticker_route::sticker_window_key_for(&entry_id);
        let route = memoria_gpui::sticker_route::sticker_route_for(&entry_id);
        let id = entry_id.clone();
        let bounds = gpui::Bounds::centered(None, gpui::size(px(420.), px(520.)), cx);
        let _ = cx.open_window(
            gpui::WindowOptions {
                window_bounds: Some(gpui::WindowBounds::Windowed(bounds)),
                kind: gpui::WindowKind::Floating,
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some(SharedString::from("Memoria — стикер")),
                    appears_transparent: true,
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |window, cx| {
                let view = cx.new(|cx| super::sticker::StickerView::new(id.clone(), window, cx));
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            },
        );
        let _ = (key, route);
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
                    f: Box<dyn Fn(&mut Memoria, &mut Context<Memoria>)>,
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
                .on_click(move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.ctx_menu = None;
                        f(this, cx);
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
                            Box::new(move |this, cx| this.toggle_pin(&id1, cx)),
                            cx,
                        ))
                        .child(item(
                            "ctx-sticker",
                            "Открыть стикером",
                            Box::new(move |this, cx| this.open_sticker(id2.clone(), cx)),
                            cx,
                        ))
                        .child(item(
                            "ctx-delete",
                            "Удалить",
                            Box::new(move |this, cx| {
                                this.confirm = Some(Confirm::DeleteEntry(id3.clone()));
                                cx.notify();
                            }),
                            cx,
                        )),
                ),
        )
    }
}
