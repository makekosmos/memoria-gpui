//! Port of `obsidianVaultExportAssets.ts` — header-side image field handling:
//! which fields carry images, entry-id → asset resolution, and value rewrite.

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

use crate::model::{Entry, NoteType};
use crate::note_type_schemas::{parse_header_template, parse_note_type_definition};

/// header template + every `kind: "image"` schema field.
pub(crate) fn export_image_field_ids(
    entry: &Entry,
    note_type: Option<&NoteType>,
) -> HashSet<String> {
    let mut field_ids = HashSet::new();
    if entry.type_id.as_deref() == Some("image_obj") || note_type.is_some_and(|t| t.slug == "image")
    {
        field_ids.insert("image".to_string());
    }
    let Some(note_type) = note_type else {
        return field_ids;
    };
    if let Ok(template) = parse_header_template(&note_type.header_template_json) {
        if let Some(id) = template.image_field_id {
            field_ids.insert(id);
        }
    }
    if let Ok(definition) = parse_note_type_definition(&note_type.schema_json) {
        for field in definition.fields {
            if field.kind == "image" {
                field_ids.insert(field.id);
            }
        }
    }
    field_ids
}

pub(crate) fn parse_entry_header_props_json_loose(raw: Option<&str>) -> Option<Map<String, Value>> {
    let parsed: Value = serde_json::from_str(raw.unwrap_or("{}")).ok()?;
    parsed.as_object().cloned()
}

pub(crate) struct HeaderImageAssetReference {
    pub(crate) source: String,
    pub(crate) preferred_file_name: Option<String>,
    pub(crate) mime_type: Option<String>,
}

pub(crate) fn resolve_header_image_asset_reference(
    field_id: &str,
    value: &Value,
    header_props: &Map<String, Value>,
    entries_by_id: &HashMap<String, Entry>,
) -> Option<HeaderImageAssetReference> {
    let candidate = first_string_value(value);
    let inline_metadata = extract_header_image_asset_metadata(value);
    if let Some(linked) = candidate.as_deref().and_then(|id| entries_by_id.get(id)) {
        let linked_props = parse_entry_header_props_json_loose(linked.header_props_json.as_deref())
            .unwrap_or_default();
        let source = linked_props
            .get("source_path")
            .and_then(first_string_value)
            .or_else(|| linked_props.get("image").and_then(first_string_value))?;
        return Some(HeaderImageAssetReference {
            source,
            preferred_file_name: linked_props.get("file_name").and_then(first_string_value),
            mime_type: linked_props.get("mime_type").and_then(first_string_value),
        });
    }

    let meta_source = inline_metadata.as_ref().map(|m| m.source.clone());
    let meta_pref = inline_metadata
        .as_ref()
        .and_then(|m| m.preferred_file_name.clone());
    let meta_mime = inline_metadata.as_ref().and_then(|m| m.mime_type.clone());
    let metadata_source = if field_id == "image" {
        header_props
            .get("source_path")
            .and_then(first_string_value)
            .or(meta_source)
    } else {
        meta_source
    };
    let source = metadata_source.or(candidate)?;
    Some(HeaderImageAssetReference {
        source,
        preferred_file_name: if field_id == "image" {
            header_props
                .get("file_name")
                .and_then(first_string_value)
                .or(meta_pref)
        } else {
            meta_pref
        },
        mime_type: if field_id == "image" {
            header_props
                .get("mime_type")
                .and_then(first_string_value)
                .or(meta_mime)
        } else {
            meta_mime
        },
    })
}

pub(crate) fn extract_header_image_asset_metadata(
    value: &Value,
) -> Option<HeaderImageAssetReference> {
    let map = value.as_object()?;
    let source = map
        .get("source_path")
        .and_then(first_string_value)
        .or_else(|| map.get("image").and_then(first_string_value))?;
    Some(HeaderImageAssetReference {
        source,
        preferred_file_name: map.get("file_name").and_then(first_string_value),
        mime_type: map.get("mime_type").and_then(first_string_value),
    })
}

pub(crate) fn first_string_value(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => {
            let trimmed = s.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Value::Array(items) => items.iter().find_map(first_string_value),
        _ => None,
    }
}

pub(crate) fn rewrite_header_image_value(value: &Value, rewritten_relative_path: &str) -> Value {
    if let Value::Array(items) = value {
        let mut replaced = false;
        return Value::Array(
            items
                .iter()
                .map(|item| {
                    if !replaced && item.as_str().is_some_and(|s| !s.trim().is_empty()) {
                        replaced = true;
                        return Value::String(rewritten_relative_path.to_string());
                    }
                    item.clone()
                })
                .collect(),
        );
    }
    Value::String(rewritten_relative_path.to_string())
}
