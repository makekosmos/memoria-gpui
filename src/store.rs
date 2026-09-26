//! Engine is the only owner of note persistence — no database or local mirror.
//! A worker thread owns `EntryApi`; the UI sends `Command`s and drains `Reply`s
//! (agenda-gpui `Worker` pattern). Engine change events arrive as
//! `Reply::Event` so the list can refresh without an app restart.

mod entry_api;
mod note_type_api;
#[cfg(test)]
mod tests;
pub mod transport;
mod trash_api;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use crate::model::{DeleteEntryResult, Entry, NoteType, SaveEntryResult, SearchResult};

pub use entry_api::EntryApi;
pub use note_type_api::{NoteTypeApi, MEMORIA_NOTE_TYPE_PROP};
pub use transport::{ArkBridge, Engine, EngineError, EngineEvent};
pub use trash_api::TrashStorageApi;

pub enum Command {
    /// `listEntries` — summaries for the given visible type ids (`[]` = all).
    LoadList(Vec<String>),
    /// `loadEntry` — full object + links. `content_only` skips links.
    LoadEntry { id: String, content_only: bool },
    /// `saveEntry` — full guard chain; Engine write happens last.
    SaveEntry(Box<Entry>),
    /// `deleteEntry` — soft delete via Engine.
    DeleteEntry(String),
    /// `listNoteTypes` — custom + legacy + system types.
    LoadNoteTypes,
    /// `ensureCollectionObjects` — collection stubs for system types.
    EnsureCollections,
    /// `searchEntries` — full-text search; empty query clears.
    Search(String),
}

pub enum Reply {
    List(Result<Vec<Entry>, String>),
    Entry {
        id: String,
        result: Result<Option<Entry>, String>,
    },
    Saved(Result<SaveEntryResult, String>),
    Deleted {
        id: String,
        result: Result<DeleteEntryResult, String>,
    },
    NoteTypes(Result<Vec<NoteType>, String>),
    Collections(Result<Vec<Entry>, String>),
    Search {
        query: String,
        result: Result<Vec<SearchResult>, String>,
    },
    /// Engine push — `Online`/`Offline`/`Changed(payload)`.
    Event(EngineEvent),
}

pub struct Worker {
    pub commands: Sender<Command>,
    pub replies: Receiver<Reply>,
    stop: Arc<AtomicBool>,
}

impl Worker {
    pub fn start() -> Self {
        Self::with_engine(Engine::default())
    }

    /// Test seam — explicit data dir / Engine config.
    pub fn with_engine(engine: Engine) -> Self {
        let (commands, requests) = mpsc::channel::<Command>();
        let (results, replies) = mpsc::channel::<Reply>();
        let stop = Arc::new(AtomicBool::new(false));
        let sub_stop = stop.clone();
        std::thread::spawn(move || {
            {
                // Forward Engine push events as replies.
                let results = results.clone();
                let (events, forward) = mpsc::channel::<EngineEvent>();
                engine.subscribe_events(events, sub_stop);
                std::thread::spawn(move || {
                    for event in forward {
                        if results.send(Reply::Event(event)).is_err() {
                            break;
                        }
                    }
                });
            }
            let mut api = EntryApi::<Engine>::new(engine);
            for request in requests {
                let reply = match request {
                    Command::LoadList(type_ids) => {
                        Reply::List(api.list_entries(&type_ids).map_err(err_string))
                    }
                    Command::LoadEntry { id, content_only } => {
                        let result = api.load_entry(&id, content_only).map_err(err_string);
                        Reply::Entry { id, result }
                    }
                    Command::SaveEntry(entry) => {
                        Reply::Saved(api.save_entry(entry.as_ref()).map_err(err_string))
                    }
                    Command::DeleteEntry(id) => {
                        let result = api.delete_entry(&id).map_err(err_string);
                        Reply::Deleted { id, result }
                    }
                    Command::LoadNoteTypes => {
                        Reply::NoteTypes(api.note_types.list_note_types().map_err(err_string))
                    }
                    Command::EnsureCollections => {
                        let types = api.note_types.list_note_types().unwrap_or_default();
                        Reply::Collections(
                            api.note_types
                                .ensure_collection_objects(&types)
                                .map_err(err_string),
                        )
                    }
                    Command::Search(query) => {
                        let result = api.search_entries(&query).map_err(err_string);
                        Reply::Search { query, result }
                    }
                };
                if results.send(reply).is_err() {
                    break;
                }
            }
        });
        Self {
            commands,
            replies,
            stop,
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn err_string(error: EngineError) -> String {
    error.to_string()
}
