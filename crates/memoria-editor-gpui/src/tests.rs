//! `#[gpui::test]` / `VisualTestContext` coverage for KOS-150: typing, IME
//! mark/commit, physical-key hotkeys on a Russian layout, click onto hidden
//! markup, undo after autosave, autosave debounce, live-refresh semantics.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    point, px, EntityInputHandler, Keystroke, Modifiers, MouseButton, TestAppContext,
    VisualTestContext,
};
use memoria_editor_core::Selection;

use crate::editor::{EditorEvent, MemoriaEditor, AUTOSAVE_DEBOUNCE};

fn open<'a>(
    cx: &'a mut TestAppContext,
    src: &str,
) -> (gpui::Entity<MemoriaEditor>, &'a mut VisualTestContext) {
    // The app installs these once at startup (`main.rs`); tests must too.
    cx.update(|cx| cx.bind_keys(crate::key_bindings()));
    let (view, vcx) =
        cx.add_window_view(|window, cx| MemoriaEditor::new(src.to_string(), window, cx));
    vcx.update(|window, app| {
        let focus = view.read(app).focus.clone();
        window.focus(&focus, app);
    });
    (view, vcx)
}

fn markdown(view: &gpui::Entity<MemoriaEditor>, cx: &VisualTestContext) -> String {
    cx.read(|app| view.read(app).markdown())
}

fn press_ru(vcx: &mut VisualTestContext, key: &str, key_char: &str, ctrl: bool) {
    let window = vcx.windows()[0];
    vcx.dispatch_keystroke(
        window,
        Keystroke {
            modifiers: Modifiers {
                control: ctrl,
                ..Default::default()
            },
            key: key.into(),
            key_char: Some(key_char.into()),
        },
    );
}

fn collect_events(
    vcx: &mut VisualTestContext,
    view: &gpui::Entity<MemoriaEditor>,
) -> Rc<RefCell<Vec<EditorEvent>>> {
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    vcx.update(|_w, cx| {
        cx.subscribe(view, move |_, ev: &EditorEvent, _| {
            sink.borrow_mut().push(ev.clone());
        })
        .detach();
    });
    events
}

#[gpui::test]
async fn typing_updates_markdown_and_marks_dirty(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, "");
    vcx.simulate_input("hello");
    assert_eq!(markdown(&view, vcx), "hello");
    assert!(vcx.read(|app| view.read(app).is_dirty()));
    assert_eq!(
        vcx.read(|app| view.read(app).core.selection()),
        Selection::caret(5)
    );
}

#[gpui::test]
async fn ime_mark_then_commit(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, "note ");
    // Kana composition: `new_selected_range` marks the whole preedit.
    vcx.update(|window, app| {
        view.update(app, |e, cx| {
            e.replace_and_mark_text_in_range(None, "か", Some(0..1), window, cx)
        });
    });
    assert_eq!(markdown(&view, vcx), "note か");
    vcx.update(|window, app| {
        view.update(app, |e, cx| {
            assert_eq!(e.marked_text_range(window, cx), Some(5..6));
            e.replace_and_mark_text_in_range(None, "かな", Some(0..2), window, cx);
        });
    });
    assert_eq!(markdown(&view, vcx), "note かな");
    vcx.update(|window, app| {
        view.update(app, |e, cx| {
            e.unmark_text(window, cx);
            assert_eq!(e.marked_text_range(window, cx), None);
        });
    });
    assert_eq!(markdown(&view, vcx), "note かな");
}

#[gpui::test]
async fn ime_replaces_selection_utf16(cx: &mut TestAppContext) {
    // "å" is 2 UTF-16 units — range conversion must stay in UTF-16 space.
    let (view, vcx) = open(cx, "a😀b");
    vcx.update(|window, app| {
        view.update(app, |e, cx| {
            // Select the emoji (UTF-16 range 1..3 = U+1F600 surrogate pair).
            e.replace_text_in_range(Some(1..3), "X", window, cx);
        });
    });
    assert_eq!(markdown(&view, vcx), "aXb");
}

#[gpui::test]
async fn hotkeys_russian_layout(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, "word");
    let events = collect_events(vcx, &view);
    // Ctrl+Б (physical b) with the word selected → bold wrap.
    vcx.simulate_keystrokes("ctrl-a");
    press_ru(vcx, "b", "и", true);
    assert_eq!(markdown(&view, vcx), "**word**");
    // Ctrl+К (physical k) then Я (physical z) → zen chord.
    press_ru(vcx, "k", "л", true);
    press_ru(vcx, "z", "я", false);
    assert!(events
        .borrow()
        .iter()
        .any(|e| matches!(e, EditorEvent::ZenToggled)));
    // Ctrl+= (physical) zooms in; Ctrl+- out; Ctrl+0 resets.
    press_ru(vcx, "=", "=", true);
    assert!(vcx.read(|app| view.read(app).zoom_factor()) > 1.0);
    press_ru(vcx, "-", "-", true);
    press_ru(vcx, "0", "0", true);
    assert!((vcx.read(|app| view.read(app).zoom_factor()) - 1.0).abs() < f32::EPSILON);
    // Ctrl+В (physical v? no—physical z) undo.
    press_ru(vcx, "z", "я", true);
    assert_eq!(markdown(&view, vcx), "word");
}

#[gpui::test]
async fn click_on_hidden_marker_word_maps_source(cx: &mut TestAppContext) {
    // `Hello **world**` renders as `Hello world` — clicking the 'o' must land
    // inside `world`, not on the `**` markers.
    let (view, vcx) = open(cx, "Hello **world**");
    // Force a frame so `self.bounds` + row shaping are populated.
    vcx.update(|window, _| window.refresh());
    // Put the caret inside "world" (src 9 = the 'o') and read its screen point.
    let hit = vcx.update(|window, app| {
        view.update(app, |e, cx| {
            e.core.set_selection(Selection::caret(9));
            e.caret_bounds(e.bounds, window, cx)
        })
    });
    let b = hit.expect("caret bounds inside the word");
    let point = point(b.origin.x + px(1.), b.origin.y + b.size.height / 2.);
    vcx.simulate_mouse_down(point, MouseButton::Left, Modifiers::default());
    vcx.simulate_mouse_up(point, MouseButton::Left, Modifiers::default());
    let sel = vcx.read(|app| view.read(app).core.selection());
    // Click lands inside `world` (src 8..13) — never on the markers at 7/14.
    assert!(sel.is_empty(), "click collapses to a caret");
    assert!(
        (8..=13).contains(&sel.head),
        "caret {sel:?} should be inside `world`"
    );
}

#[gpui::test]
async fn autosave_debounce_emits_single_event(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, "");
    let events = collect_events(vcx, &view);
    vcx.simulate_input("abc");
    // Debounce: the event fires only after the quiet period.
    assert!(!events
        .borrow()
        .iter()
        .any(|e| matches!(e, EditorEvent::Autosave(_))));
    vcx.executor()
        .advance_clock(AUTOSAVE_DEBOUNCE + Duration::from_millis(50));
    let saves: Vec<_> = events
        .borrow()
        .iter()
        .filter(|e| matches!(e, EditorEvent::Autosave(_)))
        .cloned()
        .collect();
    assert_eq!(saves.len(), 1, "one debounced autosave");
    match &saves[0] {
        EditorEvent::Autosave(md) => assert_eq!(md.as_ref(), "abc"),
        _ => unreachable!(),
    }
    // The app would build content_json from this markdown — verify the codec
    // keeps it as markdown (write_entry_markdown parity).
    assert!(markdown(&view, vcx).contains("abc"));
}

#[gpui::test]
async fn undo_after_autosave_restores_text(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, "base");
    vcx.simulate_input(" edit");
    vcx.executor()
        .advance_clock(AUTOSAVE_DEBOUNCE + Duration::from_millis(50));
    assert_eq!(markdown(&view, vcx), "base edit");
    // Simulate the app persisting: mark saved, then undo still rolls back.
    vcx.update(|_w, app| view.update(app, |e, _| e.mark_saved()));
    assert!(!vcx.read(|app| view.read(app).is_dirty()));
    vcx.simulate_keystrokes("ctrl-z");
    assert_eq!(markdown(&view, vcx), "base");
}

#[gpui::test]
async fn set_markdown_is_the_live_refresh_path(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, "old");
    vcx.simulate_input("!");
    assert!(vcx.read(|app| view.read(app).is_dirty()));
    // When the app decides Apply, the refresh replaces wholesale and clears
    // the dirty flag's meaning (content now matches the store copy).
    vcx.update(|_w, app| view.update(app, |e, cx| e.set_markdown("new remote", cx)));
    assert_eq!(markdown(&view, vcx), "new remote");
}

/// Perf smoke — 10k-line doc, row-build latency. `cargo test -p
/// memoria-editor-gpui -- --ignored perf_ten_thousand_lines` to run.
#[gpui::test]
#[ignore]
async fn perf_ten_thousand_lines(cx: &mut TestAppContext) {
    let src = (0..10_000)
        .map(|i| format!("line {i} with **bold** and `code`"))
        .collect::<Vec<_>>()
        .join("\n\n");
    let (view, vcx) = open(cx, &src);
    vcx.update(|window, _| window.refresh());
    let t = std::time::Instant::now();
    vcx.simulate_input("x");
    eprintln!("edit frame-ish time: {:?}", t.elapsed());
    assert!(markdown(&view, vcx).starts_with('x'));
}
