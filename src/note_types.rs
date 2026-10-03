//! Port of `src/lib/typedNotes.ts` — schema parsing, ui-schema defaults and
//! naming helpers. Field/presentation resolution lives in `note_type_fields`.

use serde_json::Value;

use crate::model::{NoteType, NoteTypeDefinition, NoteTypeUiSchema};
use crate::note_type_schemas::{
    parse_header_template, parse_note_type_definition, parse_note_type_ui_schema_value,
    parse_note_type_value,
};

/// `createDefaultNoteTypeUiSchema`.
pub(crate) fn default_ui_schema() -> NoteTypeUiSchema {
    NoteTypeUiSchema {
        featured_fields: Some(Vec::new()),
        visible_fields: Some(Vec::new()),
        hidden_fields: Some(vec![
            "created_at".into(),
            "updated_at".into(),
            "deleted_at".into(),
        ]),
        read_only_fields: Some(Vec::new()),
        field_order: Some(Vec::new()),
        header_layout: Some("inline".into()),
        default_layout: Some("page".into()),
        default_template_id: None,
        collection_name: None,
    }
}

impl Default for NoteTypeUiSchema {
    /// `parseNoteTypeUiSchema(undefined)` — schema defaults, not an empty map.
    fn default() -> Self {
        default_ui_schema()
    }
}

/// `parseNoteTypeUiSchema` — `{...defaults, ...parsed}` semantics.
pub fn parse_note_type_ui_schema(ui_schema_json: Option<&str>) -> Result<NoteTypeUiSchema, String> {
    let defaults = default_ui_schema();
    let Some(raw) = ui_schema_json.filter(|s| !s.trim().is_empty()) else {
        return Ok(defaults);
    };
    let parsed: Value = serde_json::from_str(raw)
        .map_err(|e| format!("note type UI schema: invalid JSON ({e})"))?;
    let parsed = parse_note_type_ui_schema_value(&parsed)?;
    let or = |a: Option<Vec<String>>, b: Option<Vec<String>>| a.or(b);
    Ok(NoteTypeUiSchema {
        featured_fields: or(parsed.featured_fields, defaults.featured_fields),
        visible_fields: or(parsed.visible_fields, defaults.visible_fields),
        hidden_fields: or(parsed.hidden_fields, defaults.hidden_fields),
        read_only_fields: or(parsed.read_only_fields, defaults.read_only_fields),
        field_order: or(parsed.field_order, defaults.field_order),
        header_layout: parsed.header_layout.or(defaults.header_layout),
        default_layout: parsed.default_layout.or(defaults.default_layout),
        default_template_id: parsed.default_template_id,
        collection_name: parsed.collection_name,
    })
}

/// `parseNoteTypeDefinition` (keeps the Vue label in error messages).
pub fn parse_definition(schema_json: &str) -> Result<NoteTypeDefinition, String> {
    parse_note_type_definition(schema_json)
}

/// `normalizeSlug`.
pub fn normalize_slug(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_dash = false;
    for ch in input.trim().to_lowercase().chars() {
        if ch.is_whitespace() || ch == '/' || ch == '\\' {
            if !last_dash && !out.is_empty() {
                out.push('-');
                last_dash = true;
            }
            continue;
        }
        out.push(ch);
        last_dash = false;
    }
    // `/^-+|-+$/g` — leading and trailing dash runs both go.
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "note-type".into()
    } else {
        out
    }
}

/// `pluralizeNoteTypeName` — the Vue Russian suffix rules verbatim.
fn pluralize_note_type_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "Объекты".into();
    }
    let normalized = trimmed.to_lowercase();
    for suffix in ["ка", "га", "ха", "жа", "ша", "ща", "ча"] {
        if normalized.ends_with(suffix) {
            let mut out = trimmed.to_string();
            out.pop();
            out.push('и');
            return out;
        }
    }
    if normalized.ends_with('а') {
        let mut out = trimmed.to_string();
        out.pop();
        out.push('ы');
        return out;
    }
    if normalized.ends_with('я') || normalized.ends_with('й') || normalized.ends_with('ь') {
        let mut out = trimmed.to_string();
        out.pop();
        out.push('и');
        return out;
    }
    if normalized
        .chars()
        .last()
        .map(|c| "bcdfghjklmnpqrstvwxyz".contains(c.to_ascii_lowercase()))
        .unwrap_or(false)
    {
        return format!("{trimmed}s");
    }
    format!("{trimmed}ы")
}

/// `getNoteTypeCollectionName`.
pub fn get_note_type_collection_name(note_type: Option<&NoteType>) -> String {
    let Some(note_type) = note_type else {
        return "Объекты".into();
    };
    let ui = parse_note_type_ui_schema(note_type.ui_schema_json.as_deref()).unwrap_or_default();
    ui.collection_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| pluralize_note_type_name(&note_type.name))
}

/// `resolveNoteTypeHeaderLayout`.
pub fn resolve_note_type_header_layout(note_type: Option<&NoteType>) -> String {
    let Some(note_type) = note_type else {
        return "inline".into();
    };
    if let Ok(ui) = parse_note_type_ui_schema(note_type.ui_schema_json.as_deref()) {
        if let Some(layout) = ui.header_layout {
            return layout;
        }
    }
    match parse_header_template(&note_type.header_template_json) {
        Ok(template) if template.kind == "centered_profile" => "column".into(),
        _ => "inline".into(),
    }
}

/// `normalizeNoteType` over a raw payload — strict zod-equivalent parse +
/// `ui_schema_json ?? "{}"` (used for `memoria_note_type` record payloads).
pub fn normalize_note_type_value(value: &Value) -> Result<NoteType, String> {
    let mut value = value.clone();
    if value.get("ui_schema_json").is_none() || value["ui_schema_json"].is_null() {
        value["ui_schema_json"] = Value::String("{}".into());
    }
    parse_note_type_value(&value)
}

/// `normalizeNoteType` — strict parse + `ui_schema_json ?? "{}"`.
pub fn normalize_note_type(note_type: &NoteType) -> Result<NoteType, String> {
    let value = serde_json::to_value(note_type).map_err(|e| e.to_string())?;
    normalize_note_type_value(&value)
}

#[cfg(test)]
mod tests {
    use super::normalize_slug;

    #[test]
    fn slug_strips_leading_and_trailing_dash_runs() {
        // Vue `/^-+|-+$/g` — both ends, not just the tail.
        assert_eq!(normalize_slug("-a"), "a");
        assert_eq!(normalize_slug("--x--"), "x");
        assert_eq!(normalize_slug("-"), "note-type");
        assert_eq!(normalize_slug("a-b"), "a-b");
    }
}
