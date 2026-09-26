//! Agenda-compatible task sync — port of `src/lib/kepler-task-sync.ts`.
//!
//! Tasks are `com.kosmos.task` ARK objects; `propsJson` carries the shared
//! Agenda/Delphi field set (`status`, `is_completed`, `is_cancelled`,
//! `deadline`, …). Every call goes through `ArkBridge` — no direct OS or
//! network access (KOS-157).

use serde_json::{json, Value};

use crate::content::write_entry_markdown;
use crate::model::{js_truthy, ArkObjectRecord};
use crate::note_type_schemas::{parse_note_type_definition, parse_note_type_ui_schema_value};
use crate::store::transport::{ArkBridge, EngineError};
use crate::task_status::{
    derive_cancelled_flag, derive_completed_flag, normalize_status, TaskStatus,
};
use crate::time::{millis_to_iso, now_millis};

const EMPTY_TASK_TITLE: &str = "Пустая задача";

/// `EDEN_TASK_OBJECT_TYPE_ID`.
pub const EDEN_TASK_OBJECT_TYPE_ID: &str = "com.kosmos.task";

/// `TASK_OBJECT_TYPE_SCHEMA_JSON` — literal layout matches `JSON.stringify`.
pub const TASK_OBJECT_TYPE_SCHEMA_JSON: &str =
    "{\"fields\":[{\"id\":\"deadline\",\"label\":\"Дедлайн\",\"kind\":\"date\",\"required\":false,\"visible\":true,\"read_only\":false}]}";

/// `TASK_OBJECT_TYPE_UI_SCHEMA_JSON`.
pub const TASK_OBJECT_TYPE_UI_SCHEMA_JSON: &str =
    "{\"featured_fields\":[\"deadline\"],\"visible_fields\":[\"deadline\"],\"hidden_fields\":[\"created_at\",\"updated_at\",\"deleted_at\"],\"read_only_fields\":[],\"field_order\":[\"deadline\"],\"header_layout\":\"inline\",\"default_layout\":\"page\",\"collection_name\":\"Задачи\"}";

/// `normalizeTaskObjectTypeSchemaJson`.
pub fn normalize_task_object_type_schema_json(schema_json: &str) -> String {
    match parse_note_type_definition(schema_json) {
        Ok(_) => schema_json.to_string(),
        Err(_) => TASK_OBJECT_TYPE_SCHEMA_JSON.to_string(),
    }
}

/// `normalizeTaskObjectTypeUiSchemaJson`.
pub fn normalize_task_object_type_ui_schema_json(ui_schema_json: &str) -> String {
    let parsed: Result<Value, _> = serde_json::from_str(ui_schema_json);
    if let Ok(parsed) = parsed {
        if let Ok(ui_schema) = parse_note_type_ui_schema_value(&parsed) {
            let has_deadline = |list: &Option<Vec<String>>| {
                list.as_ref()
                    .is_some_and(|v| v.iter().any(|f| f == "deadline"))
            };
            if has_deadline(&ui_schema.featured_fields) && has_deadline(&ui_schema.visible_fields) {
                return ui_schema_json.to_string();
            }
        }
    }
    TASK_OBJECT_TYPE_UI_SCHEMA_JSON.to_string()
}

/// `ensureTaskObjectTypeRegistered` — idempotent `upsert_object_type`. The Vue
/// singleton-promise dedupe is a UI concern; callers here can simply retry.
pub fn ensure_task_object_type_registered<B: ArkBridge>(bridge: &B) -> Result<(), EngineError> {
    let now = millis_to_iso(now_millis());
    bridge.upsert_object_type(json!({
        "id": EDEN_TASK_OBJECT_TYPE_ID,
        "name": "Задача",
        "schemaJson": TASK_OBJECT_TYPE_SCHEMA_JSON,
        "uiSchemaJson": TASK_OBJECT_TYPE_UI_SCHEMA_JSON,
        "systemLocked": false,
        "createdAt": now,
        "updatedAt": now,
    }))?;
    Ok(())
}

fn is_deleted(record: &ArkObjectRecord) -> bool {
    record.deleted_at.as_ref().is_some_and(js_truthy)
}

/// `getTask` — `null` for missing, wrong-type, or soft-deleted objects.
pub fn get_task<B: ArkBridge>(
    bridge: &B,
    task_id: &str,
) -> Result<Option<ArkObjectRecord>, EngineError> {
    let value = bridge.get_object(task_id)?;
    if value.is_null() {
        return Ok(None);
    }
    let record: ArkObjectRecord =
        serde_json::from_value(value).map_err(|_| EngineError::Malformed)?;
    if record.type_id != EDEN_TASK_OBJECT_TYPE_ID || is_deleted(&record) {
        return Ok(None);
    }
    Ok(Some(record))
}

/// `patchTask` argument shape.
#[derive(Debug, Clone, Default)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub is_completed: Option<bool>,
    pub status: Option<TaskStatus>,
}

/// `patchTask` — merges status + derived Delphi flags into `propsJson`,
/// preserving every unknown field of the existing object.
pub fn patch_task<B: ArkBridge>(
    bridge: &B,
    task_id: &str,
    patch: &TaskPatch,
) -> Result<(), EngineError> {
    let existing = bridge.get_object(task_id)?;
    if existing.is_null() {
        return Ok(());
    }
    let mut object = existing;
    let now = millis_to_iso(now_millis());
    let props = object
        .get("propsJson")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let next_status = patch
        .status
        .or_else(|| {
            patch
                .is_completed
                .map(|done| if done { "done" } else { "todo" })
        })
        .unwrap_or_else(|| {
            normalize_status(
                props.get("status"),
                js_truthy(props.get("is_completed").unwrap_or(&Value::Null)),
                js_truthy(props.get("is_cancelled").unwrap_or(&Value::Null)),
            )
        });

    let mut next_props = props.clone();
    next_props.insert("status".into(), json!(next_status));
    next_props.insert(
        "is_completed".into(),
        json!(derive_completed_flag(next_status)),
    );
    next_props.insert(
        "is_cancelled".into(),
        json!(derive_cancelled_flag(next_status)),
    );
    if derive_completed_flag(next_status) && props.get("completed_at").is_none_or(Value::is_null) {
        next_props.insert("completed_at".into(), json!(now));
    } else if !derive_completed_flag(next_status) {
        next_props.insert("completed_at".into(), Value::Null);
    }
    if derive_cancelled_flag(next_status) && props.get("cancelled_at").is_none_or(Value::is_null) {
        next_props.insert("cancelled_at".into(), json!(now));
    } else if !derive_cancelled_flag(next_status) {
        next_props.insert("cancelled_at".into(), Value::Null);
    }

    if let Some(title) = &patch.title {
        let trimmed = title.trim();
        object["title"] = json!(if trimmed.is_empty() {
            EMPTY_TASK_TITLE
        } else {
            trimmed
        });
    }
    object["propsJson"] = Value::Object(next_props);
    object["updatedAt"] = json!(now);
    bridge.upsert_object(object)?;
    Ok(())
}

/// `createTask` — returns the task id. `explicit_id` keeps caller-owned ids.
pub fn create_task<B: ArkBridge>(
    bridge: &B,
    source_note_id: &str,
    title: &str,
    explicit_id: Option<&str>,
) -> Result<String, EngineError> {
    let task_id = explicit_id
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let now = millis_to_iso(now_millis());
    let trimmed = title.trim();
    let effective_title = if trimmed.is_empty() {
        EMPTY_TASK_TITLE
    } else {
        trimmed
    };
    bridge.upsert_object(json!({
        "id": task_id,
        "typeId": EDEN_TASK_OBJECT_TYPE_ID,
        "title": effective_title,
        "contentJson": write_entry_markdown(""),
        "propsJson": {
            "description": null,
            "priority": 0,
            "scheduled_date": null,
            "deadline": null,
            "reminder_date": null,
            "is_today": false,
            "is_evening": false,
            "is_someday": false,
            "is_completed": false,
            "completed_at": null,
            "is_cancelled": false,
            "cancelled_at": null,
            "is_trashed": false,
            "status": "triage",
            "sort_order": 0,
            "heading_id": null,
            "project_id": null,
            "area_id": null,
            "tag_ids": [],
            "checklist_items": [],
            "recurrence_rule": null,
            "billable": false,
            "price": null,
            "created_at": now,
            "source_app": "eden",
            "source_note_id": source_note_id,
            "model_version": 1,
        },
        "createdAt": now,
        "updatedAt": now,
        "deletedAt": null,
    }))?;
    Ok(task_id)
}

/// `subscribeObjectChanges` payload shape — the Engine WS emits
/// `{"event": "object_upserted"|"object_deleted", "id": …, "type_id"?: …}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskObjectEvent {
    Upserted { id: String, type_id: Option<String> },
    Deleted { id: String },
}

/// Classify one `EngineEvent::Changed` payload; `None` for unrelated events or
/// malformed ids. Wire this into the existing `Engine::subscribe_events` loop.
pub fn classify_object_change(payload: &Value) -> Option<TaskObjectEvent> {
    let event = payload.get("event").and_then(Value::as_str)?;
    let id = payload.get("id").and_then(Value::as_str)?.to_string();
    match event {
        "object_upserted" => Some(TaskObjectEvent::Upserted {
            id,
            type_id: payload
                .get("type_id")
                .or_else(|| payload.get("typeId"))
                .and_then(Value::as_str)
                .map(str::to_string),
        }),
        "object_deleted" => Some(TaskObjectEvent::Deleted { id }),
        _ => None,
    }
}

/// `softDeleteTask` — tombstones via `deletedAt`; a no-op when already gone.
pub fn soft_delete_task<B: ArkBridge>(bridge: &B, task_id: &str) -> Result<(), EngineError> {
    let existing = bridge.get_object(task_id)?;
    if existing.is_null() || js_truthy(existing.get("deletedAt").unwrap_or(&Value::Null)) {
        return Ok(());
    }
    let mut object = existing;
    let deleted_at = millis_to_iso(now_millis());
    object["updatedAt"] = json!(deleted_at);
    object["deletedAt"] = json!(deleted_at);
    bridge.upsert_object(object)?;
    Ok(())
}
#[cfg(test)]
mod tests;
