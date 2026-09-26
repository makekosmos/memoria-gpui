//! Engine `Reply` handling — list/entry hydration, trash, search, conflict
//! operation continuations (recheck/accept need a canonical remote read).
use gpui::Context;

use memoria_gpui::content;
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
            Reply::Saved(result) => self.on_saved(result, cx),
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
            Reply::Event(event) => self.on_engine_event(event, cx),
        }
        cx.notify();
    }

    fn on_entry_loaded(
        &mut self,
        id: String,
        result: Result<Option<Entry>, String>,
        cx: &mut Context<Self>,
    ) {
        // A pending conflict op owns this reply — it needs the canonical read.
        if self.conflict_op.is_some() {
            self.conflict_entry_reply(id.clone(), result.clone(), cx);
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
        result: Result<memoria_gpui::model::SaveEntryResult, String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(saved) if saved.ok => {
                if let Some(conflict_id) = self.pending_copy_save.take() {
                    // keepConflictLocalAsCopy → copy landed; run accept flow.
                    self.send_accept_remote(conflict_id, cx);
                    return;
                }
                // Restore dirty from the editor: edits typed after the
                // autosave fired are still unsaved.
                self.dirty = self
                    .editor
                    .as_ref()
                    .map(|e| {
                        e.update(cx, |e, _| {
                            e.mark_saved();
                            e.is_dirty()
                        })
                    })
                    .unwrap_or(false);
                self.status = Some("Сохранено".into());
                self.send(Command::LoadList(Vec::new()), cx);
            }
            Ok(saved) => {
                self.pending_copy_save = None;
                self.toast(
                    saved
                        .message
                        .unwrap_or_else(|| "Не удалось сохранить".into()),
                    cx,
                );
            }
            Err(error) => {
                self.pending_copy_save = None;
                self.toast(error, cx);
            }
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
                // Skip remote entry reload while the user is typing — autosave
                // will persist, and live-refresh guards protect the buffer.
                if !self.dirty {
                    if let Some(id) = self.current.as_ref().map(|e| e.id.clone()) {
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
    }

    /// `set_current` — remote-change guard + conflict upsert, then fill the
    /// M3 editor (via `pending_fill`) when the swap is safe.
    pub(crate) fn set_current(&mut self, entry: Entry, cx: &mut Context<Self>) {
        let raw = serde_json::from_str(&entry.content_json).unwrap_or(serde_json::Value::Null);
        let markdown = content::read_entry_markdown(&raw);
        let current_md = self
            .editor
            .as_ref()
            .map(|e| e.read(cx).markdown().to_string())
            .unwrap_or_default();
        let decision =
            refresh::refresh_decision(&entry, self.current.as_ref(), &current_md, self.dirty);
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
                self.pending_fill = Some((entry.title.clone(), markdown));
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
