//! End-to-end `import_obsidian_vault_dir` / `export_obsidian_vault_dir`
//! coverage against the in-memory `FakeArk` Engine.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::model::{Entry, NoteType};
use crate::obsidian::{
    export_obsidian_vault_dir, import_obsidian_vault_dir, BridgeObsidianApi,
    ObsidianImportTransactionApi as _,
};

use super::fake_ark::FakeArk;

#[test]
fn import_vault_dir_writes_entries_and_images_through_engine() {
    let ark = FakeArk::default();
    ark.state.borrow_mut().vault_files = vec![json!({
        "relativePath": "notes/A.md",
        "name": "A.md",
        "content": "Body [[B]]",
    })];
    ark.state.borrow_mut().vault_images = vec![json!({
        "relativePath": "assets/cover.png",
        "name": "cover.png",
        "fileUrl": "kosmos-local-image://file/C%3A%2Fvault%2Fassets%2Fcover.png",
        "mimeType": "image/png",
        "sizeBytes": 10,
        "width": 5,
        "height": 5,
    })];

    let outcome = import_obsidian_vault_dir(&ark, "/vault").unwrap();
    assert_eq!(outcome.created.len(), 2);

    let objects = ark.state.borrow().objects.clone();
    let note = objects
        .values()
        .find(|o| {
            o.get("propsJson")
                .and_then(|p| p.get("extensions"))
                .and_then(|e| e.get("memoria_type_id"))
                .and_then(Value::as_str)
                == Some("note_obj")
        })
        .expect("note object written");
    let props = note.get("propsJson").cloned().unwrap_or(Value::Null);
    assert_eq!(
        props["extensions"]["source_relative_path"],
        json!("notes/A.md")
    );
    let image = objects
        .values()
        .find(|o| {
            o.get("propsJson")
                .and_then(|p| p.get("extensions"))
                .and_then(|e| e.get("memoria_type_id"))
                .and_then(Value::as_str)
                == Some("image_obj")
        })
        .expect("image object written");
    assert_eq!(
        image["propsJson"]["extensions"]["file_name"],
        json!("cover.png")
    );
    assert_eq!(ark.state.borrow().closed_roots, vec!["root-1".to_string()]);
}

#[test]
fn mid_import_failure_leaves_no_partial_objects() {
    let ark = FakeArk::default();
    ark.state.borrow_mut().vault_files = vec![
        json!({"relativePath": "a.md", "name": "a.md", "content": "A"}),
        json!({"relativePath": "b.md", "name": "b.md", "content": "B"}),
    ];
    // Fail the note-type upsert — the transaction must compensate.
    ark.state.borrow_mut().fail_object_ids = vec!["memoria:type:image_obj".into()];

    let result = import_obsidian_vault_dir(&ark, "/vault");
    assert!(result.is_err());
    let objects = ark.state.borrow().objects.clone();
    assert!(
        objects.values().all(|o| {
            o.get("propsJson")
                .and_then(|p| p.get("extensions"))
                .and_then(|e| e.get("memoria_type_id"))
                .and_then(Value::as_str)
                != Some("note_obj")
        }),
        "rolled back — no note objects remain"
    );
}

#[test]
fn export_vault_dir_registers_and_writes_through_engine() {
    let ark = FakeArk::default();
    let entry = Entry {
        id: "entry-1".into(),
        title: "My Note".into(),
        type_id: Some("note_obj".into()),
        content_json: "{}".into(),
        header_props_json: Some("{}".into()),
        schema_version: Some(1),
        ..Default::default()
    };
    let note_type = NoteType {
        id: "note_obj".into(),
        name: "Note".into(),
        slug: "note".into(),
        schema_json: "{\"fields\":[]}".into(),
        header_template_json: "{}".into(),
        ..Default::default()
    };
    let bodies = HashMap::from([("entry-1".to_string(), "Hello".to_string())]);
    let titles = HashMap::new();
    let count =
        export_obsidian_vault_dir(&ark, vec![entry], &[note_type], &bodies, &titles, "/export")
            .unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        ark.state.borrow().registered_dirs,
        vec!["/export".to_string()]
    );
    let exported = &ark.state.borrow().exported_files;
    assert_eq!(exported.len(), 1);
    assert_eq!(exported[0]["relativePath"], json!("My-Note.md"));
    assert!(exported[0]["content"].as_str().unwrap().contains("Hello"));
    assert_eq!(ark.state.borrow().closed_roots, vec!["root-1".to_string()]);
}

#[test]
fn related_links_sync_via_object_links() {
    // `syncRelatedLinks` second pass: stale links delete, new targets upsert.
    let ark = FakeArk::default();
    ark.put_object(json!({
        "id": "note-a",
        "typeId": "com.kosmos.note",
        "title": "A",
        "contentJson": { "type": "markdown", "version": 1, "text": "" },
        "propsJson": {},
        "createdAt": "2026-01-01T00:00:00.000Z",
        "updatedAt": "2026-01-01T00:00:00.000Z",
        "deletedAt": null,
    }));
    ark.state.borrow_mut().links.insert(
        "note-a:related:stale".into(),
        json!({
            "id": "note-a:related:stale",
            "sourceObjectId": "note-a",
            "targetObjectId": "stale",
            "linkType": "related",
            "createdAt": "2026-01-01T00:00:00.000Z",
        }),
    );
    let entry = Entry {
        id: "note-a".into(),
        title: "A".into(),
        content_json: "{}".into(),
        type_id: Some("note_obj".into()),
        header_props_json: Some("{\"related_notes\":[\"keep\",\"new\"]}".into()),
        updated_at: 1000,
        ..Default::default()
    };
    let mut api = BridgeObsidianApi {
        bridge: ark.clone(),
    };
    api.save_entry(&entry).unwrap();
    let links = ark.state.borrow().links.clone();
    assert!(!links.contains_key("note-a:related:stale"));
    assert_eq!(
        links["note-a:related:keep"]["targetObjectId"],
        json!("keep")
    );
    assert_eq!(links["note-a:related:new"]["targetObjectId"], json!("new"));
}
