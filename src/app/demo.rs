//! In-memory Engine stand-in for `MEMORIA_DEMO=1` and UI tests. It answers
//! `Command`s with the same observable semantics as `EntryApi`/`TrashApi`:
//! entries sort `updated_at` desc, trash sorts `deleted_at` desc, search
//! matches title/body case-insensitively.
use memoria_gpui::model::{DeleteEntryResult, Entry, NoteType, SaveEntryResult, SearchResult};
use memoria_gpui::store::{Command, Reply};
use memoria_gpui::system_types_data::{
    system_types, SYSTEM_TYPE_BOOK_ID, SYSTEM_TYPE_COLLECTION_ID, SYSTEM_TYPE_NOTE_ID,
};
use memoria_gpui::time::now_millis;
mod ark;
pub(crate) use ark::DemoArk;

pub(crate) struct DemoStore {
    pub entries: Vec<Entry>,
    pub note_types: Vec<NoteType>,
    pub ark: DemoArk,
    seq: u64,
}

fn markdown_body(text: &str) -> String {
    memoria_gpui::content::write_entry_markdown(text).to_string()
}

impl DemoStore {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            note_types: system_types(),
            ark: DemoArk::default(),
            seq: 0,
        }
    }

    /// `MEMORIA_DEMO=1` seed — a small believable workspace.
    pub fn seeded() -> Self {
        let mut store = Self::new();
        store.entries = vec![
            store.entry(
                "n-1",
                SYSTEM_TYPE_NOTE_ID,
                "Планы на неделю",
                "# Планы\n\n- Собрать дизайн-ревью\n- Написать Jack'у\n- Проверить корзину",
            ),
            store.entry(
                "n-2",
                SYSTEM_TYPE_NOTE_ID,
                "Список покупок",
                "Молоко, хлеб, кофе.",
            ),
            store.entry("b-1", SYSTEM_TYPE_BOOK_ID, "Война и мир", "Роман Толстого."),
            store.entry(
                "b-2",
                SYSTEM_TYPE_BOOK_ID,
                "Мастер и Маргарита",
                "Роман Булгакова.",
            ),
        ];
        store.entries[2].header_props_json = Some(r#"{"author":"Лев Толстой"}"#.into());
        store
    }

    fn entry(&mut self, id: &str, type_id: &str, title: &str, body: &str) -> Entry {
        self.seq += 1;
        Entry {
            id: id.into(),
            title: title.into(),
            content_json: markdown_body(body),
            content_loaded: Some(true),
            created_at: 1_767_225_600_000 + self.seq as i64,
            updated_at: 1_767_225_600_000 + self.seq as i64,
            type_id: Some(type_id.into()),
            ..Default::default()
        }
    }

    pub(crate) fn dispatch(&mut self, command: Command) -> Vec<Reply> {
        let reply = match command {
            Command::LoadList(type_ids) => {
                let mut list: Vec<Entry> = self
                    .entries
                    .iter()
                    .filter(|e| e.deleted_at.is_none())
                    .filter(|e| {
                        type_ids.is_empty()
                            || e.type_id
                                .as_deref()
                                .map(|t| type_ids.iter().any(|s| s == t))
                                .unwrap_or(false)
                    })
                    .cloned()
                    .collect();
                list.sort_by_key(|e| std::cmp::Reverse(e.updated_at));
                Reply::List(Ok(list))
            }
            Command::LoadEntry { id, .. } => {
                let found = self.entries.iter().find(|e| e.id == id).cloned();
                Reply::Entry {
                    id,
                    result: Ok(found),
                }
            }
            Command::SaveEntry(entry) => {
                let mut entry = *entry;
                entry.updated_at = now_millis();
                match self.entries.iter_mut().find(|e| e.id == entry.id) {
                    Some(slot) => *slot = entry.clone(),
                    None => self.entries.push(entry.clone()),
                }
                Reply::Saved(Ok(SaveEntryResult::ok(entry.id)))
            }
            Command::DeleteEntry(id) => {
                if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
                    e.deleted_at = Some(now_millis());
                }
                Reply::Deleted {
                    id: id.clone(),
                    result: Ok(DeleteEntryResult {
                        ok: true,
                        entry_id: Some(id),
                        ..Default::default()
                    }),
                }
            }
            Command::DeleteForever(id) => {
                self.entries.retain(|e| e.id != id);
                Reply::Deleted {
                    id: id.clone(),
                    result: Ok(DeleteEntryResult {
                        ok: true,
                        entry_id: Some(id),
                        ..Default::default()
                    }),
                }
            }
            Command::RestoreEntry(id) => {
                if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
                    e.deleted_at = None;
                    e.updated_at = now_millis();
                }
                Reply::Restored {
                    id: id.clone(),
                    result: Ok(DeleteEntryResult {
                        ok: true,
                        entry_id: Some(id),
                        ..Default::default()
                    }),
                }
            }
            Command::LoadTrash => {
                let mut trash: Vec<Entry> = self
                    .entries
                    .iter()
                    .filter(|e| e.deleted_at.is_some())
                    .cloned()
                    .collect();
                trash.sort_by_key(|e| std::cmp::Reverse(e.deleted_at.unwrap_or(0)));
                Reply::Trash(Ok(trash))
            }
            Command::LoadNoteTypes => Reply::NoteTypes(Ok(self.note_types.clone())),
            Command::EnsureCollections => {
                // `ensureCollectionObjects` — one `collection:<type>` entry per
                // note type shown as a collection (Vue kepler-entry-mappers).
                let mut created = Vec::new();
                for nt in self.note_types.clone() {
                    if !memoria_gpui::system_types::should_show_as_eden_collection(&nt.id) {
                        continue;
                    }
                    let id = format!("collection:{}", nt.id);
                    if self.entries.iter().any(|e| e.id == id) {
                        continue;
                    }
                    let mut e = self.entry(
                        &id,
                        SYSTEM_TYPE_COLLECTION_ID,
                        &memoria_gpui::note_types::get_note_type_collection_name(Some(&nt)),
                        "",
                    );
                    e.header_props_json = Some(format!(r#"{{"object_type_id":"{}"}}"#, nt.id));
                    self.entries.push(e.clone());
                    created.push(e);
                }
                Reply::Collections(Ok(created))
            }
            Command::Search(query) => {
                let q = query.to_lowercase();
                let mut results = Vec::new();
                for e in self.entries.iter().filter(|e| e.deleted_at.is_none()) {
                    if q.is_empty() {
                        break;
                    }
                    let title_hit = e.title.to_lowercase().contains(&q);
                    let body =
                        memoria_gpui::preview::entry_preview(&e.content_json, 800).to_lowercase();
                    let body_hit = !title_hit && body.contains(&q);
                    if title_hit || body_hit {
                        results.push(SearchResult {
                            entry_id: e.id.clone(),
                            text: if body_hit {
                                body.lines()
                                    .find(|l| l.contains(&q))
                                    .unwrap_or_default()
                                    .to_string()
                            } else {
                                e.title.clone()
                            },
                            ..Default::default()
                        });
                    }
                }
                Reply::Search {
                    query,
                    result: Ok(results),
                }
            }
            command => self.dispatch_bubble(command),
        };
        vec![reply]
    }
}
