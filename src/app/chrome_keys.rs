//! Titlebar / shell keyboard chords (source-size split from chrome.rs).
use gpui::{Context, Window};

use super::Memoria;

impl Memoria {
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
        let ctrl = k.modifiers.control || k.modifiers.platform;
        // Esc dismisses the topmost overlay (Vue Modal/@keydown.esc).
        if k.key == "escape" {
            let mut closed = false;
            if self.prop_picker.is_some() {
                self.prop_picker = None;
                closed = true;
            } else if self.metadata_modal.is_some() {
                self.metadata_modal = None;
                closed = true;
            } else if self.cover_modal.is_some() {
                self.cover_modal = None;
                closed = true;
            }
            if closed {
                cx.notify();
                return;
            }
        }
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
