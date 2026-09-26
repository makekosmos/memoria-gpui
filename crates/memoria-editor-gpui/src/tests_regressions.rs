//! Post-review regression tests — undo isolation, autosave races, picker.

use std::time::Duration;

use gpui::TestAppContext;

use memoria_editor_core::Selection;

use crate::editor::{EditorEvent, AUTOSAVE_DEBOUNCE};
use crate::tests::{collect_events, markdown, open};

#[gpui::test]
async fn note_switch_resets_undo_history(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, "note A");
    vcx.simulate_input("!");
    // Loading note B must not leave A's undo entries armed — Ctrl+Z in B
    // would otherwise resurrect A's text into B's buffer.
    vcx.update(|_w, app| view.update(app, |e, cx| e.set_markdown("note B", cx)));
    vcx.simulate_keystrokes("ctrl-z");
    assert_eq!(markdown(&view, vcx), "note B");
    assert!(!vcx.read(|app| view.read(app).is_dirty()));
}

#[gpui::test]
async fn stale_save_reply_keeps_dirty(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, "v1");
    vcx.simulate_input("a");
    // Debounce fires → autosave emits rev1, dirty clears.
    vcx.executor()
        .advance_clock(AUTOSAVE_DEBOUNCE + Duration::from_millis(50));
    assert!(!vcx.read(|app| view.read(app).is_dirty()));
    // User types while the save reply is in flight.
    vcx.simulate_input("b");
    // The stale `Saved` reply for rev1 must not clear the new dirty flag.
    vcx.update(|_w, app| view.update(app, |e, _| e.mark_saved()));
    assert!(vcx.read(|app| view.read(app).is_dirty()));
}

#[gpui::test]
async fn flush_before_switch_emits_once(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, "draft");
    vcx.simulate_input("!");
    let events = collect_events(vcx, &view);
    // Note switch flushes pending edits synchronously.
    vcx.update(|_w, app| view.update(app, |e, cx| e.flush_autosave(cx)));
    assert!(events
        .borrow()
        .iter()
        .any(|e| matches!(e, EditorEvent::Autosave(m) if m.as_ref() == "draft!")));
    // The already-armed debounce must not emit a second save.
    vcx.executor()
        .advance_clock(AUTOSAVE_DEBOUNCE + Duration::from_millis(50));
    let n = events
        .borrow()
        .iter()
        .filter(|e| matches!(e, EditorEvent::Autosave(_)))
        .count();
    assert_eq!(n, 1);
}

#[gpui::test]
async fn picker_applies_language_to_original_block(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, "```\ncode\n```\n\nafter");
    // Caret inside the block, open the picker.
    vcx.update(|_w, app| view.update(app, |e, _| e.core.set_selection(Selection::caret(5))));
    vcx.simulate_keystrokes("ctrl-shift-l");
    // Type the query — capture-phase keys feed the picker, not the doc.
    vcx.simulate_keystrokes("p y");
    // Caret moves away after the picker opened — Enter must still apply at
    // the recorded block position.
    vcx.update(|_w, app| view.update(app, |e, _| e.core.set_selection(Selection::caret(15))));
    vcx.simulate_keystrokes("enter");
    assert!(markdown(&view, vcx).starts_with("```python\n"));
}

#[gpui::test]
async fn picker_plain_text_clears_language(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, "```rust\nx\n```");
    vcx.update(|_w, app| view.update(app, |e, _| e.core.set_selection(Selection::caret(7))));
    vcx.simulate_keystrokes("ctrl-shift-l");
    // Clear the prefilled "rust" query → «Plain text» is row 0.
    vcx.simulate_keystrokes("backspace backspace backspace backspace");
    vcx.simulate_keystrokes("enter");
    assert_eq!(markdown(&view, vcx), "```\nx\n```");
}

#[gpui::test]
async fn same_language_pick_is_a_noop(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, "```rust\nx\n```");
    vcx.update(|_w, app| view.update(app, |e, _| e.core.set_selection(Selection::caret(7))));
    let events = collect_events(vcx, &view);
    vcx.simulate_keystrokes("ctrl-shift-l");
    // Query prefilled with "rust" → row 0 is Rust again → Enter no-ops.
    vcx.simulate_keystrokes("enter");
    assert_eq!(markdown(&view, vcx), "```rust\nx\n```");
    assert!(!vcx.read(|app| view.read(app).is_dirty()));
    assert!(!events
        .borrow()
        .iter()
        .any(|e| matches!(e, EditorEvent::Edited)));
}

#[test]
fn language_aliases_normalize_to_registry() {
    use crate::languages::registry_name;
    assert_eq!(registry_name("js"), Some("javascript"));
    assert_eq!(registry_name("py"), Some("python"));
    assert_eq!(registry_name("c++"), Some("cpp"));
    assert_eq!(registry_name("yml"), Some("yaml"));
    assert_eq!(registry_name("objc"), Some("objective-c"));
    assert_eq!(registry_name("sh"), Some("bash"));
    assert_eq!(registry_name("klingon"), None);
}
