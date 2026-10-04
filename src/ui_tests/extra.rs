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
