//! Navigation — route application, `navigateTo` port, history replay.
use gpui::Context;

use memoria_gpui::content;
use memoria_gpui::entry_conflicts::conflict_for_entry;
use memoria_gpui::object_views::collection_target_type_id;
use memoria_gpui::routes::Route;
use memoria_gpui::store::Command;

use super::Memoria;

impl Memoria {
    /// `navigate(route)` — every route transition records history first,
    /// matching the Vue snapshot watcher.
    pub(crate) fn navigate(&mut self, route: Route, cx: &mut Context<Self>) {
        self.history.record(route.clone());
        self.apply_route(route, cx);
    }

    /// Apply a route without touching history (back/forward replay).
    fn apply_route(&mut self, route: Route, cx: &mut Context<Self>) {
        self.ctx_menu = None;
        self.kind_menu_for = None;
        // Vue App.vue watch(route) — `calendarOpen` resets on every
        // navigation away from the diary.
        if !matches!(route, Route::Diary) {
            self.diary_calendar_open = false;
        }
        self.route = route.clone();
        match &route {
            Route::Diary => {
                self.current = None;
                self.loading_entry = None;
                self.start_diary(cx);
            }
            Route::Entry(id) => self.open_entry(id.clone(), cx),
            Route::Collection(type_id) => {
                self.loading_entry = None;
                // Vue keeps currentEntry = the collection object when known.
                self.current = self
                    .list
                    .iter()
                    .find(|e| {
                        collection_target_type_id(Some(e)).as_deref() == Some(type_id.as_str())
                    })
                    .cloned();
            }
            _ => {
                self.current = None;
                self.loading_entry = None;
            }
        }
        self.route_changed_settings(cx);
        cx.notify();
    }

    /// `eden.navigateTo` — conflict guard, collection redirect, then load.
    pub(crate) fn open_entry(&mut self, id: String, cx: &mut Context<Self>) {
        // Flush pending edits so the note-switch can't lose the last
        // <300ms of typing or misattribute them to the next entry.
        if let Some(editor) = self.editor.clone() {
            editor.update(cx, |e, cx| e.flush_autosave(cx));
        }
        if let Some(conflict) = conflict_for_entry(&self.conflicts.conflicts, &id) {
            let local = conflict.local.clone();
            self.queue_editor_fill(&local);
            self.current = Some(local);
            self.loading_entry = None;
            self.route = Route::Entry(id);
            self.start_conflict_recheck(cx);
            return;
        }
        if let Some(preview) = self.list.iter().find(|e| e.id == id).cloned() {
            if let Some(target) = collection_target_type_id(Some(&preview)) {
                self.route = Route::Collection(target.clone());
                // Route was already recorded as Entry — rewrite the snapshot.
                self.history.record(Route::Collection(target));
                self.current = Some(preview);
                return;
            }
            if preview.content_loaded == Some(true) {
                self.queue_editor_fill(&preview);
                self.current = Some(preview);
                self.loading_entry = None;
                self.send(
                    Command::LoadEntry {
                        id,
                        content_only: false,
                    },
                    cx,
                );
                return;
            }
        }
        self.loading_entry = Some(id.clone());
        self.send(
            Command::LoadEntry {
                id,
                content_only: false,
            },
            cx,
        );
    }

    /// `navigateBack`/`navigateForward` — pop stack, suppress recording, apply.
    pub(crate) fn go_back(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.history.navigate_back(&self.route) else {
            return;
        };
        self.replay(target, cx);
    }

    pub(crate) fn go_forward(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.history.navigate_forward(&self.route) else {
            return;
        };
        self.replay(target, cx);
    }

    fn replay(&mut self, target: Route, cx: &mut Context<Self>) {
        self.history.set_suppress(true);
        self.apply_route(target.clone(), cx);
        // `record` under suppression just updates `previous` — keeps the
        // stacks consistent when the next real navigation happens.
        self.history.record(target);
        self.history.set_suppress(false);
    }

    /// `eden.deleteEntry` — soft delete; if it was open, return to «Всё».
    pub(crate) fn delete_entry(&mut self, id: String, cx: &mut Context<Self>) {
        self.send(Command::DeleteEntry(id), cx);
    }
    /// Stash title+markdown for the M3 editor — applied in `Render` with a Window.
    fn queue_editor_fill(&mut self, entry: &memoria_gpui::model::Entry) {
        let raw = serde_json::from_str(&entry.content_json).unwrap_or(serde_json::Value::Null);
        let markdown = content::read_entry_markdown(&raw);
        self.pending_fill = Some((entry.title.clone(), markdown));
        self.dirty = false;
    }
}
