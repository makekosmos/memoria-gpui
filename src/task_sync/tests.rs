use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::{json, Map, Value};

use super::*;

/// Fake `ArkBridge` — records stored by id; the minimum RPC surface task sync
/// touches (`get_object`, `upsert_object`, `delete_object`, `upsert_object_type`).
#[derive(Clone, Default)]
struct FakeBridge {
    objects: Arc<Mutex<BTreeMap<String, Value>>>,
    object_types: Arc<Mutex<BTreeMap<String, Value>>>,
}

impl ArkBridge for FakeBridge {
    fn rpc(&self, operation: &str, params: Value) -> Result<Value, EngineError> {
        match operation {
            "get_object" => {
                let id = params.get("id").and_then(Value::as_str).unwrap_or("");
                Ok(self
                    .objects
                    .lock()
                    .unwrap()
                    .get(id)
                    .cloned()
                    .unwrap_or(Value::Null))
            }
            "upsert_object" => {
                let object = params.get("object").cloned().unwrap_or(params.clone());
                let id = object
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                self.objects.lock().unwrap().insert(id, object);
                Ok(json!({ "id": params.get("id").cloned().unwrap_or(Value::Null) }))
            }
            "delete_object" => {
                let id = params.get("id").and_then(Value::as_str).unwrap_or("");
                Ok(json!(self.objects.lock().unwrap().remove(id).is_some()))
            }
            "upsert_object_type" => {
                let object_type = params.get("object_type").cloned().unwrap_or(params.clone());
                let id = object_type
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                self.object_types.lock().unwrap().insert(id, object_type);
                Ok(Value::Null)
            }
            _ => Err(EngineError::Rpc(format!("unexpected op {operation}"))),
        }
    }
}

impl FakeBridge {
    fn stored(&self, id: &str) -> Value {
        self.objects
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .unwrap_or(Value::Null)
    }

    fn stored_props(&self, id: &str) -> Map<String, Value> {
        self.stored(id)
            .get("propsJson")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default()
    }
}

#[test]
fn registers_the_task_object_type() {
    let bridge = FakeBridge::default();
    ensure_task_object_type_registered(&bridge).unwrap();
    let record = bridge.object_types.lock().unwrap()[EDEN_TASK_OBJECT_TYPE_ID].clone();
    assert_eq!(record["id"], json!(EDEN_TASK_OBJECT_TYPE_ID));
    assert_eq!(record["schemaJson"], json!(TASK_OBJECT_TYPE_SCHEMA_JSON));
    assert_eq!(
        record["uiSchemaJson"],
        json!(TASK_OBJECT_TYPE_UI_SCHEMA_JSON)
    );
}

#[test]
fn schema_normalization_falls_back_on_invalid_json() {
    assert_eq!(
        normalize_task_object_type_schema_json("not json"),
        TASK_OBJECT_TYPE_SCHEMA_JSON
    );
    assert_eq!(
        normalize_task_object_type_ui_schema_json("{}"),
        TASK_OBJECT_TYPE_UI_SCHEMA_JSON
    );
    assert_eq!(
        normalize_task_object_type_schema_json(TASK_OBJECT_TYPE_SCHEMA_JSON),
        TASK_OBJECT_TYPE_SCHEMA_JSON
    );
}

#[test]
fn create_and_get_roundtrip() {
    let bridge = FakeBridge::default();
    let id = create_task(&bridge, "note:1", "Buy milk", None).unwrap();
    let task = get_task(&bridge, &id).unwrap().unwrap();
    assert_eq!(task.type_id, EDEN_TASK_OBJECT_TYPE_ID);
    assert_eq!(task.title, "Buy milk");
    let props = task.props_json.as_object().unwrap();
    assert_eq!(props["status"], json!("triage"));
    assert_eq!(props["source_note_id"], json!("note:1"));
    assert_eq!(props["is_completed"], json!(false));
    // Empty title falls back to the shared placeholder.
    let id2 = create_task(&bridge, "note:1", "   ", None).unwrap();
    assert_eq!(
        get_task(&bridge, &id2).unwrap().unwrap().title,
        EMPTY_TASK_TITLE
    );
}

#[test]
fn patch_task_derives_flags_and_preserves_props() {
    let bridge = FakeBridge::default();
    let id = create_task(&bridge, "note:1", "Task", None).unwrap();
    bridge.objects.lock().unwrap().get_mut(&id).unwrap()["propsJson"]
        .as_object_mut()
        .unwrap()
        .insert("custom".into(), json!("keep"));

    patch_task(
        &bridge,
        &id,
        &TaskPatch {
            status: Some("done"),
            ..Default::default()
        },
    )
    .unwrap();
    let props = bridge.stored_props(&id);
    assert_eq!(props["status"], json!("done"));
    assert_eq!(props["is_completed"], json!(true));
    assert_eq!(props["is_cancelled"], json!(false));
    assert_eq!(props["custom"], json!("keep"));
    assert!(props["completed_at"].is_string());

    patch_task(
        &bridge,
        &id,
        &TaskPatch {
            status: Some("canceled"),
            ..Default::default()
        },
    )
    .unwrap();
    let props = bridge.stored_props(&id);
    assert_eq!(props["is_completed"], json!(false));
    assert_eq!(props["is_cancelled"], json!(true));
    assert_eq!(props["completed_at"], json!(Value::Null));
    assert!(props["cancelled_at"].is_string());

    // is_completed-only patch maps to done (Vue `patchTask({isCompleted})`).
    patch_task(
        &bridge,
        &id,
        &TaskPatch {
            is_completed: Some(true),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(bridge.stored_props(&id)["status"], json!("done"));
}

#[test]
fn soft_delete_tombstones_once() {
    let bridge = FakeBridge::default();
    let id = create_task(&bridge, "note:1", "Task", None).unwrap();
    soft_delete_task(&bridge, &id).unwrap();
    assert!(get_task(&bridge, &id).unwrap().is_none());
    let deleted = bridge.stored(&id)["deletedAt"].clone();
    assert!(deleted.is_string());
    // Second delete is a no-op (deletedAt unchanged).
    soft_delete_task(&bridge, &id).unwrap();
    assert_eq!(bridge.stored(&id)["deletedAt"], deleted);
    // Missing id is a no-op, not an error.
    soft_delete_task(&bridge, "task:missing").unwrap();
}

#[test]
fn classify_object_change_maps_engine_events() {
    assert_eq!(
        classify_object_change(
            &json!({"event": "object_upserted", "id": "t1", "type_id": EDEN_TASK_OBJECT_TYPE_ID})
        ),
        Some(TaskObjectEvent::Upserted {
            id: "t1".into(),
            type_id: Some(EDEN_TASK_OBJECT_TYPE_ID.into()),
        })
    );
    assert_eq!(
        classify_object_change(&json!({"event": "object_deleted", "id": "t1"})),
        Some(TaskObjectEvent::Deleted { id: "t1".into() })
    );
    assert_eq!(
        classify_object_change(&json!({"event": "entity_changed", "id": "t1"})),
        None
    );
    assert_eq!(
        classify_object_change(&json!({"event": "object_upserted"})),
        None
    );
}
