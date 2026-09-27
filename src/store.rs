//! Engine is the only owner of note persistence — no database or local mirror.
//! A worker thread owns `EntryApi`; the UI sends `Command`s and drains `Reply`s
//! (agenda-gpui `Worker` pattern). Engine change events arrive as
//! `Reply::Event` so the list can refresh without an app restart.

pub mod app_network_api;
pub mod bubble_api;
mod bubble_migrate;
mod entry_api;
pub(crate) mod note_type_api;
#[cfg(test)]
mod tests;
pub mod transport;
mod trash_api;
mod worker_extra;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use crate::diary::{BubbleKind, BubbleTimelineNode};
use crate::model::{DeleteEntryResult, Entry, NoteType, SaveEntryResult, SearchResult};

pub use bubble_api::{BubbleApi, BubblePatch};
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
    /// `listTrashEntries` — soft-deleted objects, newest deleted first.
    LoadTrash,
    /// `restoreEntry` — clears `deletedAt` and bumps `updatedAt`.
    RestoreEntry(String),
    /// `permanentDeleteEntry` — hard delete (trash only).
    DeleteForever(String),
    /// `listBubbles` — thread-normalized diary bubbles.
    ListBubbles,
    /// `createBubble` — `input` parses `#tag`s; `content_json` keeps the
    /// composer tiptap doc so rich nodes persist verbatim.
    CreateBubble {
        input: String,
        kind: BubbleKind,
        parent_id: Option<String>,
        content_json: Option<serde_json::Value>,
    },
    /// `updateBubble` — text/kind patch; preserves occurrence + extra props.
    UpdateBubble { id: String, patch: BubblePatch },
    /// `deleteBubble` — object plus its `reply_to` links.
    DeleteBubble(String),
    /// `migrateBubble` — deterministic id + read-back check.
    MigrateBubble {
        namespace: String,
        source_id: String,
        bubble: Box<BubbleTimelineNode>,
    },
    /// `migrateLocalBubbles` + `migrateJournalEntries` — diary start-up
    /// migration: the local-storage blob is imported first, then dated legacy
    /// journal entries become bubbles; entries whose bubbles all migrated are
    /// deleted (Vue `deleteImportedJournalEntries`).
    MigrateDiary {
        local_bubbles_json: Option<serde_json::Value>,
    },
    /// `bookMetadata.lookupIsbn` — Engine-side Open Library lookup.
    LookupIsbn(String),
    /// `bookMetadata.fetchPage` — normalized `{finalUrl, html}` for local
    /// extraction (`book_metadata_extract` runs in-app, like Vue's DOMParser).
    FetchBookPage(String),
    /// `images.dominantColor` — cover spine color for a URL or local path.
    DominantColor(String),
    /// `images.storeCover` — persist a dropped cover file via Engine.
    StoreCover {
        source_path: String,
        entry_id: String,
    },
    /// `images.fetch` — remote image → Engine-stored local path + color.
    FetchImage(String),
}

pub enum Reply {
    List(Result<Vec<Entry>, String>),
    Entry {
        id: String,
        result: Result<Option<Entry>, String>,
    },
    Saved {
        /// Entry id — multiple documents (note + sticker windows) share one
        /// worker, so saves route back to the owning document.
        id: String,
        result: Result<SaveEntryResult, String>,
    },
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
    Trash(Result<Vec<Entry>, String>),
    Restored {
        id: String,
        result: Result<DeleteEntryResult, String>,
    },
    /// `bookMetadata.lookupIsbn` result — `None` = ISBN not found.
    BookMetadata(Result<Option<crate::book_metadata::BookMetadata>, String>),
    /// `bookMetadata.fetchPage` result — page for local extraction.
    BookMetadataPage(Result<Option<crate::book_metadata::BookMetadataPage>, String>),
    /// `images.dominantColor` result for the requested source.
    DominantColor {
        source: String,
        result: Result<Option<String>, String>,
    },
    /// `images.storeCover` result — the stored local path.
    CoverStored {
        entry_id: String,
        result: Result<String, String>,
    },
    /// `images.fetch` result — `(stored_path, dominant color)`.
    ImageFetched {
        url: String,
        result: Result<(String, Option<String>), String>,
    },
    /// Engine push — `Online`/`Offline`/`Changed(payload)`.
    Event(EngineEvent),
    Bubbles(Result<Vec<BubbleTimelineNode>, String>),
    BubbleCreated(Result<String, String>),
    BubbleUpdated {
        id: String,
        result: Result<(), String>,
    },
    BubbleDeleted {
        id: String,
        result: Result<(), String>,
    },
    BubbleMigrated {
        source_id: String,
        result: Result<String, String>,
    },
    /// Result of `MigrateDiary` — `Ok(Some(remaining))` carries the
    /// local-blob sources that could not migrate (empty ⇒ clear the blob,
    /// like `localStorage.removeItem`); `Ok(None)` means the blob was
    /// absent/invalid so the storage key stays untouched.
    DiaryMigrated(Result<Option<Vec<serde_json::Value>>, String>),
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
            let mut api = EntryApi::<Engine>::new(engine.clone());
            let bubbles = BubbleApi::<Engine>::new(engine.clone());
            let mut trash = TrashStorageApi::<Engine>::new(engine);
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
                        let result = api.save_entry(entry.as_ref()).map_err(err_string);
                        Reply::Saved {
                            id: entry.id.clone(),
                            result,
                        }
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
                    Command::LoadTrash => {
                        Reply::Trash(trash.list_trash_entries().map_err(err_string))
                    }
                    Command::RestoreEntry(id) => {
                        let result = trash.restore_entry(&id).map_err(err_string);
                        Reply::Restored { id, result }
                    }
                    Command::DeleteForever(id) => {
                        let result = trash.permanent_delete_entry(&id).map_err(err_string);
                        Reply::Deleted { id, result }
                    }
                    other => worker_extra::dispatch_bubble_or_network(other, &mut api, &bubbles)
                        .unwrap_or_else(|| unreachable!("unhandled Command")),
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
