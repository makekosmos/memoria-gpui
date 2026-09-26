//! Port of `tests/liveRefresh.test.ts` — `shouldApplyRemoteEntry` decisions.

use memoria_gpui::content::write_entry_markdown;
use memoria_gpui::live_refresh::{
    is_older_remote_entry, should_apply_remote_entry, RemoteEntryDecision, RemoteEntryParams,
};
use memoria_gpui::model::Entry;

fn make_entry(id: &str, markdown: &str) -> Entry {
    Entry {
        id: id.into(),
        title: "Тест".into(),
        content_json: serde_json::to_string(&write_entry_markdown(markdown)).unwrap(),
        created_at: 1000,
        updated_at: 2000,
        type_id: Some("note_obj".into()),
        header_props_json: Some("{}".into()),
        schema_version: Some(1),
        ..Default::default()
    }
}

fn decide(fresh: &Entry, current: &Entry, dirty: bool) -> RemoteEntryDecision {
    should_apply_remote_entry(&RemoteEntryParams {
        fresh,
        current_content_json: &current.content_json,
        current_entry: None,
        is_editor_dirty: dirty,
    })
}

#[test]
fn changed_content_clean_editor_applies() {
    let current = make_entry("abc", "старый текст");
    let fresh = make_entry("abc", "новый текст от другого устройства");
    assert_eq!(decide(&fresh, &current, false), RemoteEntryDecision::Apply);
}

#[test]
fn same_content_is_self_echo_skip() {
    let current = make_entry("abc", "мой текст");
    let fresh = make_entry("abc", "мой текст");
    assert_eq!(
        decide(&fresh, &current, false),
        RemoteEntryDecision::SkipSameContent
    );
    // … даже когда редактор dirty.
    assert_eq!(
        decide(&fresh, &current, true),
        RemoteEntryDecision::SkipSameContent
    );
}

#[test]
fn changed_content_dirty_editor_skips_dirty() {
    let current = make_entry("abc", "мой черновик");
    let fresh = make_entry("abc", "удалённое изменение");
    assert_eq!(
        decide(&fresh, &current, true),
        RemoteEntryDecision::SkipDirty
    );
}

#[test]
fn equal_markdown_across_json_wrappers_is_a_skip() {
    // The content-equality guard compares markdown, not the JSON envelope —
    // a bare doc and a markdown envelope with the same text both skip.
    let markdown = "# Привет\n\nМир";
    let current = make_entry("abc", markdown);
    let mut fresh = make_entry("abc", markdown);
    fresh.content_json = serde_json::to_string(&serde_json::json!({
        "type": "doc",
        "content": [
            { "type": "heading", "attrs": { "level": 1 },
              "content": [{ "type": "text", "text": "Привет" }] },
            { "type": "paragraph", "content": [{ "type": "text", "text": "Мир" }] },
        ],
    }))
    .unwrap();
    assert_eq!(
        decide(&fresh, &current, false),
        RemoteEntryDecision::SkipSameContent
    );
}

#[test]
fn empty_current_content_with_fresh_nonempty_applies() {
    let current = make_entry("abc", "");
    let fresh = make_entry("abc", "новый контент");
    assert_eq!(decide(&fresh, &current, false), RemoteEntryDecision::Apply);
}

#[test]
fn metadata_only_remote_update_is_not_a_self_echo() {
    let current = make_entry("abc", "same text");
    let mut fresh = make_entry("abc", "same text");
    fresh.title = "Новое имя".into();
    let decision = should_apply_remote_entry(&RemoteEntryParams {
        fresh: &fresh,
        current_entry: Some(&current),
        current_content_json: &current.content_json,
        is_editor_dirty: false,
    });
    assert_eq!(decision, RemoteEntryDecision::Apply);
}

#[test]
fn metadata_only_update_while_dirty_is_a_conflict() {
    let current = make_entry("abc", "same text");
    let mut fresh = make_entry("abc", "same text");
    fresh.header_layout = Some("column".into());
    let decision = should_apply_remote_entry(&RemoteEntryParams {
        fresh: &fresh,
        current_entry: Some(&current),
        current_content_json: &current.content_json,
        is_editor_dirty: true,
    });
    assert_eq!(decision, RemoteEntryDecision::SkipDirty);
}

#[test]
fn older_remote_entry_is_rejected() {
    let mut stale = make_entry("abc", "старое");
    stale.updated_at = 100;
    let mut current = make_entry("abc", "новое");
    current.updated_at = 101;
    assert!(is_older_remote_entry(&stale, &current));

    let mut same = make_entry("abc", "такая же");
    same.updated_at = 100;
    current.updated_at = 100;
    assert!(!is_older_remote_entry(&same, &current));
    let mut newer = make_entry("abc", "новее");
    newer.updated_at = 101;
    assert!(!is_older_remote_entry(&newer, &current));
}
