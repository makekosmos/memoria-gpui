//! Port of `tests/entryConflicts.test.ts` — durable entry conflicts.
//! The Vue `localStorage` fallback tests map onto `ConflictStore` bridges:
//! `merge_entry_conflict_snapshots` covers the bridge/local merge cases.

use memoria_model::conflict_store::{
    load_entry_conflicts, persist_entry_conflicts, ConflictRepository, ConflictStore,
};
use memoria_model::entry_conflicts::{
    conflict_for_entry, mark_entry_conflict_resolved, unresolved_entry_conflicts,
    update_entry_conflict_local, upsert_entry_conflict, EntryConflictState, ENTRY_CONFLICTS_FILE,
};
use memoria_model::model::Entry;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

fn entry(id: &str, text: &str, updated_at: i64) -> Entry {
    Entry {
        id: id.into(),
        title: "Note".into(),
        content_json: serde_json::to_string(&serde_json::json!({
            "type": "markdown", "version": 1, "text": text,
        }))
        .unwrap(),
        created_at: 1,
        updated_at,
        type_id: Some("note_obj".into()),
        header_props_json: Some("{}".into()),
        schema_version: Some(1),
        ..Default::default()
    }
}

/// In-memory `ConflictStore` (`window.kepler.userData` equivalent).
#[derive(Clone, Default)]
struct MemStore(Rc<RefCell<HashMap<String, Value>>>);

impl ConflictStore for MemStore {
    fn read_json(&self, name: &str) -> Result<Option<Value>, String> {
        Ok(self.0.borrow().get(name).cloned())
    }
    fn write_json(&mut self, name: &str, value: Value) -> Result<(), String> {
        self.0.borrow_mut().insert(name.to_string(), value);
        Ok(())
    }
}

#[test]
fn retains_local_draft_and_newest_remote_revision() {
    let local = entry("n1", "local", 20);
    let remote = entry("n1", "remote", 21);
    let mut conflicts = upsert_entry_conflict(
        &[],
        &local,
        Some(&remote),
        EntryConflictState::RemoteUpdated,
        30,
    );
    conflicts = upsert_entry_conflict(
        &conflicts,
        &entry("n1", "newer local", 22),
        Some(&entry("n1", "old", 19)),
        EntryConflictState::RemoteUpdated,
        31,
    );
    assert_eq!(conflicts.len(), 1);
    assert!(conflicts[0].local.content_json.contains("local"));
    assert!(conflicts[0]
        .remote
        .as_ref()
        .unwrap()
        .content_json
        .contains("remote"));
    assert_eq!(conflicts[0].remote_revision, Some(21));
}

#[test]
fn remote_delete_is_an_explicit_unresolved_tombstone() {
    let local = entry("n1", "draft", 20);
    let conflicts = upsert_entry_conflict(&[], &local, None, EntryConflictState::RemoteDeleted, 30);
    assert_eq!(conflicts[0].state, EntryConflictState::RemoteDeleted);
    assert!(conflict_for_entry(&conflicts, "n1")
        .unwrap()
        .local
        .content_json
        .contains("draft"));
    assert_eq!(unresolved_entry_conflicts(&conflicts).len(), 1);
}

#[test]
fn repository_records_and_resolves_in_memory() {
    let mut repository = ConflictRepository::<MemStore>::new(None);
    let conflict = repository
        .record(
            &entry("n1", "draft", 10),
            Some(&entry("n1", "remote", 11)),
            EntryConflictState::StaleSave,
        )
        .unwrap();
    assert_eq!(repository.conflicts[0].id, conflict.id);
    repository.resolve(&conflict.id, Some("keep-local-copy"));
    assert_eq!(repository.conflicts[0].state, EntryConflictState::Resolved);
    assert_eq!(unresolved_entry_conflicts(&repository.conflicts).len(), 0);
}

#[test]
fn local_snapshot_follows_subsequent_draft_edits() {
    let initial = upsert_entry_conflict(
        &[],
        &entry("n1", "first", 20),
        Some(&entry("n1", "remote", 21)),
        EntryConflictState::RemoteUpdated,
        30,
    );
    let updated = update_entry_conflict_local(&initial, &entry("n1", "second", 22));
    assert!(updated[0].local.content_json.contains("second"));
    assert_eq!(updated[0].local_revision, 22);
    assert!(updated[0]
        .remote
        .as_ref()
        .unwrap()
        .content_json
        .contains("remote"));
}

#[test]
fn resolved_records_are_not_reused_for_later_conflicts() {
    let local = entry("n1", "draft", 10);
    let initial = upsert_entry_conflict(&[], &local, None, EntryConflictState::RemoteDeleted, 30);
    let resolved = mark_entry_conflict_resolved(
        &initial,
        &initial[0].id,
        Some("accept-remote"),
        EntryConflictState::Resolved,
        31,
    );
    let next = upsert_entry_conflict(
        &resolved,
        &entry("n1", "new draft", 32),
        Some(&entry("n1", "new remote", 33)),
        EntryConflictState::RemoteUpdated,
        34,
    );
    assert_eq!(next.len(), 2);
    assert_eq!(
        conflict_for_entry(&next, "n1").unwrap().state,
        EntryConflictState::RemoteUpdated
    );
}

#[test]
fn round_trips_unresolved_draft_across_restart() {
    let store = MemStore::default();
    let pending = upsert_entry_conflict(
        &[],
        &entry("n1", "unsaved draft", 20),
        None,
        EntryConflictState::RemoteDeleted,
        30,
    );
    persist_entry_conflicts(&mut store.clone(), &pending).unwrap();
    assert!(store.0.borrow().contains_key(ENTRY_CONFLICTS_FILE));

    let loaded = load_entry_conflicts(&store);
    assert!(loaded[0].local.content_json.contains("unsaved draft"));
    assert_eq!(loaded[0].state, EntryConflictState::RemoteDeleted);
}

#[test]
fn malformed_local_checkpoint_keeps_valid_bridge_snapshot() {
    let bridge = upsert_entry_conflict(
        &[],
        &entry("n1", "bridge draft", 20),
        None,
        EntryConflictState::RemoteDeleted,
        30,
    );
    // `localStorage` was corrupt (`{ malformed`) → contributes nothing; the
    // bridge snapshot must survive the merge.
    let loaded =
        memoria_model::conflict_store::merge_entry_conflict_snapshots(bridge.clone(), vec![]);
    assert!(loaded[0].local.content_json.contains("bridge draft"));

    // And a fresher local conflict wins over a stale bridge one.
    let mut local = bridge.clone();
    local[0].detected_at = 99;
    let merged =
        memoria_model::conflict_store::merge_entry_conflict_snapshots(bridge, local.clone());
    assert_eq!(merged[0].detected_at, 99);
}

#[test]
fn resolve_can_mark_a_conflict_merged() {
    let mut repository = ConflictRepository::new(Some(MemStore::default()));
    let conflict = repository
        .record(
            &entry("n1", "draft", 10),
            None,
            EntryConflictState::StaleSave,
        )
        .unwrap();
    repository.resolve_with_state(
        &conflict.id,
        Some("merged-into-remote"),
        EntryConflictState::Merged,
    );
    assert_eq!(repository.conflicts[0].state, EntryConflictState::Merged);
    assert_eq!(
        repository.conflicts[0].resolution.as_deref(),
        Some("merged-into-remote")
    );
}

#[test]
fn snapshot_load_accepts_integral_float_numbers() {
    // JS `typeof === "number"` — `version: 1.0`, `localRevision: 10.0` are
    // numbers; only non-integral values and non-numbers are rejected.
    let store = MemStore::default();
    let local = entry("n1", "draft", 10);
    store.0.borrow_mut().insert(
        ENTRY_CONFLICTS_FILE.into(),
        serde_json::json!({
            "version": 1.0,
            "conflicts": [{
                "id": "c1",
                "entryId": "n1",
                "state": "stale-save",
                "local": serde_json::to_value(&local).unwrap(),
                "remote": null,
                "localRevision": 10.0,
                "remoteRevision": null,
                "detectedAt": 30.0,
            }],
        }),
    );
    let loaded = load_entry_conflicts(&store);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].local_revision, 10);
    assert_eq!(loaded[0].detected_at, 30);
}
