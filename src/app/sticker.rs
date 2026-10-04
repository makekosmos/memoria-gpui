//! Sticker window — `StickerNoteView.vue`/`lib/sticker.ts` port on GPUI-owned
//! windows. A `WindowKind::Floating` window per note key renders the *shared*
//! `NoteDoc` (same title + `MemoriaEditor` buffer as the main window) with no
//! sidebar. Reopening a note's sticker focuses the existing window — Vue gets
//! this from `kepler.window.open`'s keyed-window semantics; we keep a
//! key→window registry on `Memoria`.
//!
//! Platform notes:
//! - `WindowKind::Floating` sets a floating level at creation where the
//!   platform supports it (macOS NSFloatingWindowLevel; Windows topmost-ish).
//! - Vue's pin button toggles `setAlwaysOnTop` at runtime — GPUI has no
//!   runtime window-level API, so the pin renders as a state indicator only
//!   where `sticker_always_on_top_supported()` (PARITY.md «Стикеры» GAP).
//! - Wayland has no always-on-top protocol for regular windows — the sticker
//!   there is an ordinary floating window (documented gap).

use gpui::{
    div, prelude::*, px, Bounds, Context, Entity, MouseButton, SharedString, Subscription, Window,
    WindowBounds, WindowKind, WindowOptions,
};
use gpui_component::input::{Input, InputState};

use memoria_model::sticker_route::{
    sticker_always_on_top_supported, sticker_window_key_for, STICKER_WINDOW_HEIGHT,
    STICKER_WINDOW_MIN_HEIGHT, STICKER_WINDOW_MIN_WIDTH, STICKER_WINDOW_WIDTH,
};

use super::doc::NoteDoc;
use super::Memoria;
use crate::theme::*;

/// One `WindowKind::Floating` window per `stickerWindowKeyFor(entry_id)`;
/// reopening a key focuses the existing window instead of spawning a second.
/// Returns `false` when the window could not be opened.
pub(crate) fn open_sticker_window(
    app: &mut Memoria,
    entry_id: &str,
    window: &mut Window,
    cx: &mut Context<Memoria>,
) -> bool {
    let key = sticker_window_key_for(entry_id);
    if let Some(handle) = app.stickers.get(&key).copied() {
        // Keyed-window reopen: focus the existing sticker. A stale handle
        // (window was closed) drops through to a fresh `open_window`.
        if handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
        {
            return true;
        }
        app.stickers.remove(&key);
    }
    let doc = app.ensure_doc(entry_id, window, cx);
    let entry_title = doc
        .read(cx)
        .entry
        .as_ref()
        .map(|e| e.title.clone())
        .filter(|t| !t.is_empty());
    let window_title = entry_title.unwrap_or_else(|| "Memoria".into());
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            gpui::size(px(STICKER_WINDOW_WIDTH), px(STICKER_WINDOW_HEIGHT)),
            cx,
        ))),
        // No native titlebar — the view draws the compact Vue-style header
        // (title input + pin indicator).
        titlebar: None,
        kind: WindowKind::Floating,
        is_movable: true,
        window_min_size: Some(gpui::size(
            px(STICKER_WINDOW_MIN_WIDTH),
            px(STICKER_WINDOW_MIN_HEIGHT),
        )),
        ..Default::default()
    };
    let Ok(handle) = cx.open_window(options, move |window, cx| {
        window.set_window_title(&window_title);
        window.activate_window();
        let view = cx.new(|cx| StickerNoteView::new(doc, window, cx));
        cx.new(|cx| gpui_component::Root::new(view, window, cx))
    }) else {
        return false;
    };
    app.stickers.insert(key, handle);
    true
}

/// Compact note view for a sticker window: mini titlebar (this window's own
/// title binding over the shared canonical title + pin indicator) and the
/// shared `MemoriaEditor`. No sidebar — parity with the Vue sticker surface.
pub(crate) struct StickerNoteView {
    doc: Entity<NoteDoc>,
    /// This window's title binding — `InputState` can't be shared across
    /// windows (per-view layout is cached on the entity), so the sticker
    /// binds its own input to the doc's canonical `title_text`.
    title: Entity<InputState>,
    _obs: Subscription,
}

impl StickerNoteView {
    pub(crate) fn new(doc: Entity<NoteDoc>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let title = doc.update(cx, |doc, cx| {
            doc.ensure_editor_attached(window, cx);
            doc.title_for(window, cx)
        });
        let _obs = cx.observe(&doc, |_, _, cx| cx.notify());
        Self { doc, title, _obs }
    }
}

impl Render for StickerNoteView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let doc = self.doc.clone();
        doc.update(cx, |doc, cx| doc.apply_pending(window, cx));
        let doc = doc.read(cx);
        let entry_title = doc
            .entry
            .as_ref()
            .map(|e| e.title.clone())
            .unwrap_or_default();

        // Mini titlebar: editable title (bound to the shared canonical
        // string) + always-on-top indicator where the platform supports it.
        let mut titlebar = div()
            .id("sticker-titlebar")
            .h(px(36.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .px_2()
            .gap(px(6.))
            .border_b_1()
            .border_color(c(BORDER()))
            .child(
                div().flex_1().min_w_0().child(
                    Input::new(&self.title)
                        .text_size(px(13.))
                        .text_color(c(FG()))
                        .appearance(false)
                        .bordered(false)
                        .p_0(),
                ),
            );
        if sticker_always_on_top_supported() {
            // `WindowKind::Floating` already gives the floating level — GPUI
            // has no runtime `setAlwaysOnTop`, so this is indicator-only.
            titlebar = titlebar.child(
                div()
                    .id("sticker-pin")
                    .debug_selector(|| "sticker-pin".into())
                    .text_size(px(12.))
                    .text_color(c(MUTED_FG()))
                    .child("📌"),
            );
        }

        let body = if doc.failed {
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(c(WARN()))
                        .child(doc.status.clone().unwrap_or(entry_title)),
                )
                .child(
                    div()
                        .id("close-sticker")
                        .px_3()
                        .py_1()
                        .rounded(px(6.))
                        .bg(c(CARD()))
                        .text_color(c(FG()))
                        .cursor_pointer()
                        .text_size(px(12.))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|_, _, window, _| window.remove_window()),
                        )
                        .child("Закрыть"),
                )
        } else if doc.loading && doc.entry.is_none() {
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .items_center()
                .justify_center()
                .text_color(c(MUTED_FG()))
                .child("Загрузка…")
        } else {
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .p_2()
                .children(doc.status.clone().map(|s| {
                    div()
                        .pt_1()
                        .text_size(px(11.))
                        .text_color(c(MUTED_FG()))
                        .child(SharedString::from(s))
                }))
                // The shared `MemoriaEditor` — one buffer across windows.
                .child(div().flex_1().min_h_0().child(doc.editor.clone()))
        };

        div()
            .id("sticker-root")
            .debug_selector(|| "sticker-root".into())
            .size_full()
            .bg(c(BG()))
            .text_color(c(FG()))
            .flex()
            .flex_col()
            .child(titlebar)
            .child(body)
    }
}
