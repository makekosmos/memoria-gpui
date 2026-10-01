//! `FakeArk` — an in-memory Engine used by the run-level tests.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use serde_json::{json, Value};

use crate::store::transport::{ArkBridge, EngineError};

/// In-memory fake Engine: object/link/type tables + vault grant bookkeeping.
#[derive(Clone, Default)]
pub(super) struct FakeArk {
    pub(super) state: Rc<RefCell<ArkState>>,
}

#[derive(Default)]
pub(super) struct ArkState {
    pub(super) objects: BTreeMap<String, Value>,
    pub(super) links: BTreeMap<String, Value>,
    pub(super) object_types: BTreeMap<String, Value>,
    pub(super) vault_files: Vec<Value>,
    pub(super) vault_images: Vec<Value>,
    pub(super) exported_files: Vec<Value>,
    pub(super) registered_dirs: Vec<String>,
    pub(super) closed_roots: Vec<String>,
    pub(super) fail_next_upsert_link: bool,
    pub(super) fail_object_ids: Vec<String>,
}

impl FakeArk {
    pub(super) fn object(&self, id: &str) -> Value {
        self.state
            .borrow()
            .objects
            .get(id)
            .cloned()
            .unwrap_or(Value::Null)
    }

    pub(super) fn put_object(&self, object: Value) {
        if let Some(id) = object.get("id").and_then(Value::as_str) {
            self.state.borrow_mut().objects.insert(id.into(), object);
        }
    }
}

impl ArkBridge for FakeArk {
    fn rpc(&self, operation: &str, params: Value) -> Result<Value, EngineError> {
        match operation {
            "get_object" => {
                let id = params.get("id").and_then(Value::as_str).unwrap_or("");
                Ok(self.object(id))
            }
            "list_objects"
            | "list_objects_by_type"
            | "list_object_summaries"
            | "list_object_summaries_by_type" => {
                let type_id = params.get("type_id").and_then(Value::as_str);
                Ok(Value::Array(
                    self.state
                        .borrow()
                        .objects
                        .values()
                        .filter(|o| {
                            type_id.is_none() || o.get("typeId").and_then(Value::as_str) == type_id
                        })
                        .cloned()
                        .collect(),
                ))
            }
            "list_object_types" => Ok(Value::Array(
                self.state.borrow().object_types.values().cloned().collect(),
            )),
            "get_object_type" => {
                let id = params.get("type_id").and_then(Value::as_str).unwrap_or("");
                Ok(self
                    .state
                    .borrow()
                    .object_types
                    .get(id)
                    .cloned()
                    .unwrap_or(Value::Null))
            }
            "list_object_links" => Ok(Value::Array(
                self.state.borrow().links.values().cloned().collect(),
            )),
            "upsert_object" => {
                let object = params.get("object").cloned().unwrap_or(params.clone());
                let id = object.get("id").and_then(Value::as_str).unwrap_or("");
                if self.state.borrow().fail_object_ids.iter().any(|f| f == id) {
                    return Err(EngineError::engine("injected upsert failure"));
                }
                self.put_object(object);
                Ok(Value::Null)
            }
            "upsert_object_type" => {
                let object_type = params.get("object_type").cloned().unwrap_or(params.clone());
                let id = object_type.get("id").and_then(Value::as_str).unwrap_or("");
                self.state
                    .borrow_mut()
                    .object_types
                    .insert(id.into(), object_type);
                Ok(Value::Null)
            }
            "delete_object" => {
                let id = params.get("id").and_then(Value::as_str).unwrap_or("");
                Ok(json!(self.state.borrow_mut().objects.remove(id).is_some()))
            }
            "upsert_object_link" => {
                if self.state.borrow().fail_next_upsert_link {
                    self.state.borrow_mut().fail_next_upsert_link = false;
                    return Err(EngineError::engine("syncRelatedLinks failed"));
                }
                let link = params.get("object_link").cloned().unwrap_or(params.clone());
                let id = link
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                self.state.borrow_mut().links.insert(id, link);
                Ok(Value::Null)
            }
            "delete_object_link" => {
                let id = params.get("id").and_then(Value::as_str).unwrap_or("");
                Ok(json!(self.state.borrow_mut().links.remove(id).is_some()))
            }
            "filesystem.vault.open" => {
                let state = self.state.borrow();
                Ok(json!({
                    "rootId": "root-1",
                    "name": "vault",
                    "vaultKey": "c:/vault",
                    "files": state.vault_files,
                    "images": state.vault_images,
                }))
            }
            "filesystem.vault.register" => {
                let path = params.get("path").and_then(Value::as_str).unwrap_or("");
                self.state
                    .borrow_mut()
                    .registered_dirs
                    .push(path.to_string());
                Ok(json!({ "rootId": "root-1", "vaultKey": "c:/export" }))
            }
            "filesystem.vault.export" => {
                let files = params.get("files").cloned().unwrap_or(Value::Array(vec![]));
                let count = files.as_array().map(|f| f.len()).unwrap_or(0);
                self.state.borrow_mut().exported_files =
                    files.as_array().cloned().unwrap_or_default();
                Ok(json!({ "exportedCount": count }))
            }
            "filesystem.vault.read" => Ok(json!({ "bytesBase64": "" })),
            "filesystem.vault.close" => {
                let root = params.get("rootId").and_then(Value::as_str).unwrap_or("");
                self.state.borrow_mut().closed_roots.push(root.to_string());
                Ok(Value::Null)
            }
            _ => Err(EngineError::engine(&format!("unexpected op {operation}"))),
        }
    }
}

pub(super) fn summary_fixture() -> (Value, Value) {
    let iso = "2026-01-01T00:00:00.001Z";
    let object = json!({
        "id": "note-a",
        "typeId": "com.kosmos.note",
        "title": "Note A",
        "contentJson": { "type": "markdown", "version": 1, "text": "old" },
        "propsJson": {
            "memoria_type_id": "note_obj",
            "memoria_record_kind": "entry",
        },
        "createdAt": iso,
        "updatedAt": iso,
        "deletedAt": null,
    });
    let link = json!({
        "id": "note-a:related:target",
        "sourceObjectId": "note-a",
        "targetObjectId": "old-target",
        "linkType": "related",
        "createdAt": iso,
    });
    (object, link)
}
