//! `typedNoteSchemas.ts` port — the zod schemas become hand-rolled validators
//! over `serde_json::Value` (same accept/reject surface; errors → `Err`).

use serde_json::{Map, Value};

use crate::model::{HeaderTemplate, NoteType, NoteTypeDefinition, NoteTypeField, NoteTypeUiSchema};

const NOTE_FIELD_KINDS: &[&str] = &[
    "text",
    "url",
    "long_text",
    "number",
    "date",
    "boolean",
    "select",
    "multi_select",
    "image",
    "relation",
];
const HEADER_TEMPLATE_KINDS: &[&str] = &["default", "centered_profile"];
const HEADER_LAYOUT_KINDS: &[&str] = &["inline", "column"];
const DEFAULT_LAYOUT_KINDS: &[&str] = &["page", "list", "gallery", "board"];

type Res<T> = Result<T, String>;

fn err(label: &str, detail: &str) -> String {
    format!("{label}: invalid JSON ({detail})")
}

fn str_at<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    map.get(key).and_then(Value::as_str)
}

fn str_list(value: Option<&Value>, min_len: bool) -> Option<Vec<String>> {
    match value {
        None => None,
        Some(Value::Array(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                let s = item.as_str()?;
                if min_len && s.is_empty() {
                    return None;
                }
                out.push(s.to_string());
            }
            Some(out)
        }
        Some(_) => None,
    }
}

/// `noteTypeFieldSchema.parse` — declared keys only; unknown keys dropped.
fn parse_field(value: &Value) -> Res<NoteTypeField> {
    let map = value
        .as_object()
        .ok_or_else(|| err("field", "not an object"))?;
    let id = str_at(map, "id")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| err("field", "id"))?;
    let label = str_at(map, "label")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| err("field", "label"))?;
    let kind = str_at(map, "kind")
        .filter(|k| NOTE_FIELD_KINDS.contains(k))
        .ok_or_else(|| err("field", "kind"))?;
    let required = map
        .get("required")
        .and_then(Value::as_bool)
        .ok_or_else(|| err("field", "required"))?;
    let opt_str = |key: &str| -> Res<Option<String>> {
        match map.get(key) {
            None => Ok(None),
            Some(v) => v
                .as_str()
                .map(|s| s.to_string())
                .map(Some)
                .ok_or_else(|| err("field", key)),
        }
    };
    let opt_bool_key = |key: &str| -> Res<Option<bool>> {
        match map.get(key) {
            None => Ok(None),
            Some(v) => v.as_bool().map(Some).ok_or_else(|| err("field", key)),
        }
    };
    let opt_list = |key: &str| -> Res<Option<Vec<String>>> {
        match map.get(key) {
            None => Ok(None),
            Some(v) => str_list(Some(v), true)
                .map(Some)
                .ok_or_else(|| err("field", key)),
        }
    };
    Ok(NoteTypeField {
        id: id.into(),
        label: label.into(),
        kind: kind.into(),
        required,
        options: opt_list("options")?,
        placeholder: opt_str("placeholder")?,
        visible: opt_bool_key("visible")?,
        read_only: opt_bool_key("read_only")?,
        multiple: opt_bool_key("multiple")?,
        system: opt_bool_key("system")?,
        link_type: opt_str("link_type")?,
        allowed_object_types: opt_list("allowed_object_types")?,
    })
}

/// `noteTypeDefinitionSchema.parse` over a JSON string.
pub fn parse_note_type_definition(schema_json: &str) -> Res<NoteTypeDefinition> {
    let parsed: Value =
        serde_json::from_str(schema_json).map_err(|e| err("note type schema", &e.to_string()))?;
    let map = parsed
        .as_object()
        .ok_or_else(|| err("note type schema", "not an object"))?;
    let fields = match map.get("fields") {
        Some(Value::Array(items)) => items.iter().map(parse_field).collect::<Res<Vec<_>>>()?,
        _ => return Err(err("note type schema", "fields")),
    };
    Ok(NoteTypeDefinition { fields })
}

/// `headerTemplateSchema.parse` over a JSON string.
pub fn parse_header_template(template_json: &str) -> Res<HeaderTemplate> {
    let parsed: Value = serde_json::from_str(template_json)
        .map_err(|e| err("note type header template", &e.to_string()))?;
    let map = parsed
        .as_object()
        .ok_or_else(|| err("note type header template", "not an object"))?;
    let kind = str_at(map, "kind")
        .filter(|k| HEADER_TEMPLATE_KINDS.contains(k))
        .ok_or_else(|| err("note type header template", "kind"))?;
    let opt_list = |key: &str| -> Res<Option<Vec<String>>> {
        match map.get(key) {
            None => Ok(None),
            Some(v) => str_list(Some(v), false)
                .map(Some)
                .ok_or_else(|| err("note type header template", key)),
        }
    };
    let image_field_id = match map.get("imageFieldId") {
        None | Some(Value::Null) => None,
        Some(v) => v
            .as_str()
            .map(|s| s.to_string())
            .map(Some)
            .ok_or_else(|| err("note type header template", "imageFieldId"))?,
    };
    Ok(HeaderTemplate {
        kind: kind.into(),
        primary_field_ids: opt_list("primaryFieldIds")?,
        secondary_field_ids: opt_list("secondaryFieldIds")?,
        image_field_id,
    })
}

/// `noteTypeUiSchema.parse` on an already-parsed object (defaults applied by
/// the caller — see `note_types::parse_note_type_ui_schema`).
pub fn parse_note_type_ui_schema_value(parsed: &Value) -> Res<NoteTypeUiSchema> {
    let map = parsed
        .as_object()
        .ok_or_else(|| err("note type UI schema", "not an object"))?;
    let opt_list = |key: &str| -> Res<Option<Vec<String>>> {
        match map.get(key) {
            None => Ok(None),
            Some(v) => str_list(Some(v), false)
                .map(Some)
                .ok_or_else(|| err("note type UI schema", key)),
        }
    };
    let opt_enum = |key: &str, kinds: &[&str]| -> Res<Option<String>> {
        match map.get(key) {
            None => Ok(None),
            Some(v) => v
                .as_str()
                .filter(|s| kinds.contains(s))
                .map(|s| s.to_string())
                .map(Some)
                .ok_or_else(|| err("note type UI schema", key)),
        }
    };
    let default_template_id = match map.get("default_template_id") {
        // `null`/`absent` collapse — Vue merges via `??` anyway.
        None | Some(Value::Null) => None,
        Some(v) => v
            .as_str()
            .map(|s| s.to_string())
            .map(Some)
            .ok_or_else(|| err("note type UI schema", "default_template_id"))?,
    };
    let collection_name = match map.get("collection_name") {
        None => None,
        Some(v) => v
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .map(Some)
            .ok_or_else(|| err("note type UI schema", "collection_name"))?,
    };
    Ok(NoteTypeUiSchema {
        featured_fields: opt_list("featured_fields")?,
        visible_fields: opt_list("visible_fields")?,
        hidden_fields: opt_list("hidden_fields")?,
        read_only_fields: opt_list("read_only_fields")?,
        field_order: opt_list("field_order")?,
        header_layout: opt_enum("header_layout", HEADER_LAYOUT_KINDS)?,
        default_layout: opt_enum("default_layout", DEFAULT_LAYOUT_KINDS)?,
        default_template_id,
        collection_name,
    })
}

/// `noteTypeSchema.parse` on a `Value` (used by `recordToNoteType`).
pub fn parse_note_type_value(value: &Value) -> Res<NoteType> {
    let map = value
        .as_object()
        .ok_or_else(|| err("note type", "not an object"))?;
    let need_str = |key: &str| -> Res<String> {
        str_at(map, key)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .ok_or_else(|| err("note type", key))
    };
    let nullable_str = |key: &str| -> Res<Option<String>> {
        match map.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v
                .as_str()
                .map(|s| s.to_string())
                .map(Some)
                .ok_or_else(|| err("note type", key)),
        }
    };
    let opt_str = |key: &str| -> Res<Option<String>> {
        match map.get(key) {
            None => Ok(None),
            Some(v) => v
                .as_str()
                .map(|s| s.to_string())
                .map(Some)
                .ok_or_else(|| err("note type", key)),
        }
    };
    let number = |key: &str| -> Res<i64> {
        map.get(key)
            .and_then(Value::as_f64)
            .map(|n| n as i64)
            .ok_or_else(|| err("note type", key))
    };
    // `icon`/`color` are `.nullable()` — an absent key is a zod failure, so
    // require the key to exist (as string or null).
    for key in ["icon", "color"] {
        if !map.contains_key(key) {
            return Err(err("note type", key));
        }
    }
    Ok(NoteType {
        id: need_str("id")?,
        name: need_str("name")?,
        slug: need_str("slug")?,
        icon: nullable_str("icon")?,
        color: nullable_str("color")?,
        schema_json: map
            .get("schema_json")
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .ok_or_else(|| err("note type", "schema_json"))?,
        header_template_json: map
            .get("header_template_json")
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .ok_or_else(|| err("note type", "header_template_json"))?,
        ui_schema_json: opt_str("ui_schema_json")?,
        created_at: number("created_at")?,
        updated_at: number("updated_at")?,
        extra: Map::new(),
    })
}
