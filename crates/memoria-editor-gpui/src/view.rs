//! `Render` for `MemoriaEditor` + the keymap. Every hotkey is bound on the
//! *physical* key (`Keystroke.key` — the Linux backend maps Russian letters
//! back to their ASCII keycap, see `guess_ascii` in gpui-pre-linux), matching
//! Vue's `e.code`-based handling in `useKeyboard.ts`.

use gpui::{div, prelude::*, px, Context, KeyBinding, ScrollDelta, ScrollWheelEvent, Window};
use memoria_editor_core::cmd::Command;

use crate::editor::{EditorEvent, MemoriaEditor};
use crate::element::EditorElement;
use crate::style;

gpui::actions!(
    memoria_editor,
    [
        Left,
        Right,
        Up,
        Down,
        Home,
        End,
        DocHome,
        DocEnd,
        WordLeft,
        WordRight,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectHome,
        SelectEnd,
        SelectWordLeft,
        SelectWordRight,
        SelectAll,
        Backspace,
        Delete,
        Enter,
        ShiftEnter,
        Tab,
        Backtab,
        Undo,
        Redo,
        ToggleBold,
        ToggleItalic,
        ToggleCode,
        ToggleStrike,
        OpenLangPicker,
        CtrlK,
        ZenToggle,
        Submit,
        ZoomIn,
        ZoomOut,
        ZoomReset,
    ]
);

/// The key context the editor's listeners live under.
pub const KEY_CONTEXT: &str = "MemoriaEditor";

/// Default key bindings — the app installs these once (`cx.bind_keys`).
/// Physical keys only: `ctrl-k`/`ctrl-alt-z`/`ctrl-b`… fire identically on
/// the Russian layout because the platform maps keycodes to ASCII caps.
pub fn key_bindings() -> Vec<KeyBinding> {
    let c = Some(KEY_CONTEXT);
    vec![
        KeyBinding::new("left", Left, c),
        KeyBinding::new("right", Right, c),
        KeyBinding::new("up", Up, c),
        KeyBinding::new("down", Down, c),
        KeyBinding::new("home", Home, c),
        KeyBinding::new("end", End, c),
        KeyBinding::new("ctrl-home", DocHome, c),
        KeyBinding::new("ctrl-end", DocEnd, c),
        KeyBinding::new("alt-left", WordLeft, c),
        KeyBinding::new("alt-right", WordRight, c),
        KeyBinding::new("ctrl-left", WordLeft, c),
        KeyBinding::new("ctrl-right", WordRight, c), // Linux word jump
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new("shift-up", SelectUp, c),
        KeyBinding::new("shift-down", SelectDown, c),
        KeyBinding::new("shift-home", SelectHome, c),
        KeyBinding::new("shift-end", SelectEnd, c),
        KeyBinding::new("alt-shift-left", SelectWordLeft, c),
        KeyBinding::new("alt-shift-right", SelectWordRight, c),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, c),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, c),
        KeyBinding::new("ctrl-a", SelectAll, c),
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("delete", Delete, c),
        KeyBinding::new("enter", Enter, c),
        KeyBinding::new("shift-enter", ShiftEnter, c),
        KeyBinding::new("tab", Tab, c),
        KeyBinding::new("shift-tab", Backtab, c),
        KeyBinding::new("ctrl-z", Undo, c),
        KeyBinding::new("ctrl-shift-z", Redo, c),
        KeyBinding::new("ctrl-y", Redo, c),
        KeyBinding::new("ctrl-b", ToggleBold, c),
        KeyBinding::new("ctrl-i", ToggleItalic, c),
        KeyBinding::new("ctrl-e", ToggleCode, c),
        KeyBinding::new("ctrl-shift-s", ToggleStrike, c),
        // Ctrl+K arms the zen chord (and emits `CtrlK` for search wiring).
        KeyBinding::new("ctrl-k", CtrlK, c),
        // Legacy Vue alternative: Ctrl+Alt+Z toggles zen directly.
        KeyBinding::new("ctrl-alt-z", ZenToggle, c),
        // Composer submit — `Ctrl`/`Cmd`+`Enter` (Vue `handleKeyDown` →
        // `addDraftBubble`); the handler is a no-op in non-compact embeds.
        KeyBinding::new("ctrl-enter", Submit, c),
        KeyBinding::new("cmd-enter", Submit, c),
        // Code-block language picker («Поиск языка...») — Vue opens it from
        // the block toolbar; M3 binds a physical key instead.
        KeyBinding::new("ctrl-shift-l", OpenLangPicker, c),
        // Zoom — `useKeyboard.ts` ZOOM_STEP table (physical Equal/Minus/0 +
        // numpad add/subtract; numpad 0 arrives as `0`).
        KeyBinding::new("ctrl-=", ZoomIn, c),
        KeyBinding::new("ctrl-add", ZoomIn, c),
        KeyBinding::new("ctrl--", ZoomOut, c),
        KeyBinding::new("ctrl-subtract", ZoomOut, c),
        KeyBinding::new("ctrl-0", ZoomReset, c),
    ]
}

impl Render for MemoriaEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let mut root = div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .size_full()
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(gpui::MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(gpui::MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
            .capture_key_down(cx.listener(Self::capture_key_down))
            .child(EditorElement::new(view));

        macro_rules! wire {
            ($a:ident => $f:expr) => {
                root =
                    root.on_action(cx.listener(move |e: &mut Self, _: &$a, w, cx| ($f)(e, w, cx)));
            };
        }
        wire!(Left => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.move_horiz(true, false, cx));
        wire!(Right => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.move_horiz(false, false, cx));
        wire!(Up => |e: &mut Self, w: &mut Window, cx: &mut Context<Self>| e.move_vert(true, false, w, cx));
        wire!(Down => |e: &mut Self, w: &mut Window, cx: &mut Context<Self>| e.move_vert(false, false, w, cx));
        wire!(Home => |e: &mut Self, w: &mut Window, cx: &mut Context<Self>| e.move_home_end(false, false, w, cx));
        wire!(End => |e: &mut Self, w: &mut Window, cx: &mut Context<Self>| e.move_home_end(true, false, w, cx));
        wire!(DocHome => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.move_doc_edge(false, false, cx));
        wire!(DocEnd => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.move_doc_edge(true, false, cx));
        wire!(WordLeft => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.move_word(true, false, cx));
        wire!(WordRight => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.move_word(false, false, cx));
        wire!(SelectLeft => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.move_horiz(true, true, cx));
        wire!(SelectRight => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.move_horiz(false, true, cx));
        wire!(SelectUp => |e: &mut Self, w: &mut Window, cx: &mut Context<Self>| e.move_vert(true, true, w, cx));
        wire!(SelectDown => |e: &mut Self, w: &mut Window, cx: &mut Context<Self>| e.move_vert(false, true, w, cx));
        wire!(SelectHome => |e: &mut Self, w: &mut Window, cx: &mut Context<Self>| e.move_home_end(false, true, w, cx));
        wire!(SelectEnd => |e: &mut Self, w: &mut Window, cx: &mut Context<Self>| e.move_home_end(true, true, w, cx));
        wire!(SelectWordLeft => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.move_word(true, true, cx));
        wire!(SelectWordRight => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.move_word(false, true, cx));
        wire!(SelectAll => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.select_all(cx));
        wire!(Backspace => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.key_backspace(cx));
        wire!(Delete => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.key_delete(cx));
        wire!(Enter => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.key_enter(cx));
        wire!(ShiftEnter => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.key_shift_enter(cx));
        wire!(Tab => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.key_tab(false, cx));
        wire!(Backtab => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.key_tab(true, cx));
        wire!(Undo => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.undo(cx));
        wire!(Redo => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.redo(cx));
        wire!(ToggleBold => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.command(Command::Bold, cx));
        wire!(ToggleItalic => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.command(Command::Italic, cx));
        wire!(ToggleCode => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.command(Command::InlineCode, cx));
        wire!(ToggleStrike => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.command(Command::Strike, cx));
        wire!(OpenLangPicker => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.open_lang_picker(cx));
        wire!(CtrlK => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.ctrl_k(cx));
        wire!(ZenToggle => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| {
            e.clear_zen_chord();
            if !e.compact {
                cx.emit(EditorEvent::ZenToggled);
            }
        });
        wire!(Submit => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| {
            if e.compact {
                cx.emit(EditorEvent::Submit);
            }
        });
        wire!(ZoomIn => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.zoom_in(cx));
        wire!(ZoomOut => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.zoom_out(cx));
        wire!(ZoomReset => |e: &mut Self, _w: &mut Window, cx: &mut Context<Self>| e.zoom_reset(cx));

        if let Some(picker) = &self.picker {
            root = root.child(picker_overlay(picker));
        }
        root
    }
}

/// Floating language list (styled after `EdenCodeBlockTools`' popover).
fn picker_overlay(p: &crate::picker::LangPicker) -> impl IntoElement {
    let mut list =
        div()
            .absolute()
            .top(px(72.))
            .left(gpui::relative(0.5))
            .w(px(260.))
            .max_h(px(280.))
            .overflow_hidden()
            .bg(style::code_bg())
            .rounded(px(style::CODE_RADIUS))
            .border_1()
            .border_color(style::marker_color())
            .text_color(style::text_color())
            .text_size(px(13.))
            .child(div().px_3().py_2().text_color(style::muted_color()).child(
                if p.query.is_empty() {
                    "Поиск языка...".to_string()
                } else {
                    p.query.clone()
                },
            ));
    for (i, lang) in p.filtered().iter().enumerate().take(40) {
        let mut row = div().px_3().py_1().child(lang.label.to_string());
        if i == p.selected {
            row = row.bg(style::selection_bg());
        }
        list = list.child(row);
    }
    list
}

impl MemoriaEditor {
    /// Capture-phase keys: picker input first, then the zen chord's `z`.
    fn capture_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Picker keys are consumed earlier by the `intercept_keystrokes`
        // hook registered in `MemoriaEditor::new` — it runs before action
        // dispatch, which `capture_key_down` does not in this gpui-kit.
        let k = &event.keystroke;
        // Zen chord: armed `ctrl-k` + plain `z` (Vue CHORD_WINDOW_MS).
        if self.zen_chord_active()
            && k.key == "z"
            && !k.modifiers.control
            && !k.modifiers.alt
            && !k.modifiers.platform
        {
            self.clear_zen_chord();
            cx.emit(EditorEvent::ZenToggled);
            cx.stop_propagation();
            cx.notify();
        }
    }

    /// Wheel scroll — pixel deltas pass through; line deltas × line height.
    fn on_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        let dy = match event.delta {
            ScrollDelta::Pixels(p) => -p.y,
            ScrollDelta::Lines(p) => px(-p.y * style::BODY_SIZE * style::BODY_LH * self.zoom.0),
        };
        crate::geo::apply_scroll(self, dy);
        cx.notify();
    }
}
