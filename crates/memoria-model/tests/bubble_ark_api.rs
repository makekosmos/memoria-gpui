//! Port of `tests/bubbleArkApi.test.ts` — `BubbleApi` over a map-backed
//! `ArkBridge` fake that mirrors the Vue `fakeArk` (objects/links maps,
//! `reorderJsonOnRead` for the key-order-insensitive read-back case).

mod common;

use common::fake_ark::{api_for, FakeArk};
use memoria_model::diary::BubbleKind;
use serde_json::json;

#[test]
fn persists_canonical_contract_and_excludes_unmarked_journals() {
    let ark = FakeArk::new(false);
    ark.objects.lock().unwrap().insert(
        "ordinary".into(),
        json!({
            "id": "ordinary", "typeId": "com.kosmos.note", "title": "ordinary",
            "contentJson": {}, "propsJson": {},
            "createdAt": "2026-07-01T00:00:00.000Z",
            "updatedAt": "2026-07-01T00:00:00.000Z", "deletedAt": null,
        }),
    );
    let api = api_for(&ark);

    let id = api
        .create_bubble("Текст #Tag #tag", BubbleKind::Plain, None, None)
        .unwrap();
    let objects = ark.objects.lock().unwrap();
    let object = &objects[&id];
    assert_eq!(object["typeId"], "com.kosmos.note");
    assert_eq!(object["propsJson"]["entry_kind"], "bubble");
    assert_eq!(object["propsJson"]["bubble_kind"], "plain");
    assert_eq!(object["propsJson"]["tags"], json!(["tag"]));
    assert!(object["deletedAt"].is_null());
    drop(objects);
    assert_eq!(api.list_bubbles().unwrap().len(), 1);
}

#[test]
fn updates_in_place_preserving_occurrence_and_unrelated_props() {
    let ark = FakeArk::new(false);
    let api = api_for(&ark);
    let id = api
        .create_bubble("До", BubbleKind::Plain, None, None)
        .unwrap();
    {
        let mut objects = ark.objects.lock().unwrap();
        objects.get_mut(&id).unwrap()["propsJson"]["unrelated"] = json!(42);
    }
    let created_at = ark.objects.lock().unwrap()[&id]["createdAt"].clone();

    api.update_bubble(
        &id,
        memoria_model::store::BubblePatch {
            input: Some("После #новое".into()),
            kind: Some(BubbleKind::Idea),
        },
    )
    .unwrap();

    let objects = ark.objects.lock().unwrap();
    let after = &objects[&id];
    assert_eq!(after["createdAt"], created_at);
    assert_eq!(after["propsJson"]["bubble_kind"], "idea");
    assert_eq!(after["propsJson"]["tags"], json!(["новое"]));
    assert_eq!(after["propsJson"]["unrelated"], json!(42));
}

#[test]
fn kind_only_update_preserves_unknown_tiptap_nodes() {
    let ark = FakeArk::new(false);
    let api = api_for(&ark);
    let doc = json!({
        "type": "doc",
        "content": [
            { "type": "paragraph", "content": [{ "type": "text", "text": "С узлом" }] },
            { "type": "kanbanBoard", "attrs": { "columns": 3 },
              "content": [{ "type": "text", "text": "колонка" }] },
        ],
    });
    let id = api
        .create_bubble("С узлом", BubbleKind::Plain, None, Some(doc.clone()))
        .unwrap();
    let before = ark.objects.lock().unwrap()[&id]["contentJson"].clone();
    assert_eq!(
        before,
        memoria_model::content::write_entry_tiptap_doc(doc.clone())
    );

    api.update_bubble(
        &id,
        memoria_model::store::BubblePatch {
            input: None,
            kind: Some(BubbleKind::Highlight),
        },
    )
    .unwrap();

    let objects = ark.objects.lock().unwrap();
    let after = &objects[&id];
    assert_eq!(after["propsJson"]["bubble_kind"], "highlight");
    assert_eq!(
        after["contentJson"], before,
        "unknown tiptap nodes must survive a kind-only update"
    );
}

#[test]
fn reconstructs_replies_and_preserves_children_when_deleting_root() {
    let ark = FakeArk::new(false);
    let api = api_for(&ark);
    let root = api
        .create_bubble("Корень", BubbleKind::Plain, None, None)
        .unwrap();
    let child = api
        .create_bubble("Ответ", BubbleKind::Plain, Some(&root), None)
        .unwrap();
    let sibling = api
        .create_bubble("Второй", BubbleKind::Plain, Some(&root), None)
        .unwrap();

    let list = api.list_bubbles().unwrap();
    assert_eq!(
        list.iter()
            .find(|b| b.id == child)
            .unwrap()
            .parent_id
            .as_deref(),
        Some(root.as_str())
    );
    assert_eq!(ark.links.lock().unwrap().len(), 2);

    api.delete_bubble(&root).unwrap();
    assert_eq!(ark.links.lock().unwrap().len(), 0);
    let reloaded = api.list_bubbles().unwrap();
    let mut ids: Vec<&str> = reloaded.iter().map(|b| b.id.as_str()).collect();
    ids.sort();
    let mut expected = [child.as_str(), sibling.as_str()];
    expected.sort();
    assert_eq!(ids, expected);
    assert!(reloaded.iter().all(|b| b.parent_id.is_none()));
    assert!(ark
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|c| c == "delete_object"));
}

#[test]
fn deleting_a_child_keeps_root_and_sibling_intact() {
    let ark = FakeArk::new(false);
    let api = api_for(&ark);
    let root = api
        .create_bubble("Корень", BubbleKind::Plain, None, None)
        .unwrap();
    let child = api
        .create_bubble("Ответ", BubbleKind::Plain, Some(&root), None)
        .unwrap();
    let sibling = api
        .create_bubble("Второй", BubbleKind::Plain, Some(&root), None)
        .unwrap();
    api.delete_bubble(&child).unwrap();

    let reloaded = api.list_bubbles().unwrap();
    assert_eq!(
        reloaded.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(),
        [root.as_str(), sibling.as_str()]
    );
    assert_eq!(reloaded[1].parent_id.as_deref(), Some(root.as_str()));
}
