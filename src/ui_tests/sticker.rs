//! M7 sticker tests — `TestAppContext` multi-window checks: shared document
//! buffer between the main window and a floating sticker, one save per
//! autosave cycle, keyed-window reopen dedup, and a separate doc per entry.

use std::time::Duration;

use gpui::{AnyWindowHandle, AppContext, Keystroke, Modifiers, TestAppContext};
use memoria_gpui::sticker_route::sticker_window_key_for;

use super::{click, launch, redraw};

/// Open a sticker for `entry_id` through the real `open_sticker` path and
/// return the new window's handle.
fn open_sticker(
    cx: &mut gpui::VisualTestContext,
    app: &gpui::Entity<crate::app::Memoria>,
    entry_id: &str,
) -> AnyWindowHandle {
    let before: Vec<AnyWindowHandle> = cx.windows();
    let id = entry_id.to_string();
    cx.update(|window, cx| app.update(cx, |app, cx| app.open_sticker(id.clone(), window, cx)));
    let new: Vec<AnyWindowHandle> = cx
        .windows()
        .into_iter()
        .filter(|h| !before.contains(h))
        .collect();
    assert_eq!(new.len(), 1, "expected exactly one new sticker window");
    new[0]
}

/// Focus the shared editor inside the sticker window and repaint so its
/// input handler registers on that window's platform side.
fn focus_sticker_editor(
    cx: &mut gpui::VisualTestContext,
    app: &gpui::Entity<crate::app::Memoria>,
    sticker: AnyWindowHandle,
    entry_id: &str,
) {
    let editor = app.read_with(cx, |a, cx| {
        a.docs
            .get(entry_id)
            .expect("doc exists")
            .read(cx)
            .editor
            .clone()
    });
    let focus = editor.read_with(cx, |e, _| e.focus.clone());
    cx.update_window(sticker, |_, window, cx| {
        focus.focus(window, cx);
    })
    .unwrap();
    redraw(cx);
}

/// Type `text` into whatever is focused in `window`, then advance the fake
/// clock past the editor's 300ms autosave debounce and pump — timers don't
/// fire under `run_until_parked` alone (same pattern as the editor crate's
/// own autosave tests).
fn type_in_window(cx: &mut TestAppContext, window: AnyWindowHandle, text: &str) {
    cx.update_window(window, |_, window, cx| {
        for ch in text.chars() {
            window.dispatch_keystroke(
                Keystroke {
                    modifiers: Modifiers::default(),
                    key: ch.to_lowercase().to_string(),
                    key_char: Some(ch.to_string()),
                },
                cx,
            );
        }
    })
    .unwrap();
    cx.executor()
        .advance_clock(memoria_editor_gpui::AUTOSAVE_DEBOUNCE + Duration::from_millis(50));
    cx.executor().run_until_parked();
}

/// Sticker on the open note: shared doc, shared editor buffer, one save.
#[gpui::test]
fn sticker_window_shares_document_and_dedupes_save(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    click(cx, "everything-card-n-1");
    redraw(cx);

    // Vue `titlebar-open-sticker` — the note toolbar entry point.
    assert!(cx.debug_bounds("titlebar-open-sticker").is_some());
    let before = cx.windows();
    click(cx, "titlebar-open-sticker");
    redraw(cx);
    assert_eq!(cx.windows().len(), before.len() + 1);
    let sticker = *cx
        .windows()
        .iter()
        .find(|h| !before.contains(h))
        .expect("sticker window");
    focus_sticker_editor(cx, &app, sticker, "n-1");

    type_in_window(cx, sticker, "x");

    // The edit landed on the *shared* buffer — the main window's editor is
    // the same entity, so both windows observe one document.
    let markdown = app.read_with(cx, |a, cx| {
        let doc = a.docs.get("n-1").expect("doc for n-1");
        let editor = doc.read(cx).editor.clone();
        editor.read(cx).markdown()
    });
    assert!(
        markdown.contains('x'),
        "sticker edit must land: {markdown:?}"
    );

    // Exactly one save through the shared Engine/demo path for the cycle.
    cx.executor().run_until_parked();
    let saves = app.read_with(cx, |a, _| match &a.backend {
        crate::app::Backend::Demo(store) => store.save_count,
        crate::app::Backend::Engine(_) => usize::MAX,
    });
    assert_eq!(saves, 1, "one upsert per autosave cycle");

    let dirty = app.read_with(cx, |a, cx| a.docs.get("n-1").unwrap().read(cx).is_dirty(cx));
    assert!(!dirty, "save completed — doc is clean");
}

/// Reopening the same sticker key focuses the existing window — no dup.
#[gpui::test]
fn sticker_reopen_focuses_existing_window(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    click(cx, "everything-card-n-1");
    redraw(cx);

    let sticker = open_sticker(cx, &app, "n-1");
    let count = cx.windows().len();

    let id = "n-1".to_string();
    cx.update(|window, cx| app.update(cx, |app, cx| app.open_sticker(id.clone(), window, cx)));
    assert_eq!(cx.windows().len(), count, "reopen must not spawn a window");

    // The registry still maps the key to the same handle.
    let key = sticker_window_key_for("n-1");
    let registered = app.read_with(cx, |a, _| a.stickers.get(&key).copied());
    assert_eq!(registered.map(|h| h.window_id()), Some(sticker.window_id()));
}

/// A different entry gets its own doc and its own sticker window.
#[gpui::test]
fn sticker_for_other_entry_opens_its_own_doc(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    click(cx, "everything-card-n-1");
    redraw(cx);

    let first = open_sticker(cx, &app, "n-1");
    let second = open_sticker(cx, &app, "n-2");
    assert_ne!(first.window_id(), second.window_id());
    assert_eq!(cx.windows().len(), 3);

    let (docs, distinct_editors) = app.read_with(cx, |a, cx| {
        let d1 = a.docs.get("n-1").expect("doc n-1").clone();
        let d2 = a.docs.get("n-2").expect("doc n-2").clone();
        (a.docs.len(), d1.read(cx).editor != d2.read(cx).editor)
    });
    assert_eq!(docs, 2);
    assert!(distinct_editors, "each entry keeps its own document");
}
