//! Review-fix regressions — wired-path divergences found by the M1 parity
//! review, exercised through the same `ArkBridge` fake as `ark_api.rs`.

use memoria_gpui::store::transport::{ArkBridge, EngineError};
use memoria_gpui::store::{EntryApi, NoteTypeApi, TrashStorageApi};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

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

fn entry(id: &str) -> memoria_gpui::model::Entry {
    memoria_gpui::model::Entry {
        id: id.into(),
        title: id.into(),
        content_json: serde_json::to_string(&json!({
            "type": "markdown", "version": 1, "text": "текст",
        }))
        .unwrap(),
        content_loaded: Some(true),
        updated_at: 1_800_000_000_000,
        ..Default::default()
    }
}

#[test]
fn delete_entry_proceeds_when_deleted_at_is_falsy() {
    // `existing.deletedAt` is a JS-truthiness check: 0/""/false mean "not
    // deleted", so `delete_object` still runs for malformed Engine records.
    let calls = Arc::new(Mutex::new(Vec::<String>::new()));
    let calls_c = calls.clone();
    let mut deleted = record("note-a", "com.kosmos.note");
    deleted["deletedAt"] = json!(0);
    let bridge = FakeArk::new(move |op, _| {
        calls_c.lock().unwrap().push(op.to_string());
        match op {
            "get_object" => Ok(deleted.clone()),
            "delete_object" => Ok(Value::from(true)),
            _ => panic!("unexpected operation {op}"),
        }
    });
    let mut api = EntryApi::new(bridge);

    let result = api.delete_entry("note-a").unwrap();
    assert!(result.ok);
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        ["get_object", "delete_object"]
    );
}

#[test]
fn save_entry_accepts_empty_string_header_props_json() {
    // `JSON.parse(entry.header_props_json || "{}")` — a falsy "" falls back
    // to "{}" instead of erroring with invalid_type_metadata.
    let bridge = FakeArk::new(|op, _| match op {
        // `list_all_objects` (type discovery + per-type listing) comes back
        // empty, so the stale/duplicate gates pass and the save lands.
        "list_objects_by_type" | "list_object_types" => Ok(json!([])),
        "list_object_links" => Ok(json!([])),
        "upsert_object" => Ok(Value::from(true)),
        _ => panic!("unexpected operation {op}"),
    });
    let mut api = EntryApi::new(bridge);
    let mut e = entry("note-a");
    e.type_id = Some("person_obj".into());
    e.header_props_json = Some("".into());

    let result = api.save_entry(&e).unwrap();
    assert!(
        result.reason.as_deref() != Some("invalid_type_metadata"),
        "empty header_props_json must not hit the parse-error path: {result:?}"
    );
}

#[test]
fn save_entry_propagates_malformed_type_schema() {
    // Vue's `validateHeaderProps` → `buildHeaderPropsSchema` →
    // `parseNoteTypeDefinition` throws out of `saveEntry` on a malformed
    // type schema — the port surfaces that as a transport-level error, not
    // a silent save.
    let bridge = FakeArk::new(|op, _| match op {
        "list_objects_by_type" => Ok(json!([])),
        "list_object_types" => Ok(json!([{
            "id": "weird_obj",
            "name": "Странный",
            "schemaJson": "{not json",
            "uiSchemaJson": "{}",
            "systemLocked": false,
        }])),
        _ => panic!("unexpected operation {op}"),
    });
    let mut api = EntryApi::new(bridge);
    let mut e = entry("note-a");
    e.type_id = Some("weird_obj".into());
    e.header_props_json = Some("{}".into());

    assert!(matches!(api.save_entry(&e), Err(EngineError::Malformed)));
}

#[test]
fn malformed_legacy_types_are_dropped_not_fabricated() {
    // `mapArkObjectTypeToNoteType` throws on bad `uiSchemaJson`; Vue catches
    // per-type (list) or returns null (getById) — no phantom NoteType.
    let bridge = FakeArk::new(|op, _| match op {
        "list_objects_by_type" => Ok(json!([])),
        "list_object_types" => Ok(json!([
            { "id": "good_obj", "name": "Хороший", "schemaJson": "{}", "uiSchemaJson": "{}", "systemLocked": false },
            { "id": "bad_obj", "name": "Битый", "schemaJson": "{}", "uiSchemaJson": "{oops", "systemLocked": false },
        ])),
        "get_object_type" => Ok(json!({
            "id": "bad_obj", "name": "Битый", "schemaJson": "{}", "uiSchemaJson": "{oops",
        })),
        _ => panic!("unexpected operation {op}"),
    });
    let api = NoteTypeApi::new(bridge);

    let types = api.list_note_types().unwrap();
    assert!(types.iter().any(|t| t.id == "good_obj"));
    assert!(!types.iter().any(|t| t.id == "bad_obj"));

    let api = NoteTypeApi::new(FakeArk::new(|op, _| match op {
        "list_objects_by_type" | "list_object_types" => Ok(json!([])),
        "get_object_type" => Ok(json!({
            "id": "bad_obj", "name": "Битый", "schemaJson": "{}", "uiSchemaJson": "{oops",
        })),
        _ => panic!("unexpected operation {op}"),
    }));
    assert!(api.get_note_type_by_id("bad_obj").unwrap().is_none());
}

#[test]
fn list_all_entries_propagates_link_failures() {
    // Vue `Promise.all([listAllObjects(), listObjectLinks()])` — a links
    // failure rejects the whole list instead of silently losing relations.
    let bridge = FakeArk::new(|op, _| match op {
        "list_object_types" => Ok(json!([])),
        "list_objects_by_type" => Ok(json!([record("note-a", "com.kosmos.note")])),
        "list_object_links" => Err(EngineError::Unreachable),
        _ => panic!("unexpected operation {op}"),
    });
    let mut api = EntryApi::new(bridge);
    assert!(matches!(
        api.list_all_entries(),
        Err(EngineError::Unreachable)
    ));
}

#[test]
fn dedupe_keeps_the_last_duplicate_like_js_map() {
    // `new Map(list.map(o => [o.id, o]))` — first position wins, last value
    // wins. A record duplicated across compat type lists keeps the later
    // projection, not the earlier one.
    let bridge = FakeArk::new(|op, params| match op {
        "list_object_types" => Ok(json!([
            { "id": "a_obj", "name": "A", "schemaJson": "{}", "uiSchemaJson": "{}", "systemLocked": false },
            { "id": "b_obj", "name": "B", "schemaJson": "{}", "uiSchemaJson": "{}", "systemLocked": false },
        ])),
        "list_objects_by_type" if params["type_id"] == json!("a_obj") => {
            let mut first = record("dup", "a_obj");
            first["title"] = json!("summary-projection");
            Ok(json!([first]))
        }
        "list_objects_by_type" if params["type_id"] == json!("b_obj") => {
            let mut second = record("dup", "b_obj");
            second["title"] = json!("full-projection");
            second["deletedAt"] = json!("2026-02-01T00:00:00.000Z");
            Ok(json!([second]))
        }
        "list_objects_by_type" => Ok(json!([])),
        "list_object_links" => Ok(json!([])),
        _ => panic!("unexpected operation {op}"),
    });
    let mut api = TrashStorageApi::new(bridge);
    let trashed = api.list_trash_entries().unwrap();
    assert_eq!(trashed.len(), 1);
    assert_eq!(trashed[0].title, "full-projection");
}
