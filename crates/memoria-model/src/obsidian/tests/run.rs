//! `BridgeObsidianApi` + `syncRelatedLinks` rollback test — ports the final
//! case of `obsidianVaultImportTransaction.test.ts`.

use serde_json::json;

use crate::model::Entry;
use crate::obsidian::{
    run_obsidian_import_transaction, BridgeObsidianApi, ImportTransactionOptions,
    MemoryJournalStore, ObsidianImportOperation,
};
use crate::store::EntryApi;

use super::fake_ark::{summary_fixture, FakeArk};

#[test]
fn summary_origin_entry_restored_when_link_sync_fails() {
    let (object, link) = summary_fixture();
    let ark = FakeArk::default();
    ark.put_object(object);
    ark.state
        .borrow_mut()
        .links
        .insert("note-a:related:target".into(), link);
    ark.state.borrow_mut().fail_next_upsert_link = true;

    let summary = EntryApi::new(ark.clone())
        .load_entry("note-a", false)
        .unwrap()
        .unwrap();
    assert_eq!(summary.content_loaded, Some(true));

    let mut after = Entry {
        id: "note-a".into(),
        title: "note-a".into(),
        content_json: serde_json::to_string(&json!({
            "type": "markdown", "version": 1, "text": "new"
        }))
        .unwrap(),
        type_id: Some("note_obj".into()),
        header_props_json: Some("{\"related_notes\":[\"target\"]}".into()),
        schema_version: Some(1),
        ..Default::default()
    };
    after.created_at = summary.created_at;

    let mut api = BridgeObsidianApi {
        bridge: ark.clone(),
    };
    let mut store = MemoryJournalStore::default();
    let result = run_obsidian_import_transaction(
        vec![ObsidianImportOperation {
            kind: "entry".into(),
            id: "note-a".into(),
            before: serde_json::to_value(&summary).ok(),
            after: serde_json::to_value(&after).unwrap(),
        }],
        &mut api,
        &mut store,
        &mut ImportTransactionOptions::default(),
    );
    assert!(result.unwrap_err().contains("syncRelatedLinks"));

    let stored = ark.object("note-a");
    let expected = crate::mapping::map_entry_to_ark_object(&summary);
    assert_eq!(stored["contentJson"], expected.content_json);
    assert_eq!(stored["title"], json!("Note A"));
}
