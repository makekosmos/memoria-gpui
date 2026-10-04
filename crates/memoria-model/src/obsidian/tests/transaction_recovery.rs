//! Port of `tests/obsidianVaultImportTransaction.test.ts` — recovery,
//! cancellation, and partial-upsert compensation cases.

use crate::model::{Entry, NoteType};
use crate::obsidian::{
    recover_obsidian_import_journal, run_obsidian_import_transaction, ImportTransactionOptions,
    MemoryJournalStore, ObsidianImportJournal, ObsidianImportTransactionApi,
};

use super::transaction::{content_of, entry, entry_op, FakeApi};
#[test]
fn applying_journal_recovers_and_rolls_back_applied_writes() {
    let before = entry("note-a", "before");
    let after = entry("note-a", "after");
    let operation = entry_op("note-a", Some(before.clone()), after.clone());
    let mut store = MemoryJournalStore {
        journal: Some(ObsidianImportJournal {
            version: 1,
            id: "import-1".into(),
            status: "applying".into(),
            operations: vec![operation],
            applied: vec![0],
            in_flight: None,
            rolled_back: vec![],
            error: None,
        }),
        ..Default::default()
    };
    let mut api = FakeApi::default();
    api.entries.insert(after.id.clone(), after);
    recover_obsidian_import_journal(&mut api, &mut store).unwrap();
    assert_eq!(
        content_of(api.entries.get("note-a").unwrap()),
        content_of(&before)
    );
    assert_eq!(store.journal.unwrap().status, "rolled-back");
}

#[test]
fn cancellation_compensates_writes_already_applied() {
    let mut api = FakeApi::default();
    let checks = std::cell::Cell::new(0usize);
    let operations = vec![
        entry_op("note-a", None, entry("note-a", "new")),
        entry_op("note-b", None, entry("note-b", "new")),
    ];
    let mut store = MemoryJournalStore::default();
    let result = run_obsidian_import_transaction(
        operations,
        &mut api,
        &mut store,
        &mut ImportTransactionOptions {
            is_cancelled: Some(Box::new(move || {
                checks.set(checks.get() + 1);
                checks.get() > 1
            })),
            ..Default::default()
        },
    );
    assert!(result.unwrap_err().contains("cancelled"));
    assert!(api.entries.is_empty());
}

#[test]
fn post_upsert_failure_rolls_back_the_landed_write() {
    let before = entry("note-a", "before");
    let mut api = FakeApi {
        fail_after_upsert: Some("note-a".into()),
        ..Default::default()
    };
    api.entries.insert(before.id.clone(), before.clone());
    let mut store = MemoryJournalStore::default();
    let result = run_obsidian_import_transaction(
        vec![entry_op(
            "note-a",
            Some(before.clone()),
            entry("note-a", "after"),
        )],
        &mut api,
        &mut store,
        &mut ImportTransactionOptions::default(),
    );
    assert!(result.unwrap_err().contains("after-upsert"));
    assert_eq!(
        content_of(api.entries.get("note-a").unwrap()),
        content_of(&before)
    );
    assert!(store.journal.unwrap().in_flight.is_none());
}

#[test]
fn link_sync_partial_upsert_restores_the_original_note() {
    let before = entry("note-a", "before");
    let first_pass = entry("note-a", "first-pass");
    let linked = entry("note-a", "linked");
    // Second save fails after the upsert landed (`link-sync failed after
    // upsert`).
    let mut api = FakeFailOnNthSave {
        inner: FakeApi::default(),
        saves: 0,
    };
    api.inner.entries.insert(before.id.clone(), before.clone());
    let mut store = MemoryJournalStore::default();
    let result = run_obsidian_import_transaction(
        vec![
            entry_op("note-a", Some(before.clone()), first_pass),
            entry_op("note-a", Some(entry("note-a", "first-pass")), linked),
        ],
        &mut api,
        &mut store,
        &mut ImportTransactionOptions::default(),
    );
    assert!(result.unwrap_err().contains("link-sync"));
    assert_eq!(
        content_of(api.inner.entries.get("note-a").unwrap()),
        content_of(&before)
    );
}

/// Fake api that fails the 2nd `save_entry` after writing.
struct FakeFailOnNthSave {
    inner: FakeApi,
    saves: usize,
}

impl ObsidianImportTransactionApi for FakeFailOnNthSave {
    fn save_note_type(&mut self, v: &NoteType) -> Result<(), String> {
        self.inner.save_note_type(v)
    }
    fn delete_note_type(&mut self, id: &str) -> Result<(), String> {
        self.inner.delete_note_type(id)
    }
    fn save_entry(&mut self, v: &Entry) -> Result<(), String> {
        self.saves += 1;
        self.inner.entries.insert(v.id.clone(), v.clone());
        if self.saves == 2 {
            return Err("link-sync failed after upsert".into());
        }
        Ok(())
    }
    fn delete_entry(&mut self, id: &str) -> Result<(), String> {
        self.inner.delete_entry(id)
    }
}

#[test]
fn recovery_compensates_in_flight_write_after_crash() {
    let before = entry("note-a", "before");
    let after = entry("note-a", "after");
    let mut store = MemoryJournalStore {
        journal: Some(ObsidianImportJournal {
            version: 1,
            id: "import-crashed".into(),
            status: "applying".into(),
            operations: vec![entry_op("note-a", Some(before.clone()), after.clone())],
            applied: vec![],
            in_flight: Some(0),
            rolled_back: vec![],
            error: None,
        }),
        ..Default::default()
    };
    let mut api = FakeApi::default();
    api.entries.insert(after.id.clone(), after);
    recover_obsidian_import_journal(&mut api, &mut store).unwrap();
    assert_eq!(
        content_of(api.entries.get("note-a").unwrap()),
        content_of(&before)
    );
    assert_eq!(store.journal.unwrap().status, "rolled-back");
}

#[test]
fn refresh_failure_backs_up_updated_notes_links_and_images() {
    let old_note = entry("note-a", "old");
    let old_image = entry("image-a", "old-image");
    let mut api = FakeApi::default();
    api.entries.insert(old_note.id.clone(), old_note.clone());
    api.entries.insert(old_image.id.clone(), old_image.clone());
    let operations = vec![
        entry_op("note-a", Some(old_note.clone()), entry("note-a", "new")),
        entry_op(
            "note-a",
            Some(entry("note-a", "new")),
            entry("note-a", "linked"),
        ),
        entry_op(
            "image-a",
            Some(old_image.clone()),
            entry("image-a", "new-image"),
        ),
    ];
    let mut store = MemoryJournalStore::default();
    let result = run_obsidian_import_transaction(
        operations,
        &mut api,
        &mut store,
        &mut ImportTransactionOptions {
            on_commit: Some(Box::new(|| Err("refresh failed".into()))),
            ..Default::default()
        },
    );
    assert!(result.unwrap_err().contains("refresh failed"));
    assert_eq!(
        content_of(api.entries.get("note-a").unwrap()),
        content_of(&old_note)
    );
    assert_eq!(
        content_of(api.entries.get("image-a").unwrap()),
        content_of(&old_image)
    );
    assert_eq!(store.journal.unwrap().status, "rolled-back");
}
