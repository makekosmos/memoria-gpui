//! Engine `Reply` handling — list/entry hydration, trash, search, conflict
//! operation continuations (recheck/accept need a canonical remote read).
use gpui::Context;

use memoria_gpui::entry_conflicts::EntryConflictState;
use memoria_gpui::live_refresh::RemoteEntryDecision;
use memoria_gpui::model::Entry;
use memoria_gpui::object_views::collection_target_type_id;
use memoria_gpui::routes::Route;
use memoria_gpui::store::{Command, EngineEvent, Reply};

use super::refresh;
use super::types::ENGINE_OFFLINE;
use super::Memoria;

impl Memoria {
    pub(crate) fn on_reply(&mut self, reply: Reply, cx: &mut Context<Self>) {
        self.busy = false;
        match reply {
            Reply::List(Ok(list)) => {
                self.list = list;
                if self.online {
                    self.banner = None;
                }
            }
            Reply::List(Err(error)) | Reply::NoteTypes(Err(error)) => {
                self.online = false;
                self.banner = Some(error);
            }
            Reply::NoteTypes(Ok(types)) => {
                self.note_types = types
                    .iter()
                    .map(memoria_gpui::system_types::normalize_system_note_type)
                    .collect();
            }
            Reply::Entry { id, result } => self.on_entry_loaded(id, result, cx),
            Reply::Saved { id, result } => self.on_saved(id, result, cx),
            Reply::Deleted { id, result } => self.on_deleted(id, result, cx),
            Reply::Restored { id, result } => match result {
                Ok(done) if done.ok => {
                    self.toast("Заметка восстановлена", cx);
                    self.send(Command::LoadTrash, cx);
                    self.send(Command::LoadList(Vec::new()), cx);
                }
                Ok(done) => self.toast(done.message.unwrap_or_else(|| "Ошибка".into()), cx),
                Err(e) => {
                    let _ = id;
                    self.toast(e, cx);
                }
            },
            Reply::Trash(Ok(trash)) => {
                self.trash = trash;
                self.trash_loaded = true;
            }
            Reply::Trash(Err(e)) => self.toast(e, cx),
            Reply::Collections(_) => {
                // Newly ensured collection objects join the list.
                self.send(Command::LoadList(Vec::new()), cx);
            }
            Reply::Search { query, result } => {
                if self.search_query() == query {
                    self.search_results = result.unwrap_or_default();
                    self.search_selected = 0;
                }
            }
            r @ (Reply::Bubbles(_)
            | Reply::BubbleCreated(_)
            | Reply::BubbleUpdated { .. }
            | Reply::BubbleDeleted { .. }
            | Reply::BubbleMigrated { .. }
            | Reply::DiaryMigrated(_)) => self.on_bubble_reply(r, cx),
            Reply::Event(event) => self.on_engine_event(event, cx),
            // App network ops — typed-header metadata/cover replies.
            Reply::BookMetadata(result) => self.on_book_metadata(result, cx),
            Reply::BookMetadataPage(result) => self.on_book_metadata_page(result, cx),
            Reply::DominantColor { source, result } => {
                if let Ok(color) = result {
                    self.spine_colors.insert(source, color);
                }
            }
            Reply::CoverStored { entry_id, result } => self.on_cover_stored(entry_id, result, cx),
            Reply::ImageFetched { url, result } => self.on_image_fetched(url, result, cx),
        }
        cx.notify();
    }

    fn on_entry_loaded(
        &mut self,
        id: String,
        result: Result<Option<Entry>, String>,
        cx: &mut Context<Self>,
    ) {
        // Route to the shared doc first — sticker windows and remote-change
        // reloads land there regardless of the main window's current entry.
        if let Some(doc) = self.docs.get(&id).cloned() {
            doc.update(cx, |doc, cx| doc.apply_entry_reply(result.clone(), cx));
        }
        // A pending conflict op owns this reply — it needs the canonical read.
        if self.conflict_op.is_some() {
            self.conflict_entry_reply(id.clone(), result.clone(), cx);
        }
        // Only a reply for the main window's pending/open entry mutates
        // `current`/`route` — a sticker-initiated load must not hijack nav.
        let is_current_request = self.loading_entry.as_deref() == Some(id.as_str())
            || self.current.as_ref().is_some_and(|e| e.id == id);
        if !is_current_request {
            return;
        }
        match result {
            Ok(Some(entry)) => {
                self.loading_entry = None;
                if let Some(target) = collection_target_type_id(Some(&entry)) {
                    self.route = Route::Collection(target);
                    self.current = Some(entry);
                    return;
                }
                self.set_current(entry, cx);
            }
            Ok(None) => {
                // Vue navigateTo → null → currentEntry = null; the shell shows
                // «Никакая страница не выбрана» while the history entry stays.
                self.loading_entry = None;
                self.current = None;
                self.toast("Заметка не найдена", cx);
            }
            Err(error) => {
                self.loading_entry = None;
                self.toast(error, cx);
            }
        }
    }

    fn on_saved(
        &mut self,
        id: String,
        result: Result<memoria_gpui::model::SaveEntryResult, String>,
        cx: &mut Context<Self>,
    ) {
        // keepConflictLocalAsCopy → copy landed; run accept flow.
        if let Some(conflict_id) = self.pending_copy_save.take() {
            match &result {
                Ok(saved) if saved.ok => {
                    self.send_accept_remote(conflict_id, cx);
                    return;
                }
                Ok(saved) => self.toast(
                    saved
                        .message
                        .clone()
                        .unwrap_or_else(|| "Не удалось сохранить".into()),
                    cx,
                ),
                Err(error) => self.toast(error.clone(), cx),
            }
        }
        let saved_ok = matches!(&result, Ok(s) if s.ok);
        // The owning doc restores dirty flags (editor revision + title) —
        // typing during an in-flight save stays dirty in every window.
        if let Some(doc) = self.docs.get(&id).cloned() {
            doc.update(cx, |doc, cx| doc.on_saved(result, cx));
        } else if !saved_ok {
            match result {
                Ok(saved) => self.toast(
                    saved
                        .message
                        .unwrap_or_else(|| "Не удалось сохранить".into()),
                    cx,
                ),
                Err(error) => self.toast(error, cx),
            }
        }
        if saved_ok {
            if self.current.as_ref().is_some_and(|e| e.id == id) {
                self.dirty = self
                    .docs
                    .get(&id)
                    .map(|d| d.read(cx).is_dirty(cx))
                    .unwrap_or(false);
                self.status = Some("Сохранено".into());
            }
            self.send(Command::LoadList(Vec::new()), cx);
        }
    }

    fn on_deleted(
        &mut self,
        id: String,
        result: Result<memoria_gpui::model::DeleteEntryResult, String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(done) if done.ok => {
                if matches!(self.route, Route::Settings(_)) {
                    self.send(Command::LoadTrash, cx);
                }
                if self.current.as_ref().map(|e| e.id.as_str()) == Some(id.as_str()) {
                    self.current = None;
                    self.navigate(Route::Everything, cx);
                }
                self.toast("Заметка удалена", cx);
                self.send(Command::LoadList(Vec::new()), cx);
            }
            Ok(done) => self.toast(done.message.unwrap_or_else(|| "Ошибка".into()), cx),
            Err(e) => self.toast(e, cx),
        }
    }

    fn on_engine_event(&mut self, event: EngineEvent, cx: &mut Context<Self>) {
        match event {
            EngineEvent::Online => {
                self.online = true;
                self.banner = None;
                self.send(Command::LoadList(Vec::new()), cx);
            }
            EngineEvent::Offline => {
                self.online = false;
                self.banner = Some(ENGINE_OFFLINE.into());
            }
            EngineEvent::Changed(_) => {
                self.send(Command::LoadList(Vec::new()), cx);
                if matches!(self.route, Route::Settings(_)) {
                    self.send(Command::LoadTrash, cx);
                }
                // Vue `subscribeBubbleChanges` — the diary re-lists on any
                // Engine change while it is the active screen.
                // Vue: `if (activeBubbleWrites === 0) void refreshLocalBubbles()`
                if matches!(self.route, Route::Diary) && self.active_bubble_writes == 0 {
                    self.send(Command::ListBubbles, cx);
                }
                // Reload every clean open doc (current note + sticker
                // windows) — typing docs are left alone; autosave persists
                // and live-refresh guards protect their buffers.
                let reload: Vec<String> = self
                    .docs
                    .iter()
                    .filter(|(_, doc)| !doc.read(cx).is_dirty(cx))
                    .map(|(id, _)| id.clone())
                    .collect();
                for id in reload {
                    self.send(
                        Command::LoadEntry {
                            id,
                            content_only: false,
                        },
                        cx,
                    );
                }
            }
        }
    }

    /// `set_current` — remote-change guard + conflict upsert, then fill the
    /// shared doc (via `pending_fill`) when the swap is safe.
    pub(crate) fn set_current(&mut self, entry: Entry, cx: &mut Context<Self>) {
        let current_md = self
            .editor
            .as_ref()
            .map(|e| e.read(cx).markdown().to_string())
            .unwrap_or_default();
        let decision = refresh::refresh_decision(
            &entry,
            self.current.as_ref(),
            &current_md,
            self.current_doc_dirty(cx),
        );
        match decision {
            RemoteEntryDecision::Apply => {
                // A remote revision differing from the open baseline while the
                // entry was viewed locally is a `remote-updated` conflict.
                if let Some(current) = &self.current {
                    if current.id == entry.id && entry.updated_at > current.updated_at {
                        self.conflicts.record(
                            current,
                            Some(&entry),
                            EntryConflictState::RemoteUpdated,
                        );
                    }
                }
                self.pending_fill = Some(entry.clone());
                self.current = Some(entry);
                self.dirty = false;
            }
            // Content differs but the editor is dirty: refresh metadata only.
            RemoteEntryDecision::SkipDirty => {
                self.current = Some(entry);
            }
            // Identical content — self-echo or no-op refresh.
            RemoteEntryDecision::SkipSameContent => {
                self.current = Some(entry);
            }
        }
        self.status = None;
        cx.notify();
    }
}
