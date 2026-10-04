//! Extra UI flow tests — conflict banner, settings, trash, sticker routes.
#![allow(clippy::missing_panics_doc)]

use gpui::TestAppContext;

use memoria_model::entry_conflicts::{upsert_entry_conflict, EntryConflictState};
use memoria_model::routes::Route;

use super::*;
/// and lands in trash.
#[gpui::test]
fn delete_current_returns_to_everything(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    click(cx, "everything-card-n-1");
    assert_eq!(route_of(cx, &app), Route::Entry("n-1".into()));
    redraw(cx);

    click(cx, "entry-menu-btn");
    redraw(cx);
    click(cx, "ctx-delete");
    redraw(cx);
    click(cx, "confirm-ok");
    redraw(cx);
    assert_eq!(route_of(cx, &app), Route::Everything);
    assert!(cx.debug_bounds("everything-card-n-1").is_none());
}

/// Trash settings tab lists deleted entries; restore puts one back.
#[gpui::test]
fn trash_restore_flow(cx: &mut TestAppContext) {
    let (_app, cx) = launch(cx);
    redraw(cx);
    // Soft-delete n-2 via menu.
    right_click(cx, "everything-card-n-2");
    redraw(cx);
    click(cx, "ctx-delete");
    redraw(cx);
    click(cx, "confirm-ok");
    redraw(cx);

    click(cx, "sb-trash");
    redraw(cx);
    assert!(cx.debug_bounds("trash-item-n-2").is_some());
    click(cx, "settings-btn-trash-restore");
    redraw(cx);
    assert!(cx.debug_bounds("trash-empty").is_some());
    click(cx, "sb-nav-everything");
    redraw(cx);
    assert!(cx.debug_bounds("everything-card-n-2").is_some());
}

/// Injecting a `remote-updated` conflict shows the banner; «Принять
/// удалённую» applies the remote and resolves the conflict.
#[gpui::test]
fn conflict_banner_accept_remote(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);

    // Simulate: local copy of n-1 vs a newer remote revision.
    let (local, remote) = app.read_with(cx, |a, _| {
        let local = a.list.iter().find(|e| e.id == "n-1").unwrap().clone();
        let mut remote = local.clone();
        remote.title = "Планы на неделю (удалённая)".into();
        remote.updated_at += 1000;
        (local, remote)
    });
    app.update(cx, |a, cx| {
        // The remote revision already won in storage (that's what makes the
        // conflict real); the conflict record keeps the local draft.
        if let crate::app::Backend::Demo(store) = &mut a.backend {
            if let Some(slot) = store.entries.iter_mut().find(|e| e.id == "n-1") {
                *slot = remote.clone();
            }
        }
        a.conflicts.conflicts = upsert_entry_conflict(
            &a.conflicts.conflicts,
            &local,
            Some(&remote),
            EntryConflictState::RemoteUpdated,
            memoria_model::entry_conflicts::now(),
        );
        cx.notify();
    });
    redraw(cx);
    assert!(cx.debug_bounds("entry-conflict-banner").is_some());

    click(cx, "conflict-accept");
    redraw(cx);
    app.read_with(cx, |a, _| {
        assert!(
            memoria_model::entry_conflicts::unresolved_entry_conflicts(&a.conflicts.conflicts)
                .is_empty(),
            "conflict must resolve after accept-remote"
        );
    });
    redraw(cx);
    assert!(cx.debug_bounds("entry-conflict-banner").is_none());
}

/// General settings toggles persist into `prefs` (local-state semantics).
#[gpui::test]
fn settings_toggles_and_trash_tab(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    click(cx, "sb-settings");
    assert_eq!(
        route_of(cx, &app),
        Route::Settings(memoria_model::routes::SettingsTab::General)
    );
    redraw(cx);
    click(cx, "pref-spellcheck");
    assert!(app.read_with(cx, |a, _| a.prefs.preferences.spellcheck_enabled));
    click(cx, "settings-tab-export");
    redraw(cx);
    assert!(cx.debug_bounds("eden-export-md").is_some());
    click(cx, "settings-tab-trash");
    redraw(cx);
    assert!(cx.debug_bounds("trash-empty").is_some());
}

/// `stickerRoutes` — the route/window key helpers map entry ids to sticker
/// routes, matching the Vue contract (window creation is manual-smoke).
#[gpui::test]
fn sticker_routes_resolve(_cx: &mut TestAppContext) {
    // Route helpers are pure — covered here via the UI build so the shell's
    // sticker entry point exercises the same strings.
    assert_eq!(
        memoria_model::sticker_route::sticker_route_for("n-1"),
        "/sticker/n-1"
    );
    assert_eq!(
        memoria_model::sticker_route::parse_sticker_route("/sticker/n-1"),
        Some("n-1".to_string())
    );
    assert_eq!(
        memoria_model::sticker_route::parse_sticker_route("/note/n-1"),
        None
    );
}

/// «Отображаемые типы» toggle must refilter the entry list — hiding
/// «Книга» drops book cards from «Всё», re-enabling brings them back.
#[gpui::test]
fn type_visibility_toggle_filters_list(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    assert!(cx.debug_bounds("everything-card-b-1").is_some());

    click(cx, "sb-settings");
    redraw(cx);
    click(cx, "pref-type-book_obj");
    redraw(cx);
    assert!(
        app.read_with(cx, |a, _| !a.prefs.visible_object_type_ids.is_empty()),
        "hiding a type persists a non-empty visible list"
    );

    click(cx, "sb-nav-everything");
    redraw(cx);
    assert!(
        cx.debug_bounds("everything-card-b-1").is_none(),
        "hidden type must leave the list"
    );
    assert!(cx.debug_bounds("everything-card-n-1").is_some());

    click(cx, "sb-settings");
    redraw(cx);
    click(cx, "pref-type-book_obj");
    redraw(cx);
    click(cx, "sb-nav-everything");
    redraw(cx);
    assert!(cx.debug_bounds("everything-card-b-1").is_some());
}

/// `keepConflictLocalAsCopy` arms `pending_copy_save` for the *copy's* id —
/// an unrelated `Saved` reply (e.g. an autosave landing mid-flow) must not
/// consume it and fire the accept-remote continuation early.
#[gpui::test]
fn unrelated_save_reply_does_not_steal_pending_copy_save(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);

    let (local, remote) = app.read_with(cx, |a, _| {
        let local = a.list.iter().find(|e| e.id == "n-1").unwrap().clone();
        let mut remote = local.clone();
        remote.title = "Планы (удалённая)".into();
        remote.updated_at += 1000;
        (local, remote)
    });
    app.update(cx, |a, cx| {
        // Remote revision already won in storage (that's the real conflict).
        if let crate::app::Backend::Demo(store) = &mut a.backend {
            if let Some(slot) = store.entries.iter_mut().find(|e| e.id == "n-1") {
                *slot = remote.clone();
            }
        }
        a.conflicts.conflicts = upsert_entry_conflict(
            &a.conflicts.conflicts,
            &local,
            Some(&remote),
            EntryConflictState::RemoteUpdated,
            memoria_model::entry_conflicts::now(),
        );
        // Armed state as `keepConflictLocalAsCopy` leaves it on the async
        // Engine backend (demo replies are synchronous, so the marker is set
        // directly): (conflict id, copy entry id).
        let conflict_id = a.conflicts.conflicts[0].id.clone();
        a.pending_copy_save = Some((conflict_id, "copy-of-n-1".into()));

        // An in-flight autosave reply for the conflicted entry arrives first.
        a.on_reply(
            memoria_model::store::Reply::Saved {
                id: "n-1".into(),
                result: Ok(memoria_model::model::SaveEntryResult::ok("n-1".to_string())),
            },
            cx,
        );
        assert!(
            a.pending_copy_save.is_some(),
            "unrelated Saved reply stole the pending copy-save"
        );
        assert!(
            a.conflict_op.is_none(),
            "accept-remote must wait for the copy's own Saved reply"
        );

        // The copy's own reply consumes the marker and runs the accept flow
        // (the demo backend resolves the follow-up `LoadEntry` inline, so the
        // conflict lands resolved in the same update).
        a.on_reply(
            memoria_model::store::Reply::Saved {
                id: "copy-of-n-1".into(),
                result: Ok(memoria_model::model::SaveEntryResult::ok(
                    "copy-of-n-1".to_string(),
                )),
            },
            cx,
        );
        assert!(a.pending_copy_save.is_none());
        assert!(
            memoria_model::entry_conflicts::unresolved_entry_conflicts(&a.conflicts.conflicts)
                .is_empty(),
            "accept flow must run once the copy's own reply lands"
        );
    });
}
