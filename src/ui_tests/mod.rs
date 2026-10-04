//! Headless UI tests for the Memoria M4 shell (KOS-151).
//!
//! Test builds always take the in-memory demo store (`cfg!(test)` in
//! `Memoria::new`), so these drive the real GPUI event pipeline — keystroke
//! dispatch, hitbox clicks, routing — without an Engine worker. Interactions
//! are located by `debug_selector` markers, not fixed coordinates.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AppContext, Bounds, Entity, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent,
    Pixels, PlatformInput, TestAppContext, VisualTestContext,
};

use memoria_model::routes::Route;

use crate::app::Memoria;

/// Builds the same view tree as `main.rs` — Memoria inside
/// `gpui_component::Root` — on the deterministic test platform.
fn launch(cx: &mut TestAppContext) -> (Entity<Memoria>, &mut VisualTestContext) {
    cx.update(gpui_component::init);
    cx.update(|cx| cx.bind_keys(memoria_editor_gpui::key_bindings()));
    let slot: Rc<RefCell<Option<Entity<Memoria>>>> = Rc::new(RefCell::new(None));
    let slot2 = slot.clone();
    let (_root, cx) = cx.add_window_view(move |window, cx| {
        let app = cx.new(Memoria::new);
        *slot2.borrow_mut() = Some(app.clone());
        gpui_component::Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().expect("window builder ran");
    (app, cx)
}

/// Force a repaint — product handlers mutate state without `cx.notify()`,
/// and the test platform only redraws when the window is marked dirty.
fn redraw(cx: &mut VisualTestContext) {
    cx.update(|_, cx| cx.refresh_windows());
}

/// Left-click the center of the painted bounds of `selector`.
fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds: Bounds<Pixels> = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("'{selector}' has no painted bounds"));
    let position = bounds.center();
    let modifiers = Modifiers::default();
    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position,
                modifiers,
                button: MouseButton::Left,
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position,
                modifiers,
                button: MouseButton::Left,
                click_count: 1,
            }),
            cx,
        );
    });
    cx.run_until_parked();
    // Repaint so the next lookup sees fresh hitboxes — e.g. the titlebar
    // history buttons toggle their enabled listener per frame.
    cx.update(|_, cx| cx.refresh_windows());
}

/// Right-click — context menus open on mousedown.
fn right_click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds: Bounds<Pixels> = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("'{selector}' has no painted bounds"));
    let position = bounds.center();
    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position,
                modifiers: Modifiers::default(),
                button: MouseButton::Right,
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
    });
    cx.run_until_parked();
    cx.update(|_, cx| cx.refresh_windows());
}

/// Type text into the focused input — `key_char` set explicitly because
/// `simulate_input` parses chars via `Keystroke::parse` and skips them.
fn type_text(cx: &mut VisualTestContext, text: &str) {
    cx.update(|window, cx| {
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
    });
    cx.run_until_parked();
}

fn route_of(cx: &VisualTestContext, app: &Entity<Memoria>) -> Route {
    app.read_with(cx, |a, _| a.route.clone())
}

/// Launch → demo list loads → «Всё» grid paints cards.
#[gpui::test]
fn launch_shows_everything(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    assert_eq!(route_of(cx, &app), Route::Everything);
    assert!(cx.debug_bounds("everything-view").is_some());
    assert!(cx.debug_bounds("everything-card-n-1").is_some());
    assert!(cx.debug_bounds("eden-sidebar").is_some());
    // Sidebar structure — nav + type rows with counters.
    assert!(cx.debug_bounds("sb-nav-everything").is_some());
}

/// Card click opens the M3 note editor; back/forward walk history.
#[gpui::test]
fn card_opens_note_and_history_works(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);

    click(cx, "everything-card-n-1");
    assert_eq!(route_of(cx, &app), Route::Entry("n-1".into()));
    redraw(cx);
    assert!(cx.debug_bounds("note-view").is_some());
    assert!(cx.debug_bounds("note-editor").is_some());

    click(cx, "nav-back");
    assert_eq!(route_of(cx, &app), Route::Everything);
    click(cx, "nav-forward");
    assert_eq!(route_of(cx, &app), Route::Entry("n-1".into()));
    click(cx, "nav-back");
    assert_eq!(route_of(cx, &app), Route::Everything);
}

/// Sidebar type row navigates to the typed collection table.
#[gpui::test]
fn sidebar_type_row_opens_collection(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    click(cx, "sb-type-book_obj");
    assert_eq!(route_of(cx, &app), Route::Collection("book_obj".into()));
    redraw(cx);
    assert!(cx.debug_bounds("type-objects-view").is_some());
    assert!(cx.debug_bounds("object-row-b-1").is_some());
}

/// Ctrl+K opens the palette; typing searches the demo Engine; Enter opens
/// the selected hit.
#[gpui::test]
fn search_opens_and_navigates(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    cx.simulate_keystrokes("ctrl-k");
    redraw(cx);
    assert!(cx.debug_bounds("eden-search-palette").is_some());

    type_text(cx, "Война");
    redraw(cx);
    assert!(cx.debug_bounds("eden-search-result-item-0").is_some());

    cx.simulate_keystrokes("enter");
    assert_eq!(route_of(cx, &app), Route::Entry("b-1".into()));
}

/// Esc closes the palette without navigating.
#[gpui::test]
fn search_escape_closes(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    cx.simulate_keystrokes("ctrl-k");
    redraw(cx);
    cx.simulate_keystrokes("escape");
    redraw(cx);
    assert!(cx.debug_bounds("eden-search-palette").is_none());
    assert_eq!(route_of(cx, &app), Route::Everything);
}

/// Right-click → «Закрепить» adds a pinned sidebar row; unpin removes it.
#[gpui::test]
fn pin_via_context_menu(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    right_click(cx, "everything-card-n-1");
    redraw(cx);
    click(cx, "ctx-pin");
    redraw(cx);
    assert!(cx.debug_bounds("sb-pin-n-1").is_some());
    assert!(app.read_with(cx, |a, _| a.prefs.pinned_entry_ids.contains(&"n-1".into())));

    right_click(cx, "everything-card-n-1");
    redraw(cx);
    click(cx, "ctx-pin");
    redraw(cx);
    assert!(cx.debug_bounds("sb-pin-n-1").is_none());
}

/// Delete via context menu + confirm → the open entry falls back to «Всё»
mod diary;
mod extra;
mod sticker;
#[cfg(test)]
mod typed;
#[cfg(test)]
mod typed_book;
