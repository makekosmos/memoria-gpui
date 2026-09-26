//! Focus-adjacent view state: caret blink task, the `Ctrl+K Z` zen chord
//! timer, zoom factor changes, post-edit bookkeeping (autosave debounce),
//! scroll clamping, and the `Focusable` impl.

use gpui::{px, App, Context, FocusHandle, Focusable, SharedString, Task};
use std::time::Duration;

use crate::editor::{EditorEvent, MemoriaEditor, AUTOSAVE_DEBOUNCE};
use crate::style::{self, EditorScale};

/// Vue `autosave` blink parity — caret toggles every 500 ms (browsers ~530 ms,
/// GPUI example uses 500 ms).
pub fn blink_task(cx: &mut Context<MemoriaEditor>) -> Task<()> {
    cx.spawn(async move |this, cx| loop {
        cx.background_executor()
            .timer(Duration::from_millis(style::BLINK_MS))
            .await;
        if this
            .update(cx, |e, cx| {
                e.cursor_visible = !e.cursor_visible;
                cx.notify();
            })
            .is_err()
        {
            break;
        }
    })
}

impl MemoriaEditor {
    /// `Ctrl+K` prefix armed → next `z` toggles zen within the timeout.
    pub(crate) fn arm_zen_chord(&mut self, cx: &mut Context<Self>) {
        self.zen_armed = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(crate::editor::ZEN_CHORD_TIMEOUT)
                .await;
            this.update(cx, |e, _| e.zen_armed = None).ok();
        }));
    }

    /// True while `z` can still complete the `ctrl-k z` chord.
    pub(crate) fn zen_chord_active(&self) -> bool {
        self.zen_armed.is_some()
    }

    pub(crate) fn clear_zen_chord(&mut self) {
        self.zen_armed = None;
    }

    /// Vue `zoomSet` steps — the app scales the whole window; the editor
    /// scales text metrics only (chrome is the app's job).
    pub fn zoom_in(&mut self, cx: &mut Context<Self>) {
        self.zoom.0 = (self.zoom.0 * 1.1).min(3.0);
        self.zoom_changed(cx);
    }
    pub fn zoom_out(&mut self, cx: &mut Context<Self>) {
        self.zoom.0 = (self.zoom.0 / 1.1).max(0.5);
        self.zoom_changed(cx);
    }
    pub fn zoom_reset(&mut self, cx: &mut Context<Self>) {
        self.zoom = EditorScale::default();
        self.zoom_changed(cx);
    }
    pub fn zoom_factor(&self) -> f32 {
        self.zoom.0
    }

    fn zoom_changed(&mut self, cx: &mut Context<Self>) {
        self.shaped.clear();
        self.relayout();
        cx.emit(EditorEvent::ZoomChanged(self.zoom.0));
        cx.notify();
    }

    // ---- edits -------------------------------------------------------------

    /// Post-edit bookkeeping: rows invalidate lazily via `rev`, but notify,
    /// blink-reset, autosave scheduling and `Edited` happen here.
    pub(crate) fn after_edit(&mut self, content_changed: bool, cx: &mut Context<Self>) {
        self.cursor_visible = true;
        self.follow_caret = true;
        if content_changed {
            self.dirty = true;
            self.schedule_save(cx);
            cx.emit(EditorEvent::Edited);
        } else {
            cx.emit(EditorEvent::SelectionChanged);
        }
        cx.notify();
    }

    /// Re-arm the autosave debounce (Vue: 300 ms after the last edit).
    fn schedule_save(&mut self, cx: &mut Context<Self>) {
        self.save_task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(AUTOSAVE_DEBOUNCE).await;
            this.update(cx, |editor, cx| {
                if editor.dirty {
                    let md: SharedString = editor.markdown().into();
                    cx.emit(EditorEvent::Autosave(md));
                    editor.dirty = false;
                }
            })
            .ok();
        });
    }

    /// Scroll clamped to `[0, content_h - viewport]`.
    pub(crate) fn clamp_scroll(&mut self) {
        let max = (self.layout.content_h - self.bounds.size.height).max(px(0.));
        self.scroll_y = self.scroll_y.clamp(px(0.), max.max(px(0.)));
    }
}

impl Focusable for MemoriaEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}
