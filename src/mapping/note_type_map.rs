//! `mapArkObjectTypeToNoteType` + `mapNoteTypeToCollectionEntry` +
//! `collectionObjectIdForType` (+ task schema repair from
//! `kepler-task-sync.ts`).

use super::*;

const TASK_SCHEMA_JSON: &str = r#"{"fields":[{"id":"deadline","label":"Дедлайн","kind":"date","required":false,"visible":true,"read_only":false}]}"#;
const TASK_UI_SCHEMA_JSON: &str = r#"{"featured_fields":["deadline"],"visible_fields":["deadline"],"hidden_fields":["created_at","updated_at","deleted_at"],"read_only_fields":[],"field_order":["deadline"],"header_layout":"inline","default_layout":"page","collection_name":"Задачи"}"#;
pub fn normalize_task_object_type_schema_json(schema_json: &str) -> String {
    match parse_note_type_definition(schema_json) {
        Ok(_) => schema_json.to_string(),
        Err(_) => TASK_SCHEMA_JSON.to_string(),
    }
}

/// `normalizeTaskObjectTypeUiSchemaJson` — deadline must stay featured+visible.
pub fn normalize_task_object_type_ui_schema_json(ui_schema_json: &str) -> String {
    let ok = parse_note_type_ui_schema(Some(ui_schema_json))
        .map(|ui| {
            ui.featured_fields
                .as_deref()
                .unwrap_or_default()
                .iter()
                .any(|f| f == "deadline")
                && ui
                    .visible_fields
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .any(|f| f == "deadline")
        })
        .unwrap_or(false);
    if ok {
        ui_schema_json.to_string()
    } else {
        TASK_UI_SCHEMA_JSON.to_string()
    }
}

fn icon_for_object_type_id(object_type_id: &str) -> String {
    match object_type_id {
        "game_obj" => "game-controller".into(),
        "task_obj" => "checkmark-circle".into(),
        _ => "document-text".into(),
    }
}

fn color_for_object_type_id(object_type_id: &str) -> String {
    match object_type_id {
        "game_obj" => "#ef4444".into(),
        "task_obj" => "#f59e0b".into(),
        _ => "#2aa7ee".into(),
    }
}

/// `mapArkObjectTypeToNoteType` — task types get schema/UI-schema repair.
/// `Err` where Vue throws (`parseNoteTypeUiSchema`/`normalizeNoteType`):
/// callers drop or `null` the type just like `kepler-note-type-api.ts`.
pub fn map_ark_object_type_to_note_type(object_type: &ArkObjectType) -> Result<NoteType, String> {
    let is_task = object_type.id == EDEN_TASK_OBJECT_TYPE_ID || object_type.id == "task_obj";
    let schema_json = if is_task {
        normalize_task_object_type_schema_json(&object_type.schema_json)
    } else {
        object_type.schema_json.clone()
    };
    let ui_schema_json = if is_task {
        normalize_task_object_type_ui_schema_json(&object_type.ui_schema_json)
    } else {
        object_type.ui_schema_json.clone()
    };
    let created_at = ark_timestamp_to_millis(&object_type.created_at, 0);
    let updated_at = ark_timestamp_to_millis(&object_type.updated_at, created_at);
    let ui = parse_note_type_ui_schema(Some(&ui_schema_json))?;
    let mut header_template = Map::new();
    header_template.insert(
        "kind".into(),
        Value::from(if ui.header_layout.as_deref() == Some("column") {
            "centered_profile"
        } else {
            "default"
        }),
    );
    header_template.insert(
        "primaryFieldIds".into(),
        Value::Array(
            ui.featured_fields
                .unwrap_or_default()
                .into_iter()
                .map(Value::from)
                .collect(),
        ),
    );
    header_template.insert(
        "secondaryFieldIds".into(),
        Value::Array(
            ui.visible_fields
                .unwrap_or_default()
                .into_iter()
                .map(Value::from)
                .collect(),
        ),
    );
    header_template.insert("imageFieldId".into(), Value::Null);
    Ok(normalize_system_note_type(&normalize_note_type(
        &NoteType {
            id: object_type.id.clone(),
            name: object_type.name.clone(),
            slug: normalize_slug(&object_type.id),
            icon: Some(icon_for_object_type_id(&object_type.id)),
            color: Some(color_for_object_type_id(&object_type.id)),
            schema_json,
            header_template_json: serde_json::to_string(&Value::Object(header_template))
                .unwrap_or_else(|_| "{}".into()),
            ui_schema_json: Some(ui_schema_json),
            created_at,
            updated_at,
            extra: Map::new(),
        },
    )?))
}

/// `collectionObjectIdForType`.
pub fn collection_object_id_for_type(note_type_id: &str) -> String {
    format!("collection:{note_type_id}")
}

/// `mapNoteTypeToCollectionEntry`.
pub fn map_note_type_to_collection_entry(
    note_type: &NoteType,
    existing: Option<&ArkObjectRecord>,
) -> Entry {
    let now = crate::time::now_millis();
    let created_at = existing
        .map(|e| ark_timestamp_to_millis(&e.created_at, now))
        .unwrap_or(now);
    let updated_at = existing
        .map(|e| ark_timestamp_to_millis(&e.updated_at, created_at))
        .unwrap_or(now);
    let mut props = existing
        .map(|e| read_memoria_props(&e.props_json))
        .unwrap_or_default();
    props.insert("object_type_id".into(), Value::from(note_type.id.clone()));
    normalize_entry(&Entry {
        id: collection_object_id_for_type(&note_type.id),
        title: get_note_type_collection_name(Some(note_type)),
        content_json: serde_json::to_string(&crate::content::write_entry_markdown(""))
            .unwrap_or_else(|_| "{}".into()),
        created_at,
        updated_at,
        folder_id: None,
        type_id: Some(SYSTEM_TYPE_COLLECTION_ID.into()),
        header_layout: Some(
            parse_note_type_ui_schema(SYSTEM_TYPE_COLLECTION.ui_schema_json.as_deref())
                .ok()
                .and_then(|u| u.header_layout)
                .unwrap_or_else(|| "inline".into()),
        ),
        header_props_json: Some(serde_json::to_string(&props).unwrap_or_else(|_| "{}".into())),
        schema_version: Some(1),
        deleted_at: existing
            .and_then(|e| e.deleted_at.as_ref())
            .filter(|v| !v.is_null())
            .map(|v| ark_timestamp_to_millis(v, updated_at)),
        ..Default::default()
    })
}
