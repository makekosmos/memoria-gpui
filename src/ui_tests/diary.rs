//! Diary (M6) UI flows — composer submit (button + Ctrl+Enter), kind change
//! via the dot menu, thread reply, tags, calendar sidebar + date jump.
//! Interactions use `debug_selector` markers; state assertions read `Memoria`.
#![allow(clippy::missing_panics_doc)]

use gpui::TestAppContext;
use memoria_model::diary::{format_bubble_date_key, BubbleKind};
use memoria_model::routes::Route;

use super::*;

/// Navigate to the diary and let the migration + list replies land.
fn open_diary(cx: &mut VisualTestContext, app: &Entity<Memoria>) {
    click(cx, "sb-nav-diary");
    assert_eq!(route_of(cx, app), Route::Diary);
    redraw(cx);
    assert!(cx.debug_bounds("diary-view").is_some());
    assert!(cx.debug_bounds("bubble-composer").is_some());
}

/// Focus the composer and type plain text (IME path — `simulate_input`).
fn type_in_composer(cx: &mut VisualTestContext, text: &str) {
    click(cx, "bubble-composer-editor");
    cx.update(|_, cx| cx.refresh_windows());
    cx.simulate_input(text);
    cx.run_until_parked();
    redraw(cx);
}

/// The last bubble id of the flat list — newest roots sort first, so 0.
fn bubble_id(app: &Entity<Memoria>, cx: &VisualTestContext, ix: usize) -> String {
    app.read_with(cx, |a, _| a.bubbles[ix].id.clone())
}

#[gpui::test]
fn diary_view_shows_composer_and_empty_state(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    open_diary(cx, &app);
    assert!(cx.debug_bounds("bubble-timeline-empty").is_some());
    // Calendar starts closed; the toggle opens it.
    assert!(cx.debug_bounds("diary-calendar-sidebar").is_none());
    click(cx, "titlebar-diary-calendar-toggle");
    redraw(cx);
    assert!(cx.debug_bounds("diary-calendar-sidebar").is_some());
}

#[gpui::test]
fn composer_submit_creates_bubble(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    open_diary(cx, &app);
    type_in_composer(cx, "Первая запись");
    click(cx, "bubble-composer-submit");
    redraw(cx);
    assert_eq!(app.read_with(cx, |a, _| a.bubbles.len()), 1);
    assert_eq!(
        app.read_with(cx, |a, _| a.bubbles[0].text.clone()),
        "Первая запись"
    );
    assert!(cx.debug_bounds("bubble-timeline-empty").is_none());
}

/// Ctrl+Enter inside the composer submits (Vue `@keydown.ctrl.enter`).
#[gpui::test]
fn composer_ctrl_enter_submits(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    open_diary(cx, &app);
    type_in_composer(cx, "Via keys");
    cx.simulate_keystrokes("ctrl-enter");
    cx.run_until_parked();
    redraw(cx);
    assert_eq!(app.read_with(cx, |a, _| a.bubbles.len()), 1);
}

/// Each kind via the rail dot dropdown («Идея»/«Задача»/«Подсветить»).
#[gpui::test]
fn bubble_kind_menu_changes_kind(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    open_diary(cx, &app);
    for (i, text) in ["one", "two", "three", "four"].iter().enumerate() {
        type_in_composer(cx, text);
        click(cx, "bubble-composer-submit");
        redraw(cx);
        let id = bubble_id(&app, cx, 0);
        let kind = [
            BubbleKind::Plain,
            BubbleKind::Idea,
            BubbleKind::Task,
            BubbleKind::Highlight,
        ][i];
        if kind != BubbleKind::Plain {
            click_owned(cx, sel(format!("bubble-kind-{id}")));
            redraw(cx);
            click_owned(cx, sel(format!("bubble-kind-option-{}", kind.as_str())));
            redraw(cx);
        }
        assert_eq!(app.read_with(cx, |a, _| a.bubbles[0].kind), kind);
    }
    assert_eq!(app.read_with(cx, |a, _| a.bubbles.len()), 4);
}

/// `debug_bounds` requires `&'static str` — tests leak the small format
/// strings (fine for a test process).
fn sel(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// `click` with a runtime-built selector.
fn click_owned(cx: &mut VisualTestContext, selector: &'static str) {
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
    cx.update(|_, cx| cx.refresh_windows());
}

/// Reply flow — «Ответить» → textarea → submit; reply nests under the root.
#[gpui::test]
fn reply_in_thread(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    open_diary(cx, &app);
    type_in_composer(cx, "Root thought");
    click(cx, "bubble-composer-submit");
    redraw(cx);
    let root = bubble_id(&app, cx, 0);

    click_owned(cx, sel(format!("bubble-reply-{root}")));
    redraw(cx);
    let input_sel = sel(format!("bubble-reply-input-{root}"));
    assert!(cx.debug_bounds(input_sel).is_some());
    click_owned(cx, input_sel);
    cx.simulate_input("thread reply");
    cx.run_until_parked();
    redraw(cx);
    click_owned(cx, sel(format!("bubble-reply-submit-{root}")));
    redraw(cx);

    let bubbles = app.read_with(cx, |a, _| a.bubbles.clone());
    assert_eq!(bubbles.len(), 2);
    assert_eq!(bubbles[0].id, root);
    assert_eq!(bubbles[1].parent_id.as_deref(), Some(root.as_str()));
    assert_eq!(bubbles[1].text, "thread reply");
    // Reply rows render without their own reply affordance.
    assert!(cx
        .debug_bounds(sel(format!("bubble-reply-{}", bubbles[1].id)))
        .is_none());
}

/// `#tag` extraction — tag chips render and the tag is stripped from text.
#[gpui::test]
fn tags_extract_and_render(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    open_diary(cx, &app);
    type_in_composer(cx, "morning note #runs #easy");
    click(cx, "bubble-composer-submit");
    redraw(cx);
    let id = bubble_id(&app, cx, 0);
    assert_eq!(
        app.read_with(cx, |a, _| a.bubbles[0].tags.clone()),
        vec!["runs".to_string(), "easy".to_string()]
    );
    assert!(cx.debug_bounds(sel(format!("bubble-tags-{id}"))).is_some());
}

/// Edit flow — time button → edit form → Обновить persists the new text.
#[gpui::test]
fn edit_flow_updates_bubble(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    open_diary(cx, &app);
    type_in_composer(cx, "draft text");
    click(cx, "bubble-composer-submit");
    redraw(cx);
    let id = bubble_id(&app, cx, 0);

    click_owned(cx, sel(format!("bubble-time-{id}")));
    redraw(cx);
    assert!(cx
        .debug_bounds(sel(format!("bubble-editor-{id}")))
        .is_some());
    click_owned(cx, sel(format!("bubble-edit-input-{id}")));
    cx.simulate_input(" +more");
    cx.run_until_parked();
    redraw(cx);
    click_owned(cx, sel(format!("bubble-update-{id}")));
    redraw(cx);
    assert_eq!(
        app.read_with(cx, |a, _| a.bubbles[0].text.clone()),
        "draft text +more"
    );
    assert!(cx
        .debug_bounds(sel(format!("bubble-editor-{id}")))
        .is_none());
}

/// Two-step delete — «Удалить» arms, «Точно удалить» removes.
#[gpui::test]
fn delete_flow_removes_bubble(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    open_diary(cx, &app);
    type_in_composer(cx, "short lived");
    click(cx, "bubble-composer-submit");
    redraw(cx);
    let id = bubble_id(&app, cx, 0);

    click_owned(cx, sel(format!("bubble-time-{id}")));
    redraw(cx);
    click_owned(cx, sel(format!("bubble-delete-{id}")));
    redraw(cx);
    // Armed — still present.
    assert_eq!(app.read_with(cx, |a, _| a.bubbles.len()), 1);
    click_owned(cx, sel(format!("bubble-delete-{id}")));
    redraw(cx);
    assert!(app.read_with(cx, |a, _| a.bubbles.is_empty()));
}

/// Calendar — day rows exist, click selects the date and the timeline
/// consumes the pending jump (scroll target).
#[gpui::test]
fn calendar_day_jump(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    open_diary(cx, &app);
    type_in_composer(cx, "today entry");
    click(cx, "bubble-composer-submit");
    redraw(cx);

    click(cx, "titlebar-diary-calendar-toggle");
    redraw(cx);
    let today = format_bubble_date_key(memoria_model::time::now_millis());
    let day_sel = sel(format!("diary-calendar-day-{today}"));
    assert!(cx.debug_bounds(day_sel).is_some());
    click_owned(cx, day_sel);
    redraw(cx);
    // Jump consumed by render → no pending target left.
    assert!(app.read_with(cx, |a, _| a.diary_jump.is_none()));
}
