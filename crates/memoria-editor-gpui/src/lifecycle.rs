//! Focus-adjacent view state: caret blink task, the `Ctrl+K Z` zen chord
//! timer, zoom factor changes, post-edit bookkeeping (autosave debounce),
//! scroll clamping, and the `Focusable` impl.

use gpui::{px, App, Context, FocusHandle, Focusable, SharedString, Subscription, Task};
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

    /// Vue `useKeyboard` ZOOM_STEP=0.1, range 0.5..2.0 (app-side persists
    /// via `memoria-zoom` localStorage — out of scope for the editor crate).
    pub fn zoom_in(&mut self, cx: &mut Context<Self>) {
        self.zoom.0 = (self.zoom.0 + 0.1).min(2.0);
        self.zoom_changed(cx);
    }
    pub fn zoom_out(&mut self, cx: &mut Context<Self>) {
        self.zoom.0 = (self.zoom.0 - 0.1).max(0.5);
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
                    editor.autosaved_rev = editor.core.revision();
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

/// App-level keystroke interceptor for the open language picker — gpui-kit
/// dispatches `capture_key_down` listeners *after* action dispatch, so bound
/// keys (Enter, arrows, Backspace) would leak into the editor before a
/// capture listener could stop them. Interceptors run before binding
/// resolution; `stop_propagation` inside keeps the key out of the document.
pub(crate) fn picker_key_interceptor(cx: &mut Context<MemoriaEditor>) -> Subscription {
    let weak = cx.entity().downgrade();
    cx.intercept_keystrokes(move |event, window, cx| {
        let Some(editor) = weak.upgrade() else {
            return;
        };
        let handled = editor.update(cx, |e, cx| {
            if e.picker.is_none() || !e.focus.is_focused(window) {
                return false;
            }
            let k = &event.keystroke;
            e.picker_key(
                &k.key,
                k.key_char.as_deref(),
                k.modifiers.control || k.modifiers.alt || k.modifiers.platform,
                cx,
            )
        });
        if handled {
            cx.stop_propagation();
        }
    })
}
