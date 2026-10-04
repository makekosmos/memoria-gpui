//! Port of `tests/entryChanges.test.ts` — `hasUserVisibleEntryChanges`.

use memoria_model::content::{write_entry_markdown, write_entry_tiptap_doc};
use memoria_model::entry_changes::has_user_visible_entry_changes;
use memoria_model::model::{Entry, NoteType};
use memoria_model::system_types_data::{SYSTEM_TYPE_NOTE, SYSTEM_TYPE_PERSON};
use serde_json::json;
use std::sync::LazyLock;

fn markdown_entry(text: &str) -> String {
    serde_json::to_string(&write_entry_markdown(text)).unwrap()
}

fn make_note_entry() -> Entry {
    Entry {
        id: "test-id".into(),
        title: "Тестовая заметка".into(),
        content_json: markdown_entry("Текст заметки"),
        created_at: 1000,
        updated_at: 2000,
        type_id: Some("note_obj".into()),
        schema_version: Some(1),
        ..Default::default()
    }
}

fn note_types() -> &'static [NoteType] {
    static TYPES: LazyLock<Vec<NoteType>> =
        LazyLock::new(|| vec![SYSTEM_TYPE_NOTE.clone(), SYSTEM_TYPE_PERSON.clone()]);
    &TYPES
}

#[test]
fn null_header_layout_vs_resolved_is_not_a_change() {
    let base = make_note_entry();
    let mut draft = make_note_entry();
    draft.header_layout = Some("inline".into());
    assert!(!has_user_visible_entry_changes(&draft, &base, note_types()).unwrap());
}

#[test]
fn null_header_layout_vs_person_column_is_not_a_change() {
    let mut base = make_note_entry();
    base.type_id = Some("person_obj".into());
    let mut draft = base.clone();
    draft.header_layout = Some("column".into());
    assert!(!has_user_visible_entry_changes(&draft, &base, note_types()).unwrap());
}

#[test]
fn null_header_props_vs_schema_defaults_is_not_a_change() {
    let base = make_note_entry();
    let mut draft = make_note_entry();
    draft.header_props_json = Some(
        serde_json::to_string(&json!({
            "description": "",
            "related_notes": [],
        }))
        .unwrap(),
    );
    assert!(!has_user_visible_entry_changes(&draft, &base, note_types()).unwrap());

    let mut base = make_note_entry();
    base.header_props_json = Some("{}".into());
    assert!(!has_user_visible_entry_changes(&draft, &base, note_types()).unwrap());
}

#[test]
fn person_default_empty_fields_are_not_a_change() {
    let mut base = make_note_entry();
    base.type_id = Some("person_obj".into());
    let mut draft = base.clone();
    draft.header_props_json = Some(
        serde_json::to_string(&json!({
            "first_name": "",
            "last_name": "",
            "patronymic": "",
            "birth_date": "",
            "photo": "",
        }))
        .unwrap(),
    );
    assert!(!has_user_visible_entry_changes(&draft, &base, note_types()).unwrap());
}

#[test]
fn title_change_is_a_change() {
    let mut base = make_note_entry();
    base.title = "Старое".into();
    let mut draft = base.clone();
    draft.title = "Новое".into();
    assert!(has_user_visible_entry_changes(&draft, &base, note_types()).unwrap());
}

#[test]
fn body_change_is_a_change() {
    let mut base = make_note_entry();
    base.content_json = markdown_entry("старый");
    let mut draft = base.clone();
    draft.content_json = markdown_entry("новый");
    assert!(has_user_visible_entry_changes(&draft, &base, note_types()).unwrap());
}

#[test]
fn tiptap_structural_change_with_same_markdown_is_a_change() {
    let mut base = make_note_entry();
    base.content_json = markdown_entry("a");
    let mut draft = base.clone();
    draft.content_json = serde_json::to_string(&write_entry_tiptap_doc(json!({
        "type": "doc",
        "content": [
            { "type": "paragraph", "content": [{ "type": "text", "text": "a" }] },
            { "type": "paragraph" },
        ],
    })))
    .unwrap();
    assert!(has_user_visible_entry_changes(&draft, &base, note_types()).unwrap());
}

#[test]
fn type_id_and_real_header_changes_are_changes() {
    let base = make_note_entry();
    let mut draft = base.clone();
    draft.type_id = Some("person_obj".into());
    assert!(has_user_visible_entry_changes(&draft, &base, note_types()).unwrap());

    let mut base = make_note_entry();
    base.type_id = Some("person_obj".into());
    base.header_props_json = Some(
        serde_json::to_string(&json!({
            "first_name": "", "last_name": "", "patronymic": "",
        }))
        .unwrap(),
    );
    let mut draft = base.clone();
    draft.header_props_json = Some(
        serde_json::to_string(&json!({
            "first_name": "Иван", "last_name": "Иванов", "patronymic": "",
        }))
        .unwrap(),
    );
    assert!(has_user_visible_entry_changes(&draft, &base, note_types()).unwrap());
}

#[test]
fn user_header_layout_change_is_a_change() {
    let mut base = make_note_entry();
    base.header_layout = Some("inline".into());
    let mut draft = base.clone();
    draft.header_layout = Some("column".into());
    assert!(has_user_visible_entry_changes(&draft, &base, note_types()).unwrap());
}

#[test]
fn identical_entries_are_not_a_change() {
    let entry = make_note_entry();
    assert!(!has_user_visible_entry_changes(&entry, &entry, note_types()).unwrap());
}
