//! `typedNotes.ts` field resolution — `resolveNoteTypeFields`,
//! `getResolvedNoteTypeField`, `getNoteTypePresentation`.

use std::collections::{HashMap, HashSet};

use crate::model::{NoteType, NoteTypeField, NoteTypePresentation, ResolvedNoteTypeField};
use crate::note_type_schemas::{parse_header_template, parse_note_type_definition};
use crate::note_types::{parse_note_type_ui_schema, resolve_note_type_header_layout};

fn legacy_featured_field_ids(note_type: &NoteType) -> Vec<String> {
    let Ok(t) = parse_header_template(&note_type.header_template_json) else {
        return Vec::new();
    };
    let mut out = t.primary_field_ids.unwrap_or_default();
    out.extend(t.secondary_field_ids.unwrap_or_default());
    out
}

fn legacy_image_field_id(note_type: &NoteType) -> Option<String> {
    parse_header_template(&note_type.header_template_json)
        .ok()
        .and_then(|t| t.image_field_id)
}

fn resolved_field_order(
    definition_fields: &[NoteTypeField],
    field_order: &[String],
) -> Vec<String> {
    let mut ordered: Vec<String> = field_order
        .iter()
        .filter(|id| definition_fields.iter().any(|f| &f.id == *id))
        .cloned()
        .collect();
    for f in definition_fields {
        if !ordered.contains(&f.id) {
            ordered.push(f.id.clone());
        }
    }
    ordered
}

/// `resolveNoteTypeFields` — ui-schema lists decide visibility when any is
/// non-empty; `field.visible === false` and `hidden_fields` force-hide;
/// fields absent from `field_order` sort at index 0 (Vue `?? 0`).
pub fn resolve_note_type_fields(note_type: Option<&NoteType>) -> Vec<ResolvedNoteTypeField> {
    let Some(note_type) = note_type else {
        return Vec::new();
    };
    let definition = parse_note_type_definition(&note_type.schema_json).unwrap_or_default();
    let ui = parse_note_type_ui_schema(note_type.ui_schema_json.as_deref()).unwrap_or_default();
    let featured = as_set(ui.featured_fields.as_deref());
    let visible = as_set(ui.visible_fields.as_deref());
    let hidden = as_set(ui.hidden_fields.as_deref());
    let read_only = as_set(ui.read_only_fields.as_deref());
    let explicit_visibility = !featured.is_empty() || !visible.is_empty() || !hidden.is_empty();
    let order = resolved_field_order(&definition.fields, ui.field_order.as_deref().unwrap_or(&[]));
    let index: HashMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();

    let mut fields = definition.fields;
    fields.sort_by_key(|f| index.get(f.id.as_str()).copied().unwrap_or(0));
    fields
        .into_iter()
        .map(|field| {
            let mut vis = field.visible != Some(false);
            if hidden.contains(field.id.as_str()) {
                vis = false;
            } else if explicit_visibility {
                vis = featured.contains(field.id.as_str()) || visible.contains(field.id.as_str());
            }
            ResolvedNoteTypeField {
                read_only: field.read_only == Some(true) || read_only.contains(field.id.as_str()),
                visible: vis,
                field,
            }
        })
        .collect()
}

fn as_set(list: Option<&[String]>) -> HashSet<&str> {
    list.unwrap_or_default()
        .iter()
        .map(String::as_str)
        .collect()
}

/// `getResolvedNoteTypeField`.
pub fn get_resolved_note_type_field(
    note_type: Option<&NoteType>,
    field_id: &str,
) -> Option<ResolvedNoteTypeField> {
    resolve_note_type_fields(note_type)
        .into_iter()
        .find(|f| f.field.id == field_id)
}

/// `getNoteTypePresentation`.
pub fn get_note_type_presentation(note_type: Option<&NoteType>) -> NoteTypePresentation {
    let resolved = resolve_note_type_fields(note_type);
    let ui = note_type
        .map(|nt| parse_note_type_ui_schema(nt.ui_schema_json.as_deref()).unwrap_or_default())
        .unwrap_or_default();
    let featured: HashSet<String> = match ui.featured_fields.as_deref() {
        Some(list) if !list.is_empty() => list.iter().cloned().collect(),
        _ => note_type
            .map(legacy_featured_field_ids)
            .unwrap_or_default()
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect(),
    };
    let definition = note_type
        .and_then(|nt| parse_note_type_definition(&nt.schema_json).ok())
        .unwrap_or_default();
    let field_order =
        resolved_field_order(&definition.fields, ui.field_order.as_deref().unwrap_or(&[]));
    let description_field = resolved
        .iter()
        .find(|f| f.field.id == "description" && f.visible)
        .cloned();
    let non_description: Vec<&ResolvedNoteTypeField> = resolved
        .iter()
        .filter(|f| f.visible && f.field.id != "description")
        .collect();
    let featured_fields: Vec<ResolvedNoteTypeField> = non_description
        .iter()
        .filter(|f| featured.contains(f.field.id.as_str()))
        .map(|f| (*f).clone())
        .collect();
    let secondary_fields: Vec<ResolvedNoteTypeField> = non_description
        .iter()
        .filter(|f| !featured.contains(f.field.id.as_str()))
        .map(|f| (*f).clone())
        .collect();
    let image_field_id = note_type.and_then(legacy_image_field_id).or_else(|| {
        resolved
            .iter()
            .find(|f| f.field.kind == "image" && f.visible)
            .map(|f| f.field.id.clone())
    });
    NoteTypePresentation {
        header_layout: resolve_note_type_header_layout(note_type),
        featured_fields,
        secondary_fields,
        description_field,
        field_order,
        image_field_id,
    }
}
