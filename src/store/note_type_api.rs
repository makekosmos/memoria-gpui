//! Port of `src/lib/kepler-note-type-api.ts` — note-type records are stored
//! as `com.kosmos.note` objects with `memoria_record_kind: "note_type"`.

use serde_json::{json, Map, Value};

use crate::mapping::{
    collection_object_id_for_type, js_truthy, map_ark_object_type_to_note_type,
    map_entry_to_ark_object, map_note_type_to_collection_entry, memoria_record_kind,
    millis_to_ark_timestamp, read_memoria_props, MEMORIA_RECORD_KIND_PROP,
};
use crate::model::{ArkObjectRecord, ArkObjectType, Entry, NoteType, SaveNoteTypeResult};
use crate::note_type_schemas::parse_header_template;
use crate::note_types::{
    normalize_note_type, normalize_slug, parse_definition, parse_note_type_ui_schema,
};
use crate::system_types::should_show_as_eden_collection;
use crate::system_types_data::system_types;

use super::transport::{ArkBridge, EngineError};

const NOTE_TYPE_RECORD_KIND: &str = "note_type";
/// `MEMORIA_NOTE_TYPE_PROP`.
pub const MEMORIA_NOTE_TYPE_PROP: &str = "memoria_note_type";

fn empty_note_content() -> Value {
    json!({ "type": "doc", "content": [] })
}

/// `noteTypeRecord` — the ARK object persisted for a note type.
fn note_type_record(note_type: &NoteType) -> Value {
    let updated = millis_to_ark_timestamp(Some(note_type.updated_at).filter(|v| *v != 0));
    let mut extensions = Map::new();
    extensions.insert(
        MEMORIA_RECORD_KIND_PROP.into(),
        Value::from(NOTE_TYPE_RECORD_KIND),
    );
    extensions.insert("note_type_id".into(), Value::from(note_type.id.clone()));
    extensions.insert(
        MEMORIA_NOTE_TYPE_PROP.into(),
        serde_json::to_value(note_type).unwrap_or(Value::Null),
    );
    let mut props = Map::new();
    props.insert("description".into(), Value::Null);
    props.insert("extensions".into(), Value::Object(extensions));
    json!({
        "id": format!("memoria:type:{}", note_type.id),
        "typeId": "com.kosmos.note",
        "typeVersion": "1.0.0",
        "title": note_type.name,
        "contentJson": empty_note_content(),
        "propsJson": Value::Object(props),
        "createdAt": millis_to_ark_timestamp(Some(note_type.created_at).filter(|v| *v != 0)),
        "updatedAt": updated,
        "deletedAt": null,
    })
}

fn is_note_type_record(value: &ArkObjectRecord) -> bool {
    value.type_id == "com.kosmos.note"
        && memoria_record_kind(&value.props_json) == Some(Value::from(NOTE_TYPE_RECORD_KIND))
}

/// `recordToNoteType` — nested `memoria_note_type` wins, `contentJson` is the
/// legacy fallback; both go through `normalizeNoteType`.
fn record_to_note_type(value: &ArkObjectRecord) -> Option<NoteType> {
    if !is_note_type_record(value)
        || (!js_truthy(&value.content_json) && !js_truthy(&value.props_json))
    {
        return None;
    }
    let nested = read_memoria_props(&value.props_json)
        .get(MEMORIA_NOTE_TYPE_PROP)
        .cloned();
    for candidate in [nested, Some(value.content_json.clone())]
        .into_iter()
        .flatten()
    {
        if !candidate.is_object() {
            continue;
        }
        // `normalizeNoteType` on the raw payload (zod strips unknown keys).
        if let Ok(nt) = crate::note_types::normalize_note_type_value(&candidate) {
            return Some(nt);
        }
    }
    None
}

/// `createNoteTypeApi`.
pub struct NoteTypeApi<B: ArkBridge> {
    bridge: B,
}

impl<B: ArkBridge> NoteTypeApi<B> {
    pub fn new(bridge: B) -> Self {
        Self { bridge }
    }

    /// `getNoteTypeById` — listed first, then the legacy `get_object_type`.
    pub fn get_note_type_by_id(&self, note_type_id: &str) -> Result<Option<NoteType>, EngineError> {
        let types = self.list_note_types()?;
        if let Some(listed) = types.iter().find(|t| t.id == note_type_id) {
            return Ok(Some(listed.clone()));
        }
        match self.bridge.get_object_type(note_type_id) {
            Ok(raw) if !raw.is_null() => match serde_json::from_value::<ArkObjectType>(raw) {
                Ok(record) => Ok(map_ark_object_type_to_note_type(&record).ok()),
                Err(_) => Ok(None),
            },
            _ => Ok(None),
        }
    }

    /// `listNoteTypes` — custom records + legacy types, then SYSTEM_TYPES;
    /// `task_obj` first among discovered.
    pub fn list_note_types(&self) -> Result<Vec<NoteType>, EngineError> {
        let records = self
            .bridge
            .list_objects_by_type("com.kosmos.note")
            .unwrap_or_default();
        let object_types = self.bridge.list_object_types().unwrap_or_default();

        let mut custom: Vec<NoteType> = records
            .iter()
            .filter_map(|v| serde_json::from_value::<ArkObjectRecord>(v.clone()).ok())
            .filter(|r| !r.deleted_at.as_ref().map(js_truthy).unwrap_or(false))
            .filter_map(|r| record_to_note_type(&r))
            .collect();
        let legacy: Vec<NoteType> = object_types
            .iter()
            .filter_map(|v| serde_json::from_value::<ArkObjectType>(v.clone()).ok())
            .filter(|t| !t.system_locked)
            .filter_map(|t| map_ark_object_type_to_note_type(&t).ok())
            .collect();
        custom.extend(legacy);

        let system_ids: Vec<String> = system_types().iter().map(|t| t.id.clone()).collect();
        let mut discovered: Vec<NoteType> = Vec::new();
        for nt in custom {
            if system_ids.iter().any(|id| id == &nt.id) || discovered.iter().any(|d| d.id == nt.id)
            {
                continue;
            }
            discovered.push(nt);
        }
        let mut ordered: Vec<NoteType> = Vec::new();
        if let Some(pos) = discovered.iter().position(|t| t.id == "task_obj") {
            ordered.push(discovered.remove(pos));
        }
        ordered.extend(discovered);
        ordered.extend(system_types());
        Ok(ordered)
    }

    /// `ensureCollectionObjects`.
    pub fn ensure_collection_objects(
        &self,
        note_types: &[NoteType],
    ) -> Result<Vec<Entry>, EngineError> {
        let mut entries = Vec::new();
        for note_type in note_types {
            if !should_show_as_eden_collection(&note_type.id) {
                continue;
            }
            let existing = self
                .bridge
                .get_object(&collection_object_id_for_type(&note_type.id))
                .ok()
                .filter(|v| !v.is_null())
                .and_then(|v| serde_json::from_value::<ArkObjectRecord>(v).ok());
            let mut entry = map_note_type_to_collection_entry(note_type, existing.as_ref());
            entry.deleted_at = None;
            let ark_object = map_entry_to_ark_object(&entry);
            self.bridge.upsert_object(json!({
                "id": ark_object.id,
                "typeId": ark_object.type_id,
                "typeVersion": ark_object.type_version,
                "title": ark_object.title,
                "contentJson": ark_object.content_json,
                "propsJson": ark_object.props_json,
                "createdAt": ark_object.created_at,
                "updatedAt": ark_object.updated_at,
                "deletedAt": null,
            }))?;
            entries.push(entry);
        }
        Ok(entries)
    }

    /// `saveNoteType` — schema/header/ui-schema validation first.
    pub fn save_note_type(&self, note_type: &NoteType) -> Result<SaveNoteTypeResult, EngineError> {
        if parse_definition(&note_type.schema_json).is_err()
            || parse_header_template(&note_type.header_template_json).is_err()
            || parse_note_type_ui_schema(note_type.ui_schema_json.as_deref()).is_err()
        {
            return Ok(SaveNoteTypeResult::failed(
                "invalid_definition",
                "Схема типа заметки заполнена некорректно",
            ));
        }
        let mut with_slug = note_type.clone();
        let slug_source = if with_slug.slug.is_empty() {
            &with_slug.name
        } else {
            &with_slug.slug
        };
        with_slug.slug = normalize_slug(slug_source);
        let normalized = match normalize_note_type(&with_slug) {
            Ok(nt) => nt,
            Err(_) => {
                return Ok(SaveNoteTypeResult::failed(
                    "invalid_definition",
                    "Схема типа заметки заполнена некорректно",
                ))
            }
        };
        self.bridge.upsert_object(note_type_record(&normalized))?;
        Ok(SaveNoteTypeResult::ok(normalized))
    }

    /// `deleteNoteType` — deletes the `memoria:type:{id}` record.
    pub fn delete_note_type(&self, note_type_id: &str) -> Result<bool, EngineError> {
        let record = self
            .bridge
            .get_object(&format!("memoria:type:{note_type_id}"))?;
        if record.is_null() {
            return Ok(false);
        }
        let record: ArkObjectRecord =
            serde_json::from_value(record).map_err(|_| EngineError::Malformed)?;
        self.bridge.delete_object(&record.id)?;
        Ok(true)
    }
}
