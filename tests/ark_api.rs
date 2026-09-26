//! Port of the bridge-driven halves of `tests/memoriaMigration.test.ts` —
//! `createEntryApi`/`createNoteTypeApi`/`createTrashStorageApi` exercised
//! through an in-memory `ArkBridge` (the Vue `ark` request fn seam).

use memoria_gpui::store::transport::{ArkBridge, EngineError};
use memoria_gpui::store::{EntryApi, NoteTypeApi, TrashStorageApi, MEMORIA_NOTE_TYPE_PROP};
use memoria_gpui::system_types_data::SYSTEM_TYPE_IMAGE;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

/// Closure-backed `ArkBridge` — the `ark: ArkRequest` injection point.
#[derive(Clone)]
struct FakeArk(Arc<dyn Fn(&str, &Value) -> Result<Value, EngineError> + Send + Sync>);

impl FakeArk {
    fn new(f: impl Fn(&str, &Value) -> Result<Value, EngineError> + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }
}

impl ArkBridge for FakeArk {
    fn rpc(&self, operation: &str, params: Value) -> Result<Value, EngineError> {
        (self.0)(operation, &params)
    }
}

fn record(id: &str, type_id: &str) -> Value {
    json!({
        "id": id,
        "typeId": type_id,
        "title": id,
        "contentJson": { "type": "doc", "content": [] },
        "propsJson": {},
        "createdAt": "2026-01-01T00:00:00.000Z",
        "updatedAt": "2026-01-01T00:00:00.000Z",
        "deletedAt": null,
    })
}

#[test]
fn delete_entry_uses_the_authorized_delete_object_operation() {
    let calls = Arc::new(Mutex::new(Vec::<(String, Value)>::new()));
    let calls_c = calls.clone();
    let bridge = FakeArk::new(move |op, params| {
        calls_c
            .lock()
            .unwrap()
            .push((op.to_string(), params.clone()));
        match op {
            "get_object" => Ok(record("note-a", "com.kosmos.note")),
            "delete_object" => Ok(Value::from(true)),
            _ => panic!("unexpected operation {op}"),
        }
    });
    let mut api = EntryApi::new(bridge);

    let result = api.delete_entry("note-a").unwrap();
    assert!(result.ok);
    assert_eq!(result.entry_id.as_deref(), Some("note-a"));

    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.iter().map(|(op, _)| op.as_str()).collect::<Vec<_>>(),
        ["get_object", "delete_object"]
    );
    // `_req_id`/`operation` are attached inside `Engine::rpc`, below the
    // bridge seam — the fake sees the raw op params only.
    assert_eq!(calls[0].1, json!({ "id": "note-a" }));
    assert_eq!(calls[1].1, json!({ "id": "note-a" }));
}

#[test]
fn save_note_type_registers_with_the_explicit_canonical_version() {
    let request = Arc::new(Mutex::new(None::<(String, Value)>));
    let request_c = request.clone();
    let bridge = FakeArk::new(move |op, params| {
        *request_c.lock().unwrap() = Some((op.to_string(), params.clone()));
        match op {
            "upsert_object" => Ok(Value::from(true)),
            _ => panic!("unexpected operation {op}"),
        }
    });
    let api = NoteTypeApi::new(bridge);

    let result = api.save_note_type(&SYSTEM_TYPE_IMAGE).unwrap();
    assert!(result.is_ok());

    let (op, params) = request.lock().unwrap().clone().unwrap();
    assert_eq!(op, "upsert_object");
    let object = &params["object"];
    assert_eq!(object["typeId"], Value::from("com.kosmos.note"));
    assert_eq!(object["typeVersion"], Value::from("1.0.0"));
    assert_eq!(
        object["contentJson"],
        json!({ "type": "doc", "content": [] })
    );
    let extensions = &object["propsJson"]["extensions"];
    assert_eq!(extensions["memoria_record_kind"], Value::from("note_type"));
    assert_eq!(
        extensions["note_type_id"],
        Value::from(SYSTEM_TYPE_IMAGE.id.clone())
    );
    let note_type = &extensions[MEMORIA_NOTE_TYPE_PROP];
    assert_eq!(note_type["id"], Value::from(SYSTEM_TYPE_IMAGE.id.clone()));
    assert_eq!(
        object["id"],
        Value::from(format!("memoria:type:{}", SYSTEM_TYPE_IMAGE.id))
    );
}

#[test]
fn note_types_read_canonical_extensions_and_legacy_content() {
    let mut canonical_type = SYSTEM_TYPE_IMAGE.clone();
    canonical_type.id = "custom_image_obj".into();
    canonical_type.name = "Custom image".into();
    canonical_type.slug = "custom-image".into();
    let mut legacy_type = SYSTEM_TYPE_IMAGE.clone();
    legacy_type.id = "legacy_image_obj".into();
    legacy_type.name = "Legacy image".into();
    legacy_type.slug = "legacy-image".into();

    let canonical = json!({
        "id": "memoria:type:custom_image_obj",
        "typeId": "com.kosmos.note",
        "typeVersion": "1.0.0",
        "title": canonical_type.name.clone(),
        "contentJson": { "type": "doc", "content": [] },
        "propsJson": {
            "description": null,
            "extensions": {
                "memoria_record_kind": "note_type",
                "note_type_id": canonical_type.id.clone(),
                "memoria_note_type": serde_json::to_value(&canonical_type).unwrap(),
            },
        },
        "createdAt": "1970-01-01T00:00:00.001Z",
        "updatedAt": "1970-01-01T00:00:00.001Z",
        "deletedAt": null,
    });
    let legacy = json!({
        "id": "memoria:type:legacy_image_obj",
        "typeId": "com.kosmos.note",
        "title": legacy_type.name.clone(),
        "contentJson": serde_json::to_value(&legacy_type).unwrap(),
        "propsJson": {
            "memoria_record_kind": "note_type",
            "note_type_id": legacy_type.id.clone(),
        },
        "createdAt": "1970-01-01T00:00:00.001Z",
        "updatedAt": "1970-01-01T00:00:00.001Z",
        "deletedAt": null,
    });

    let bridge = FakeArk::new(move |op, _params| match op {
        "list_objects_by_type" => Ok(json!({ "objects": [canonical.clone(), legacy.clone()] })),
        "list_object_types" => Ok(json!({ "types": [] })),
        _ => panic!("unexpected operation {op}"),
    });
    let api = NoteTypeApi::new(bridge);
    let listed = api.list_note_types().unwrap();

    assert!(listed
        .iter()
        .any(|t| t.id == "custom_image_obj" && t.name == "Custom image"));
    assert!(listed
        .iter()
        .any(|t| t.id == "legacy_image_obj" && t.name == "Legacy image"));
}

#[test]
fn list_all_entries_queries_compat_types_and_preserves_ids() {
    let queried = Arc::new(Mutex::new(Vec::<String>::new()));
    let queried_c = queried.clone();
    let bridge = FakeArk::new(move |op, params| match op {
        "list_object_types" => Ok(json!({ "types": [{ "id": "custom_obj" }] })),
        "list_object_links" => Ok(json!({ "links": [] })),
        "list_objects_by_type" => {
            let type_id = params["type_id"].as_str().unwrap().to_string();
            queried_c.lock().unwrap().push(type_id.clone());
            let mut object = record(&format!("object:{type_id}"), &type_id);
            if type_id == "collection_obj" {
                object["propsJson"] = json!({ "object_type_id": "note_obj" });
            }
            Ok(json!({ "objects": [object] }))
        }
        _ => panic!("unexpected operation {op}"),
    });
    let mut api = EntryApi::new(bridge);

    let entries = api.list_all_entries().unwrap();
    assert_eq!(
        queried.lock().unwrap().as_slice(),
        [
            "com.kosmos.note",
            "note_obj",
            "task_obj",
            "collection_obj",
            "custom_obj"
        ]
    );
    assert_eq!(
        entries.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
        [
            "object:com.kosmos.note",
            "object:note_obj",
            "object:task_obj",
            "object:collection_obj",
            "object:custom_obj"
        ]
    );
}

#[test]
fn list_entries_merges_full_records_on_partial_summary_failure() {
    let bridge = FakeArk::new(|op, params| match op {
        "list_object_types" => Ok(json!({ "types": [{ "id": "custom_obj" }] })),
        "list_object_links" => Ok(json!({ "links": [] })),
        "list_object_summaries_by_type" => {
            if params["type_id"] == "com.kosmos.note" {
                Ok(json!({ "objects": [{
                    "id": "summary:canonical",
                    "typeId": "com.kosmos.note",
                    "title": "Canonical",
                    "propsJson": {},
                    "createdAt": "2026-01-01T00:00:00.000Z",
                    "updatedAt": "2026-01-01T00:00:00.000Z",
                    "deletedAt": null,
                }] }))
            } else {
                Err(EngineError::Unreachable)
            }
        }
        "list_objects_by_type" => {
            let type_id = params["type_id"].as_str().unwrap().to_string();
            if type_id == "note_obj" || type_id == "custom_obj" {
                Ok(json!({ "objects": [record(&format!("full:{type_id}"), &type_id)] }))
            } else {
                Ok(json!({ "objects": [] }))
            }
        }
        _ => panic!("unexpected operation {op}"),
    });
    let mut api = EntryApi::new(bridge);

    let entries = api.list_entries(&[]).unwrap();
    let ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
    assert!(ids.contains(&"summary:canonical"));
    assert!(ids.contains(&"full:note_obj"));
    assert!(ids.contains(&"full:custom_obj"));
}

#[test]
fn trash_storage_counts_active_and_trash_records() {
    let bridge = FakeArk::new(|op, params| match op {
        "list_object_types" => Ok(json!({ "types": [{ "id": "custom_obj" }] })),
        "list_object_links" => Ok(json!({ "links": [] })),
        "list_objects_by_type" => match params["type_id"].as_str().unwrap() {
            "note_obj" => Ok(json!({ "objects": [record("legacy:active", "note_obj")] })),
            "custom_obj" => Ok(json!({ "objects": [{
                "id": "custom:trash",
                "typeId": "custom_obj",
                "title": "Custom",
                "contentJson": { "type": "doc", "content": [] },
                "propsJson": {},
                "createdAt": "2026-01-01T00:00:00.000Z",
                "updatedAt": "2026-01-01T00:00:00.000Z",
                "deletedAt": "2026-01-02T00:00:00.000Z",
            }] })),
            _ => Ok(json!({ "objects": [] })),
        },
        _ => panic!("unexpected operation {op}"),
    });
    let mut api = TrashStorageApi::new(bridge);

    let trash = api.list_trash_entries().unwrap();
    assert_eq!(trash.len(), 1);
    assert_eq!(trash[0].id, "custom:trash");

    let info = api.get_vault_storage_info();
    assert_eq!(info.entry_count, 1);
    assert_eq!(info.trash_count, 1);
}
