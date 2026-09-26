//! Persistence side of `src/store/entryConflicts.ts` — a `ConflictStore`
//! JSON bridge (`memoria-entry-conflicts.json`) plus the in-memory
//! `createConflictRepository`. The Vue `localStorage` fallback is a host
//! detail; the bridge is the only surface here.

use serde_json::Value;

use crate::entry_conflicts::{
    conflict_for_entry, is_at_least_as_fresh, mark_entry_conflict_resolved, now, parse_snapshot,
    serialize_snapshot, update_entry_conflict_local, upsert_entry_conflict, EntryConflict,
    EntryConflictState, ENTRY_CONFLICTS_FILE,
};
use crate::model::Entry;

/// `UserDataMigrationBridge`-style JSON store for the conflict file.
pub trait ConflictStore {
    fn read_json(&self, name: &str) -> Result<Option<Value>, String>;
    fn write_json(&mut self, name: &str, value: Value) -> Result<(), String>;
}

/// `loadEntryConflicts` (bridge part only — no localStorage fallback in Rust).
pub fn load_entry_conflicts<S: ConflictStore>(store: &S) -> Vec<EntryConflict> {
    match store.read_json(ENTRY_CONFLICTS_FILE) {
        Ok(Some(value)) => parse_snapshot(&value),
        _ => Vec::new(),
    }
}

/// `persistEntryConflicts`.
pub fn persist_entry_conflicts<S: ConflictStore>(
    store: &mut S,
    conflicts: &[EntryConflict],
) -> Result<(), String> {
    store.write_json(ENTRY_CONFLICTS_FILE, serialize_snapshot(conflicts))
}

/// `mergeEntryConflictSnapshots` — the bridge/local merge from
/// `loadEntryConflicts`: per conflict id, keep the fresher snapshot.
pub fn merge_entry_conflict_snapshots(
    primary: Vec<EntryConflict>,
    fallback: Vec<EntryConflict>,
) -> Vec<EntryConflict> {
    let mut merged: Vec<EntryConflict> = primary;
    for conflict in fallback {
        match merged.iter().position(|c| c.id == conflict.id) {
            Some(i) if !is_at_least_as_fresh(&conflict, &merged[i]) => {}
            Some(i) => merged[i] = conflict,
            None => merged.push(conflict),
        }
    }
    merged
}

/// `createConflictRepository` — owns the conflict list; every mutation is
/// persisted through the optional `ConflictStore` (Vue also writes a
/// synchronous localStorage checkpoint — the store is the only surface here).
pub struct ConflictRepository<S: ConflictStore> {
    pub conflicts: Vec<EntryConflict>,
    pub store: Option<S>,
    persist_failed: bool,
}

impl<S: ConflictStore> ConflictRepository<S> {
    pub fn new(store: Option<S>) -> Self {
        Self {
            conflicts: Vec::new(),
            store,
            persist_failed: false,
        }
    }

    /// `persist` — queued writes in Vue collapse to a synchronous write here.
    pub fn persist(&mut self) {
        if let Some(store) = self.store.as_mut() {
            self.persist_failed = persist_entry_conflicts(store, &self.conflicts).is_err();
        }
    }

    pub fn persist_failed(&self) -> bool {
        self.persist_failed
    }

    /// `record` — upsert + persist; returns the live conflict for `local.id`.
    pub fn record(
        &mut self,
        local: &Entry,
        remote: Option<&Entry>,
        state: EntryConflictState,
    ) -> Option<EntryConflict> {
        self.conflicts = upsert_entry_conflict(&self.conflicts, local, remote, state, now());
        let conflict = conflict_for_entry(&self.conflicts, &local.id).cloned();
        self.persist();
        conflict
    }

    /// `resolve`.
    pub fn resolve(&mut self, conflict_id: &str, resolution: Option<&str>) {
        self.conflicts = mark_entry_conflict_resolved(
            &self.conflicts,
            conflict_id,
            resolution,
            EntryConflictState::Resolved,
            now(),
        );
        self.persist();
    }

    /// `updateLocal` — persists only when a pending snapshot actually moved.
    pub fn update_local(&mut self, entry: &Entry) {
        let next = update_entry_conflict_local(&self.conflicts, entry);
        if next != self.conflicts {
            self.conflicts = next;
            self.persist();
        }
    }
}
