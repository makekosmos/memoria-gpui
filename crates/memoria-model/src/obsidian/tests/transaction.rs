//! Port of `tests/obsidianVaultImportTransaction.test.ts` — journaled apply,
//! compensation on failure/cancel, and restart recovery.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::model::{Entry, NoteType};
use crate::obsidian::{
    run_obsidian_import_transaction, ImportTransactionOptions, MemoryJournalStore,
    ObsidianImportJournal, ObsidianImportOperation, ObsidianImportTransactionApi,
};

pub(super) fn entry(id: &str, text: &str) -> Entry {
    Entry {
        id: id.into(),
        title: id.into(),
        content_json: serde_json::to_string(&json!({
            "type": "markdown",
            "version": 1,
            "text": text,
        }))
        .unwrap(),
        created_at: 1,
        updated_at: 1,
        type_id: Some("note_obj".into()),
        header_props_json: Some("{}".into()),
        schema_version: Some(1),
        ..Default::default()
    }
}

pub(super) fn note_type(id: &str) -> NoteType {
    NoteType {
        id: id.into(),
        name: id.into(),
        slug: id.into(),
        schema_json: "{\"fields\":[]}".into(),
        header_template_json: "{}".into(),
        created_at: 1,
        updated_at: 1,
        ..Default::default()
    }
}

/// `harness` — the fake Engine api (entries/types maps + injected failures).
#[derive(Default)]
pub(super) struct FakeApi {
    pub(super) entries: HashMap<String, Entry>,
    pub(super) types: HashMap<String, NoteType>,
    pub(super) fail_on: Option<String>,
    /// Upsert lands, then the call reports failure (`partialFailureHarness`).
    pub(super) fail_after_upsert: Option<String>,
    pub(super) upsert_failed: bool,
    pub(super) deletes: Vec<String>,
}

impl ObsidianImportTransactionApi for FakeApi {
    fn save_note_type(&mut self, value: &NoteType) -> Result<(), String> {
        if self.fail_on.as_deref() == Some(value.id.as_str()) {
            return Err("injected type failure".into());
        }
        self.types.insert(value.id.clone(), value.clone());
        Ok(())
    }

    fn delete_note_type(&mut self, id: &str) -> Result<(), String> {
        self.types.remove(id);
        Ok(())
    }

    fn save_entry(&mut self, value: &Entry) -> Result<(), String> {
        if self.fail_on.as_deref() == Some(value.id.as_str()) {
            return Err("injected entry failure".into());
        }
        self.entries.insert(value.id.clone(), value.clone());
        if self.fail_after_upsert.as_deref() == Some(value.id.as_str()) && !self.upsert_failed {
            self.upsert_failed = true;
            return Err("injected after-upsert failure".into());
        }
        Ok(())
    }

    fn delete_entry(&mut self, id: &str) -> Result<(), String> {
        self.deletes.push(id.to_string());
        self.entries.remove(id);
        Ok(())
    }
}

pub(super) fn op(
    kind: &str,
    id: &str,
    before: Option<Value>,
    after: Value,
) -> ObsidianImportOperation {
    ObsidianImportOperation {
        kind: kind.into(),
        id: id.into(),
        before,
        after,
    }
}

pub(super) fn entry_op(id: &str, before: Option<Entry>, after: Entry) -> ObsidianImportOperation {
    op(
        "entry",
        id,
        before.map(|e| serde_json::to_value(e).unwrap()),
        serde_json::to_value(after).unwrap(),
    )
}

pub(super) fn type_op(id: &str) -> ObsidianImportOperation {
    op(
        "note-type",
        id,
        None,
        serde_json::to_value(note_type(id)).unwrap(),
    )
}

pub(super) fn content_of(entry: &Entry) -> Value {
    serde_json::from_str(&entry.content_json).unwrap()
}

#[test]
fn journal_roundtrips_through_json_cleanly() {
    let journal = ObsidianImportJournal {
        version: 1,
        id: "import-reactive".into(),
        status: "applying".into(),
        operations: vec![entry_op(
            "note-a",
            Some(entry("note-a", "before")),
            entry("note-a", "after"),
        )],
        applied: vec![0],
        in_flight: None,
        rolled_back: vec![],
        error: None,
    };
    let value = serde_json::to_value(&journal).unwrap();
    let parsed = crate::obsidian::parse_journal(&value).unwrap();
    assert_eq!(parsed, journal);
}

#[test]
fn apply_and_rollback_values_are_clone_safe() {
    let mut api = FakeApi {
        fail_on: Some("note-b".into()),
        ..Default::default()
    };
    let operations = vec![
        entry_op(
            "note-a",
            Some(entry("note-a", "old")),
            entry("note-a", "new"),
        ),
        entry_op("note-b", None, entry("note-b", "injected failure")),
    ];
    let mut store = MemoryJournalStore::default();
    let result = run_obsidian_import_transaction(
        operations,
        &mut api,
        &mut store,
        &mut ImportTransactionOptions::default(),
    );
    assert!(result.unwrap_err().contains("injected entry failure"));
    assert_eq!(
        content_of(api.entries.get("note-a").unwrap()),
        content_of(&entry("note-a", "old"))
    );
    assert!(!api.entries.contains_key("note-b"));
}

#[test]
fn type_failure_rolls_back_and_leaves_no_partial_writes() {
    let mut api = FakeApi {
        fail_on: Some("image_obj".into()),
        ..Default::default()
    };
    let operations = vec![
        type_op("image_obj"),
        entry_op("note-a", None, entry("note-a", "new")),
    ];
    let mut store = MemoryJournalStore::default();
    let result = run_obsidian_import_transaction(
        operations,
        &mut api,
        &mut store,
        &mut ImportTransactionOptions::default(),
    );
    assert!(result.is_err());
    assert!(api.entries.is_empty());
    assert!(api.types.is_empty());
}

#[test]
fn in_flight_checkpoint_failure_stays_below_the_ark_boundary() {
    let mut api = FakeApi::default();
    let mut store = MemoryJournalStore {
        fail_on_save: Some(2),
        ..Default::default()
    };
    let result = run_obsidian_import_transaction(
        vec![entry_op("note-a", None, entry("note-a", "new"))],
        &mut api,
        &mut store,
        &mut ImportTransactionOptions::default(),
    );
    assert!(result.unwrap_err().contains("journal checkpoint"));
    assert!(api.entries.is_empty());
    let journal = store.journal.unwrap();
    assert_eq!(journal.status, "rolled-back");
    assert!(journal.in_flight.is_none());
}

#[test]
fn stale_in_flight_checkpoint_is_not_compensated() {
    let mut api = FakeApi::default();
    let mut store = MemoryJournalStore {
        write_through_fail_on_save: Some(2),
        ..Default::default()
    };
    let result = run_obsidian_import_transaction(
        vec![entry_op("note-a", None, entry("note-a", "new"))],
        &mut api,
        &mut store,
        &mut ImportTransactionOptions::default(),
    );
    assert!(result.unwrap_err().contains("journal checkpoint"));
    assert!(api.deletes.is_empty());
    assert!(api.entries.is_empty());
    let journal = store.journal.unwrap();
    assert_eq!(journal.status, "rolled-back");
    assert!(journal.in_flight.is_none());
}

#[test]
fn rollback_never_deletes_the_later_unstarted_operation() {
    let mut api = FakeApi::default();
    let mut store = MemoryJournalStore {
        fail_on_save: Some(4),
        ..Default::default()
    };
    let result = run_obsidian_import_transaction(
        vec![
            entry_op("note-a", None, entry("note-a", "new")),
            entry_op("note-b", None, entry("note-b", "new")),
        ],
        &mut api,
        &mut store,
        &mut ImportTransactionOptions::default(),
    );
    assert!(result.unwrap_err().contains("journal checkpoint"));
    assert_eq!(api.deletes, ["note-a"]);
    assert!(api.entries.is_empty());
}

#[test]
fn import_guards_run_immediately_before_each_ark_write() {
    let mut api = FakeApi::default();
    let mut store = MemoryJournalStore::default();
    let mut checked = 0usize;
    let result = run_obsidian_import_transaction(
        vec![entry_op("note-a", None, entry("note-a", "new"))],
        &mut api,
        &mut store,
        &mut ImportTransactionOptions {
            before_operation: Some(Box::new(|_, index| {
                checked = index + 1;
                Err("import guard blocked".into())
            })),
            ..Default::default()
        },
    );
    assert!(result.unwrap_err().contains("import guard blocked"));
    assert_eq!(checked, 1);
    assert!(api.entries.is_empty());
}
