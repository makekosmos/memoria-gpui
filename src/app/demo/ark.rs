//! Demo bubble backend — an in-memory ARK object/link store wired through
//! the real `BubbleApi`, so `MEMORIA_DEMO=1` and UI tests exercise the same
//! code path as Engine (`reply_to` links, migration, propsJson contract).

use memoria_gpui::model::Entry;
use memoria_gpui::store::bubble_api::{migrate_journal_entry, migrate_local_blob, BubbleApi};
use memoria_gpui::store::transport::{ArkBridge, EngineError};
use memoria_gpui::store::{Command, Reply};
use memoria_gpui::time::now_millis;
use serde_json::{Map, Value};
use std::sync::{Arc, Mutex};

use super::DemoStore;

/// In-memory ARK — the demo/test stand-in for Engine object/link tables.
#[derive(Clone, Default)]
pub(crate) struct DemoArk {
    pub objects: Arc<Mutex<Map<String, Value>>>,
    pub links: Arc<Mutex<Map<String, Value>>>,
}

impl ArkBridge for DemoArk {
    fn rpc(&self, operation: &str, params: Value) -> Result<Value, EngineError> {
        match operation {
            "list_objects_by_type" => {
                let type_id = params["type_id"].as_str().unwrap_or_default();
                Ok(Value::Array(
                    self.objects
                        .lock()
                        .unwrap()
                        .values()
                        .filter(|o| o["typeId"] == type_id)
                        .cloned()
                        .collect(),
                ))
            }
            "list_object_links" => Ok(Value::Array(
                self.links.lock().unwrap().values().cloned().collect(),
            )),
            "get_object" => Ok(self
                .objects
                .lock()
                .unwrap()
                .get(params["id"].as_str().unwrap_or_default())
                .cloned()
                .unwrap_or(Value::Null)),
            "upsert_object" => {
                let object = params["object"].clone();
                self.objects.lock().unwrap().insert(
                    object["id"].as_str().unwrap_or_default().to_string(),
                    object,
                );
                Ok(Value::from(true))
            }
            "delete_object" => {
                if let Some(object) = self
                    .objects
                    .lock()
                    .unwrap()
                    .get_mut(params["id"].as_str().unwrap_or_default())
                {
                    object["deletedAt"] =
                        Value::from(memoria_gpui::time::millis_to_iso(now_millis()));
                }
                Ok(Value::from(true))
            }
            "upsert_object_link" => {
                let link = params["object_link"].clone();
                self.links
                    .lock()
                    .unwrap()
                    .insert(link["id"].as_str().unwrap_or_default().to_string(), link);
                Ok(Value::from(true))
            }
            "delete_object_link" => {
                self.links
                    .lock()
                    .unwrap()
                    .shift_remove(params["id"].as_str().unwrap_or_default());
                Ok(Value::from(true))
            }
            _ => Ok(Value::Null),
        }
    }
}

impl DemoStore {
    /// Bubble ops run through the real `BubbleApi` on the in-memory ARK.
    fn bubble_api(&self) -> BubbleApi<DemoArk> {
        BubbleApi::new(self.ark.clone())
    }

    /// Dispatch for the `Command::*Bubble*` / `MigrateDiary` family.
    pub(crate) fn dispatch_bubble(&mut self, command: Command) -> Reply {
        match command {
            Command::ListBubbles => {
                Reply::Bubbles(self.bubble_api().list_bubbles().map_err(|e| e.to_string()))
            }
            Command::CreateBubble {
                input,
                kind,
                parent_id,
                content_json,
            } => Reply::BubbleCreated(
                self.bubble_api()
                    .create_bubble(&input, kind, parent_id.as_deref(), content_json)
                    .map_err(|e| e.to_string()),
            ),
            Command::UpdateBubble { id, patch } => Reply::BubbleUpdated {
                result: self
                    .bubble_api()
                    .update_bubble(&id, patch)
                    .map_err(|e| e.to_string()),
                id,
            },
            Command::DeleteBubble(id) => Reply::BubbleDeleted {
                result: self
                    .bubble_api()
                    .delete_bubble(&id)
                    .map_err(|e| e.to_string()),
                id,
            },
            Command::MigrateBubble {
                namespace,
                source_id,
                bubble,
            } => Reply::BubbleMigrated {
                result: self
                    .bubble_api()
                    .migrate_bubble(&namespace, &source_id, &bubble)
                    .map_err(|e| e.to_string()),
                source_id,
            },
            Command::MigrateDiary { local_bubbles_json } => {
                let api = self.bubble_api();
                let remaining = local_bubbles_json
                    .as_ref()
                    .and_then(|blob| migrate_local_blob(&api, blob));
                // Demo journal migration — same dated-entry rule as
                // `migrate_diary`, reading the in-memory entry list directly.
                let mut legacy: Vec<Entry> = self
                    .entries
                    .iter()
                    .filter(|e| memoria_gpui::diary::is_legacy_dated_journal_entry(e))
                    .cloned()
                    .collect();
                legacy.sort_by(|l, r| {
                    r.title
                        .cmp(&l.title)
                        .then_with(|| r.created_at.cmp(&l.created_at))
                });
                let deletable: Vec<String> = legacy
                    .iter()
                    .filter(|e| migrate_journal_entry(&api, e))
                    .map(|e| e.id.clone())
                    .collect();
                for id in &deletable {
                    if let Some(e) = self.entries.iter_mut().find(|e| &e.id == id) {
                        e.deleted_at = Some(now_millis());
                    }
                }
                Reply::DiaryMigrated(Ok(remaining))
            }
            _ => unreachable!("dispatch_bubble only receives bubble commands"),
        }
    }
}
