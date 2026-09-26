//! Shared object-view logic — ports of the `TypeObjectsView.vue` summary-field
//! selection, `getCollectionTargetTypeId` (edenStoreHelpers.ts) and the person
//! display-name fallback.

use serde_json::Value;

use crate::entry_titles::get_entry_display_title;
use crate::model::{Entry, NoteType, ResolvedNoteTypeField};
use crate::note_type_fields::resolve_note_type_fields;
use crate::note_type_schemas::parse_note_type_definition;
use crate::note_types::parse_note_type_ui_schema;
use crate::system_types_data::SYSTEM_TYPE_COLLECTION_ID;

/// `parseEntryHeaderProps` — tolerates empty/invalid JSON like the Vue helper.
pub fn parse_entry_header_props(entry: &Entry) -> serde_json::Map<String, Value> {
    entry
        .header_props_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

/// `getCollectionTargetTypeId` — collection objects point at the note type
/// they list via `object_type_id` in header props.
pub fn collection_target_type_id(entry: Option<&Entry>) -> Option<String> {
    let entry = entry?;
    if entry.type_id.as_deref() != Some(SYSTEM_TYPE_COLLECTION_ID) {
        return None;
    }
    let props = parse_entry_header_props(entry);
    match props.get("object_type_id") {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.clone()),
        _ => None,
    }
}

/// `summaryFields` in TypeObjectsView.vue — at most two visible non-media,
/// non-relation fields; `featured_fields` order wins, then schema order.
pub fn summary_fields(note_type: &NoteType) -> Vec<ResolvedNoteTypeField> {
    const HIDDEN: [&str; 6] = [
        "description",
        "related_notes",
        "created_at",
        "updated_at",
        "deleted_at",
        "source_path",
    ];
    let resolved = resolve_note_type_fields(Some(note_type));
    let preferred: Vec<String> = parse_note_type_ui_schema(note_type.ui_schema_json.as_deref())
        .ok()
        .and_then(|s| s.featured_fields)
        .unwrap_or_default();
    let definition = parse_note_type_definition(&note_type.schema_json)
        .map(|d| d.fields)
        .unwrap_or_default();
    let mut fields: Vec<ResolvedNoteTypeField> = resolved
        .into_iter()
        .filter(|f| f.visible)
        .filter(|f| !matches!(f.field.kind.as_str(), "image" | "long_text" | "relation"))
        .filter(|f| !HIDDEN.contains(&f.field.id.as_str()))
        .collect();
    // Vue sorts by (preferred index ?? 999) then (definition index ?? 999) —
    // featured fields first in their declared order, schema order after.
    fields.sort_by_key(|f| {
        let preferred_rank = preferred
            .iter()
            .position(|p| p == &f.field.id)
            .map(|i| i as i64)
            .unwrap_or(999);
        let def_rank = definition
            .iter()
            .position(|d| d.id == f.field.id)
            .map(|i| i as i64)
            .unwrap_or(999);
        (preferred_rank, def_rank)
    });
    fields.truncate(2);
    fields
}

/// `getPersonDisplayName` — first/patronymic/last joined, else the display title.
pub fn person_display_name(entry: &Entry) -> String {
    let props = parse_entry_header_props(entry);
    let parts = ["first_name", "patronymic", "last_name"]
        .iter()
        .filter_map(|k| props.get(*k))
        .filter_map(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    if !parts.is_empty() {
        return parts.join(" ");
    }
    entry_display_title(entry)
}

/// Display title for any entry (`getEntryDisplayTitle` shortcut). The raw
/// `header_props_json` string is passed as `Value::String` — `entry_titles`
/// parses it exactly like the Vue helper.
pub fn entry_display_title(entry: &Entry) -> String {
    let source = entry.header_props_json.as_deref().map(Value::from);
    get_entry_display_title(Some(entry.title.as_str()), source.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_types_data::SYSTEM_TYPE_PERSON_ID;

    fn entry_with(type_id: &str, props: &str) -> Entry {
        Entry {
            id: "e1".into(),
            type_id: Some(type_id.into()),
            header_props_json: Some(props.into()),
            ..Default::default()
        }
    }

    #[test]
    fn collection_target() {
        let e = entry_with(
            SYSTEM_TYPE_COLLECTION_ID,
            r#"{"object_type_id":"book_obj"}"#,
        );
        assert_eq!(collection_target_type_id(Some(&e)), Some("book_obj".into()));
        let e2 = entry_with("note_obj", r#"{"object_type_id":"book_obj"}"#);
        assert_eq!(collection_target_type_id(Some(&e2)), None);
        let e3 = entry_with(SYSTEM_TYPE_COLLECTION_ID, r#"{}"#);
        assert_eq!(collection_target_type_id(Some(&e3)), None);
    }

    #[test]
    fn person_name() {
        let e = entry_with(
            SYSTEM_TYPE_PERSON_ID,
            r#"{"first_name":"Анна","patronymic":"","last_name":"Смирнова"}"#,
        );
        assert_eq!(person_display_name(&e), "Анна Смирнова");
        let e2 = Entry {
            title: "Тест".into(),
            type_id: Some(SYSTEM_TYPE_PERSON_ID.into()),
            header_props_json: Some("{}".into()),
            ..Default::default()
        };
        assert_eq!(person_display_name(&e2), "Тест");
    }

    #[test]
    fn summary_prefers_featured_then_definition() {
        let nt = NoteType {
            id: "game_obj".into(),
            name: "Игра".into(),
            schema_json: r#"{"fields":[
                {"id":"publisher","label":"Издатель","kind":"text","required":false},
                {"id":"play_status","label":"Статус","kind":"select","required":false},
                {"id":"image","label":"Обложка","kind":"image","required":false},
                {"id":"description","label":"Описание","kind":"long_text","required":false}
            ]}"#
            .into(),
            ui_schema_json: Some(
                r#"{"featured_fields":["play_status"],"visible_fields":["publisher"],"hidden_fields":[]}"#
                    .into(),
            ),
            ..Default::default()
        };
        let fields = summary_fields(&nt);
        let ids: Vec<&str> = fields.iter().map(|f| f.field.id.as_str()).collect();
        assert_eq!(ids, vec!["play_status", "publisher"]);
    }
}
