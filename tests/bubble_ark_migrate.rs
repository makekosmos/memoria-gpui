//! `tests/bubbleArkApi.test.ts` migration half — `migrateBubble` id
//! determinism, read-back verification (key-order-insensitive), ambiguous
//! occurrence rejection.

mod common;

use common::fake_ark::{api_for, FakeArk};
use memoria_gpui::diary::{BubbleKind, BubbleTimelineNode};
use memoria_gpui::store::bubble_api::migrate_diary;
use memoria_gpui::store::EntryApi;
use serde_json::json;

#[test]
fn migration_is_idempotent_and_rejects_ambiguous_timestamps() {
    let ark = FakeArk::new(false);
    let api = api_for(&ark);
    let source = BubbleTimelineNode {
        id: "legacy".into(),
        date: Some("2026-07-01".into()),
        time: "09:41".into(),
        text: "Legacy".into(),
        content_json: Some(json!({
            "type": "doc",
            "content": [{
                "type": "codeBlock",
                "content": [{ "type": "text", "text": "Legacy" }],
            }],
        })),
        tags: vec!["one".into(), "one".into()],
        kind: BubbleKind::Highlight,
        ..Default::default()
    };
    let first = api.migrate_bubble("local", "legacy", &source).unwrap();
    let second = api.migrate_bubble("local", "legacy", &source).unwrap();
    assert_eq!(second, first);
    assert_eq!(
        ark.objects
            .lock()
            .unwrap()
            .values()
            .filter(|o| o["title"] == "Legacy")
            .count(),
        1
    );
    let migrated = &ark.objects.lock().unwrap()[&first];
    assert_eq!(
        migrated["contentJson"]["doc"]["content"][0]["type"],
        "codeBlock"
    );

    let ambiguous = BubbleTimelineNode {
        id: "ambiguous".into(),
        date: None,
        ..source.clone()
    };
    let err = api.migrate_bubble("local", "ambiguous", &ambiguous);
    assert!(format!("{err:?}").contains("Unresolved"));
}

#[test]
fn migration_read_back_ignores_json_object_key_order() {
    let ark = FakeArk::new(true);
    let api = api_for(&ark);
    let source = BubbleTimelineNode {
        id: "legacy-reordered-json".into(),
        date: Some("2026-07-01".into()),
        time: "09:41".into(),
        text: "Rich legacy".into(),
        content_json: Some(json!({
            "type": "doc",
            "attrs": { "beta": 2, "alpha": 1 },
            "content": [{
                "type": "paragraph",
                "attrs": { "zeta": true, "gamma": false },
                "content": [{ "type": "text", "text": "Rich legacy" }],
            }],
        })),
        tags: vec!["rich".into()],
        kind: BubbleKind::Idea,
        ..Default::default()
    };
    let first = api
        .migrate_bubble("local", &source.id.clone(), &source)
        .unwrap();
    let second = api
        .migrate_bubble("local", "legacy-reordered-json", &source)
        .unwrap();
    assert_eq!(second, first);
    assert_eq!(ark.objects.lock().unwrap().len(), 1);
}

/// `migrate_diary` must enumerate through `list_all_entries` — `list_entries`
/// filters out dated journal objects by design, so going through it made
/// production journal migration a silent no-op (post-review M1 fix).
#[test]
fn migrate_diary_imports_and_deletes_legacy_dated_journals() {
    let ark = FakeArk::new(false);
    let api = api_for(&ark);
    let mut entries = EntryApi::new(ark.clone());
    ark.objects.lock().unwrap().insert(
        "journal-1".into(),
        json!({
            "id": "journal-1",
            "typeId": "system-type-journal",
            "title": "2026-06-02",
            "contentJson": {
                "type": "tiptap",
                "version": 1,
                "doc": {
                    "type": "doc",
                    "content": [
                        {
                            "type": "paragraph",
                            "content": [{ "type": "text", "text": "first block" }],
                        },
                        {
                            "type": "paragraph",
                            "content": [{ "type": "text", "text": "second block" }],
                        },
                    ],
                },
            },
            "propsJson": {},
            "createdAt": "2026-06-02T08:00:00.000Z",
            "updatedAt": "2026-06-02T08:00:00.000Z",
            "deletedAt": null,
        }),
    );

    let remaining = migrate_diary(&api, &mut entries, None).unwrap();
    // No local blob was provided → nothing to write back.
    assert!(remaining.is_none());

    // Both blocks migrated under deterministic `eden-bubble-*` ids…
    let objects = ark.objects.lock().unwrap();
    let migrated: Vec<&serde_json::Value> = objects
        .values()
        .filter(|o| {
            o["propsJson"]["entry_kind"] == "bubble"
                && o["id"]
                    .as_str()
                    .unwrap_or_default()
                    .starts_with("eden-bubble-")
        })
        .collect();
    assert_eq!(migrated.len(), 2);
    // …and the legacy source is soft-deleted (Vue `deleteImportedJournalEntries`).
    assert!(!objects["journal-1"]["deletedAt"].is_null());
    drop(objects);

    // Re-running is a no-op (source deleted → gate rejects it).
    let remaining = migrate_diary(&api, &mut entries, None).unwrap();
    assert!(remaining.is_none());
    assert_eq!(ark.objects.lock().unwrap().len(), 3);
}
