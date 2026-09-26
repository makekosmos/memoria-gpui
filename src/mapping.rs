//! Port of `src/lib/kepler-entry-mappers.ts` (+ the two task-type schema
//! normalizers from `kepler-task-sync.ts`) — ARK wire records ↔ Memoria
//! `Entry`/`NoteType`.

use serde_json::{Map, Value};

use crate::model::{
    ArkObjectLink, ArkObjectRecord, ArkObjectSummary, ArkObjectType, Entry, NoteType,
};
use crate::note_type_schemas::parse_note_type_definition;
use crate::note_types::{
    get_note_type_collection_name, normalize_note_type, normalize_slug, parse_note_type_ui_schema,
};
use crate::system_types::{
    is_system_type, normalize_system_note_type, should_show_as_eden_collection,
};
use crate::system_types_data::SYSTEM_TYPE_COLLECTION;
use crate::time::{millis_to_iso, timestamp_to_millis};

/// The V2 package contract permits Memoria to own notes, not global ARK schemas.
pub const DEFAULT_ARK_TYPE_ID: &str = "com.kosmos.note";
pub const MEMORIA_TYPE_ID_PROP: &str = "memoria_type_id";
pub const MEMORIA_RECORD_KIND_PROP: &str = "memoria_record_kind";
/// `EDEN_TASK_OBJECT_TYPE_ID` from `kepler-task-sync.ts`.
pub const EDEN_TASK_OBJECT_TYPE_ID: &str = "com.kosmos.task";
pub const SYSTEM_TYPE_COLLECTION_ID: &str = "collection_obj";
pub const SYSTEM_TYPE_JOURNAL_ID: &str = "system-type-journal";
pub const SYSTEM_TYPE_NOTE_ID: &str = "note_obj";

mod note_type_map;
pub use note_type_map::*;

/// `readMemoriaProps` — the `extensions` bag wins over legacy top-level keys.
pub fn read_memoria_props(props_json: &Value) -> Map<String, Value> {
    let Some(map) = props_json.as_object() else {
        return Map::new();
    };
    let mut out = map.clone();
    if let Some(Value::Object(extensions)) = map.get("extensions") {
        out.shift_remove("extensions");
        for (key, value) in extensions {
            out.insert(key.clone(), value.clone());
        }
    } else {
        out.shift_remove("extensions");
    }
    out
}

/// `getMemoriaRecordKind`.
pub fn memoria_record_kind(props_json: &Value) -> Option<Value> {
    read_memoria_props(props_json)
        .get(MEMORIA_RECORD_KIND_PROP)
        .cloned()
}

/// `arkTimestampToMillis`.
pub fn ark_timestamp_to_millis(value: &Value, fallback: i64) -> i64 {
    match value {
        Value::Null => fallback,
        other => timestamp_to_millis(other).unwrap_or(fallback),
    }
}

/// `millisToArkTimestamp`.
pub fn millis_to_ark_timestamp(value: Option<i64>) -> String {
    millis_to_iso(value.unwrap_or_else(crate::time::now_millis))
}

pub use crate::model::js_truthy;

/// `parseHeaderPropsJson`.
pub fn parse_header_props_json(raw: Option<&str>) -> Map<String, Value> {
    let Some(raw) = raw.filter(|s| !s.trim().is_empty()) else {
        return Map::new();
    };
    match serde_json::from_str::<Value>(raw) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    }
}

/// `normalizeEntry` — Vue fills absent optional fields with defaults.
pub fn normalize_entry(entry: &Entry) -> Entry {
    let mut out = entry.clone();
    if out.content_loaded.is_none() {
        out.content_loaded = Some(true);
    }
    if out.header_props_json.is_none() {
        out.header_props_json = Some("{}".into());
    }
    if out.schema_version.is_none() {
        out.schema_version = Some(1);
    }
    // `?? null` fields (type_id/header_layout/deleted_at) are already Option.
    out
}

fn related_targets(links: &[ArkObjectLink], object_id: &str) -> Vec<Value> {
    links
        .iter()
        .filter(|l| l.source_object_id == object_id && l.link_type == "related")
        .map(|l| Value::from(l.target_object_id.clone()))
        .collect()
}

fn stored_header_props(
    object_props: &Value,
    links: &[ArkObjectLink],
    id: &str,
) -> Map<String, Value> {
    let mut props = read_memoria_props(object_props);
    props.shift_remove(MEMORIA_TYPE_ID_PROP);
    props.shift_remove(MEMORIA_RECORD_KIND_PROP);
    props.insert(
        "related_notes".into(),
        Value::Array(related_targets(links, id)),
    );
    props
}

fn entry_header_layout(object_type: Option<&ArkObjectType>) -> String {
    object_type
        .and_then(|t| {
            parse_note_type_ui_schema(Some(&t.ui_schema_json))
                .ok()?
                .header_layout
        })
        .unwrap_or_else(|| "default".into())
}

fn stored_type_id(props: &Value, fallback: &str) -> String {
    read_memoria_props(props)
        .get(MEMORIA_TYPE_ID_PROP)
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .to_string()
}

/// `mapArkObjectToEntry` — `content_loaded: true`, `contentJson ?? ""`.
pub fn map_ark_object_to_entry(
    object: &ArkObjectRecord,
    links: &[ArkObjectLink],
    object_type: Option<&ArkObjectType>,
) -> Entry {
    let created_at = ark_timestamp_to_millis(&object.created_at, 0);
    let updated_at = ark_timestamp_to_millis(&object.updated_at, created_at);
    let content_json = if object.content_json.is_null() {
        crate::content::write_entry_markdown("")
    } else {
        object.content_json.clone()
    };
    normalize_entry(&Entry {
        id: object.id.clone(),
        title: object.title.clone(),
        content_json: serde_json::to_string(&content_json).unwrap_or_else(|_| "null".into()),
        content_loaded: Some(true),
        created_at,
        updated_at,
        folder_id: None,
        type_id: Some(stored_type_id(&object.props_json, &object.type_id)),
        header_layout: Some(entry_header_layout(object_type)),
        header_props_json: Some(
            serde_json::to_string(&stored_header_props(&object.props_json, links, &object.id))
                .unwrap_or_else(|_| "{}".into()),
        ),
        schema_version: Some(1),
        deleted_at: object
            .deleted_at
            .as_ref()
            .filter(|v| js_truthy(v))
            .map(|v| ark_timestamp_to_millis(v, updated_at)),
        extra: Map::new(),
    })
}

/// `mapArkObjectSummaryToEntry` — `content_loaded: false`, empty markdown.
pub fn map_ark_object_summary_to_entry(
    object: &ArkObjectSummary,
    links: &[ArkObjectLink],
    object_type: Option<&ArkObjectType>,
) -> Entry {
    let created_at = ark_timestamp_to_millis(&object.created_at, 0);
    let updated_at = ark_timestamp_to_millis(&object.updated_at, created_at);
    normalize_entry(&Entry {
        id: object.id.clone(),
        title: object.title.clone(),
        content_json: serde_json::to_string(&crate::content::write_entry_markdown(""))
            .unwrap_or_else(|_| "{}".into()),
        content_loaded: Some(false),
        created_at,
        updated_at,
        folder_id: None,
        type_id: Some(stored_type_id(&object.props_json, &object.type_id)),
        header_layout: Some(entry_header_layout(object_type)),
        header_props_json: Some(
            serde_json::to_string(&stored_header_props(&object.props_json, links, &object.id))
                .unwrap_or_else(|_| "{}".into()),
        ),
        schema_version: Some(1),
        deleted_at: object
            .deleted_at
            .as_ref()
            .filter(|v| js_truthy(v))
            .map(|v| ark_timestamp_to_millis(v, updated_at)),
        extra: Map::new(),
    })
}

/// `mapEntryToArkObject` — bare PM doc on the wire; header props move under
/// `propsJson.extensions`, `related_notes` become links (not props).
pub fn map_entry_to_ark_object(entry: &Entry) -> ArkObjectRecord {
    let header_props = parse_header_props_json(entry.header_props_json.as_deref());
    let mut extension_props = header_props.clone();
    extension_props.shift_remove("related_notes");
    let description = match extension_props.shift_remove("description") {
        Some(v @ Value::String(_)) | Some(v @ Value::Null) => v,
        Some(_) | None => Value::Null,
    };
    let content_json = crate::content::read_entry_tiptap_doc(
        &serde_json::from_str::<Value>(&entry.content_json).unwrap_or(Value::Null),
    );
    extension_props.insert(
        MEMORIA_TYPE_ID_PROP.into(),
        Value::from(
            entry
                .type_id
                .clone()
                .unwrap_or_else(|| DEFAULT_ARK_TYPE_ID.into()),
        ),
    );
    let mut extensions = Map::new();
    extensions.extend(extension_props);
    let mut props = Map::new();
    props.insert("description".into(), description);
    props.insert("extensions".into(), Value::Object(extensions));
    ArkObjectRecord {
        id: entry.id.clone(),
        type_id: DEFAULT_ARK_TYPE_ID.into(),
        type_version: Some("1.0.0".into()),
        title: entry.title.clone(),
        content_json,
        props_json: Value::Object(props),
        created_at: Value::from(millis_to_ark_timestamp(Some(entry.created_at))),
        updated_at: Value::from(millis_to_ark_timestamp(Some(entry.updated_at))),
        // Vue `entry.deleted_at ? millis : null` — `0` is falsy.
        deleted_at: entry
            .deleted_at
            .filter(|v| *v != 0)
            .map(|v| Value::from(millis_to_ark_timestamp(Some(v)))),
        extra: Map::new(),
    }
}

/// `shouldIncludeObjectInEdenList`.
pub fn should_include_object_in_eden_list(type_id: &str, props_json: &Value) -> bool {
    let props = read_memoria_props(props_json);
    if type_id == SYSTEM_TYPE_JOURNAL_ID
        && props.get("entry_kind").and_then(Value::as_str) == Some("bubble")
    {
        return false;
    }
    if type_id != SYSTEM_TYPE_COLLECTION_ID {
        return true;
    }
    match props.get("object_type_id").and_then(Value::as_str) {
        Some(object_type_id) => should_show_as_eden_collection(object_type_id),
        None => false,
    }
}

/// `isSystemType` re-export used by callers in this layer.
pub fn is_known_system_type(id: &str) -> bool {
    is_system_type(id)
}
