//! Port of `src/store/entryChanges.ts` — decides whether a draft entry
//! actually differs from the persisted one. Header props and layout are
//! normalized through the note-type schema first so schema defaults don't
//! count as user edits (postmortem 2026-06-17).

use serde_json::Value;

use crate::content::{is_entry_tiptap_content, read_entry_markdown, read_entry_tiptap_doc};
use crate::header_props::normalize_header_props;
use crate::model::{Entry, NoteType};
use crate::note_types::resolve_note_type_header_layout;

fn normalized_entry_type_id(entry: &Entry) -> &str {
    entry.type_id.as_deref().unwrap_or("note_obj")
}

/// `resolveEntryHeaderLayout` — explicit layout wins; otherwise the type's.
fn resolve_entry_header_layout(entry: &Entry, note_types: &[NoteType]) -> String {
    if let Some(layout) = &entry.header_layout {
        return layout.clone();
    }
    let note_type = note_types
        .iter()
        .find(|nt| nt.id == normalized_entry_type_id(entry));
    resolve_note_type_header_layout(note_type)
}

/// `resolveEntryHeaderProps` — `JSON.stringify(normalizeHeaderProps(...))`
/// with `{}` on unparseable stored props.
fn resolve_entry_header_props(entry: &Entry, note_types: &[NoteType]) -> String {
    let note_type = note_types
        .iter()
        .find(|nt| nt.id == normalized_entry_type_id(entry));
    let raw: Value = entry
        .header_props_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(Value::Object(Default::default()));
    serde_json::to_string(&normalize_header_props(note_type, &raw)).unwrap_or_else(|_| "{}".into())
}

/// `entryBodyChanged` — tiptap compare for tiptap content, markdown otherwise.
fn entry_body_changed(next: &Entry, previous: &Entry) -> bool {
    let next_json: Value = serde_json::from_str(&next.content_json).unwrap_or(Value::Null);
    let prev_json: Value = serde_json::from_str(&previous.content_json).unwrap_or(Value::Null);
    if is_entry_tiptap_content(&next_json) || is_entry_tiptap_content(&prev_json) {
        // Vue compares `JSON.stringify` output — key order is significant.
        return serde_json::to_string(&read_entry_tiptap_doc(&next_json)).unwrap_or_default()
            != serde_json::to_string(&read_entry_tiptap_doc(&prev_json)).unwrap_or_default();
    }
    read_entry_markdown(&next_json) != read_entry_markdown(&prev_json)
}

/// `hasUserVisibleEntryChanges`.
pub fn has_user_visible_entry_changes(
    next: &Entry,
    previous: &Entry,
    note_types: &[NoteType],
) -> bool {
    if next.title != previous.title {
        return true;
    }
    if normalized_entry_type_id(next) != normalized_entry_type_id(previous) {
        return true;
    }
    if resolve_entry_header_layout(next, note_types)
        != resolve_entry_header_layout(previous, note_types)
    {
        return true;
    }
    if resolve_entry_header_props(next, note_types)
        != resolve_entry_header_props(previous, note_types)
    {
        return true;
    }
    entry_body_changed(next, previous)
}
