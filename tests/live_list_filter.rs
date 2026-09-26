//! Port of `tests/liveListFilter.test.ts`.

use memoria_gpui::live_list_filter::should_include_type_in_eden_list_for_live_update;
use serde_json::{json, Value};

fn include(type_id: &str, props_json: Value, visible: &[&str]) -> bool {
    let visible: Vec<String> = visible.iter().map(|s| s.to_string()).collect();
    should_include_type_in_eden_list_for_live_update(type_id, &props_json, &visible)
}

#[test]
fn bubble_marked_journal_objects_are_excluded() {
    assert!(!include(
        "system-type-journal",
        json!({ "entry_kind": "bubble" }),
        &[]
    ));
    assert!(include("system-type-journal", json!({}), &[]));
}

#[test]
fn empty_visible_ids_includes_ordinary_types() {
    assert!(include("note_obj", json!({}), &[]));
    assert!(include("person_obj", json!({}), &[]));
}

#[test]
fn empty_visible_ids_includes_visible_collections() {
    assert!(include(
        "collection_obj",
        json!({ "object_type_id": "note_obj" }),
        &[]
    ));
    // Canonical `extensions` bag carries the target id too.
    assert!(include(
        "collection_obj",
        json!({ "extensions": { "object_type_id": "note_obj" } }),
        &[]
    ));
}

#[test]
fn hidden_collection_targets_are_excluded() {
    for target in [
        "collection_obj",
        "blocklist_obj",
        "tag_obj",
        "task_obj",
        "time_entry_obj",
    ] {
        assert!(
            !include("collection_obj", json!({ "object_type_id": target }), &[]),
            "target {target} must stay hidden"
        );
    }
    // No target at all → hidden too.
    assert!(!include("collection_obj", json!({}), &[]));
}

#[test]
fn non_empty_visible_ids_filters_by_membership() {
    let visible = ["note_obj", "person_obj"];
    assert!(include("note_obj", json!({}), &visible));
    assert!(include("person_obj", json!({}), &visible));
    assert!(!include("game_obj", json!({}), &visible));
}

#[test]
fn non_empty_visible_ids_filters_collection_targets() {
    let visible = ["note_obj", "person_obj"];
    assert!(include(
        "collection_obj",
        json!({ "object_type_id": "person_obj" }),
        &visible
    ));
    assert!(!include(
        "collection_obj",
        json!({ "object_type_id": "game_obj" }),
        &visible
    ));
    assert!(!include(
        "collection_obj",
        json!({ "object_type_id": "task_obj" }),
        &visible
    ));
}
