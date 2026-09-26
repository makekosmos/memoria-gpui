//! `tests/bubbleArkApi.test.ts` migration half — `migrateBubble` id
//! determinism, read-back verification (key-order-insensitive), ambiguous
//! occurrence rejection.

mod common;

use common::fake_ark::{api_for, FakeArk};
use memoria_gpui::diary::{BubbleKind, BubbleTimelineNode};
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
