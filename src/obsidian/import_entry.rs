//! Entry-shaping helpers for the import apply loop — Vue parity for
//! `existingEntryForImport`, source stamping, and `normalizeHeaderProps` use.

use serde_json::{json, Map, Value};

use crate::mapping::parse_header_props_json;
use crate::model::{Entry, NoteType};
use crate::obsidian::{canonical_path_identity, obsidian_note_id, ObsidianImportDraft};

pub(super) fn title_key(title: &str) -> String {
    title.trim().to_lowercase()
}

/// `existingEntryForImport` — match by draft id, stable id, or source identity.
pub(super) fn existing_entry_for_import<'a>(
    draft: &ObsidianImportDraft,
    vault_identity: &str,
    entries: &'a [Entry],
) -> Option<&'a Entry> {
    if let Some(id) = &draft.id {
        if let Some(entry) = entries.iter().find(|e| &e.id == id) {
            return Some(entry);
        }
    }
    let stable_id = obsidian_note_id(vault_identity, &draft.relative_path);
    let key = format!(
        "{}\0{}",
        canonical_path_identity(vault_identity),
        canonical_path_identity(&draft.relative_path)
    );
    entries.iter().find(|entry| {
        if entry.id == stable_id {
            return true;
        }
        let props = parse_header_props_json(entry.header_props_json.as_deref());
        let vault = props
            .get("source_vault_key")
            .or_else(|| props.get("source_vault"))
            .and_then(Value::as_str);
        let rel = props
            .get("source_relative_path_key")
            .or_else(|| props.get("source_relative_path"))
            .and_then(Value::as_str);
        matches!((vault, rel), (Some(v), Some(r))
            if format!("{}\0{}", canonical_path_identity(v), canonical_path_identity(r)) == key)
    })
}

pub(super) fn source_stamped_props(
    draft: &ObsidianImportDraft,
    vault_identity: &str,
) -> Map<String, Value> {
    let mut props = draft.header_props.clone();
    props.insert("source_path".into(), json!(draft.source_path));
    props.insert("source_vault".into(), json!(vault_identity));
    props.insert(
        "source_vault_key".into(),
        json!(canonical_path_identity(vault_identity)),
    );
    props.insert("source_relative_path".into(), json!(draft.relative_path));
    props.insert(
        "source_relative_path_key".into(),
        json!(canonical_path_identity(&draft.relative_path)),
    );
    props
}

pub(super) fn normalized_props_json(
    note_type: Option<&NoteType>,
    props: Map<String, Value>,
) -> String {
    crate::header_props::normalize_header_props(note_type, &Value::Object(props))
        .map(|m| serde_json::to_string(&Value::Object(m)).unwrap_or_else(|_| "{}".into()))
        .unwrap_or_else(|_| "{}".into())
}

pub(super) fn to_entry(
    draft: &ObsidianImportDraft,
    entry_id: String,
    existing: Option<&Entry>,
    note_type: Option<&NoteType>,
    vault_identity: &str,
    now: i64,
) -> Entry {
    Entry {
        id: entry_id.clone(),
        title: if entry_id.ends_with(":copy") {
            format!("{} (copy)", draft.title)
        } else {
            draft.title.clone()
        },
        content_json: serde_json::to_string(&crate::content::write_entry_markdown(
            &draft.body_markdown,
        ))
        .unwrap_or_default(),
        created_at: existing.map(|e| e.created_at).unwrap_or(now),
        updated_at: now,
        folder_id: existing.and_then(|e| e.folder_id.clone()),
        type_id: Some(draft.type_id.clone()),
        header_layout: existing.and_then(|e| e.header_layout.clone()),
        header_props_json: Some(normalized_props_json(
            note_type,
            source_stamped_props(draft, vault_identity),
        )),
        schema_version: existing.and_then(|e| e.schema_version).or(Some(1)),
        deleted_at: None,
        content_loaded: Some(true),
        extra: Map::new(),
    }
}
