//! Persistence side of `src/store/entryConflicts.ts` — a `ConflictStore`
//! JSON bridge (`memoria-entry-conflicts.json`) plus the in-memory
//! `createConflictRepository`. The Vue `localStorage` fallback is a host
//! detail; the bridge is the only surface here.

use serde_json::Value;

use crate::entry_conflicts::{
    conflict_for_entry, is_at_least_as_fresh, mark_entry_conflict_resolved, now,
    upsert_entry_conflict, EntryConflict, EntryConflictState, ENTRY_CONFLICTS_FILE,
};

use serde_json::Map;

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
    /// Vue types `state` as `Exclude<…, "resolved"|"merged">` — open states
    /// only; assert rather than silently persisting a closed conflict.
    pub fn record(
        &mut self,
        local: &Entry,
        remote: Option<&Entry>,
        state: EntryConflictState,
    ) -> Option<EntryConflict> {
        debug_assert!(!state.is_closed(), "record() needs an open state");
        self.conflicts = upsert_entry_conflict(&self.conflicts, local, remote, state, now());
        let conflict = conflict_for_entry(&self.conflicts, &local.id).cloned();
        self.persist();
        conflict
    }

    /// `resolve` — `state` stays a parameter like Vue's
    /// `resolve(conflictId, resolution, state = "resolved")`: the merge flow
    /// closes with `EntryConflictState::Merged`.
    pub fn resolve(&mut self, conflict_id: &str, resolution: Option<&str>) {
        self.resolve_with_state(conflict_id, resolution, EntryConflictState::Resolved);
    }

    pub fn resolve_with_state(
        &mut self,
        conflict_id: &str,
        resolution: Option<&str>,
        state: EntryConflictState,
    ) {
        self.conflicts =
            mark_entry_conflict_resolved(&self.conflicts, conflict_id, resolution, state, now());
        self.persist();
    }
}

/// JS `typeof === "number"` accepts integral floats (`1.0`); `i64` does
/// not. Foreign/hand-edited snapshots can carry them — coerce before parse.
fn num_i64(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| {
        value
            .as_f64()
            .and_then(|f| (f.fract() == 0.0).then_some(f as i64))
    })
}

fn sanitize_entry_numbers(value: &Value) -> Value {
    let Value::Object(map) = value else {
        return value.clone();
    };
    let mut map = map.clone();
    for key in ["created_at", "updated_at", "schema_version", "deleted_at"] {
        if let Some(v) = map.get_mut(key) {
            if let Some(n) = num_i64(v) {
                *v = Value::from(n);
            }
        }
    }
    Value::Object(map)
}

/// `isEntry` — minimal shape check used by snapshot parsing.
fn is_entry(value: &Value) -> bool {
    value.get("id").map(Value::is_string) == Some(true)
        && value.get("title").map(Value::is_string) == Some(true)
        && value.get("content_json").map(Value::is_string) == Some(true)
        && value.get("updated_at").map(Value::is_number) == Some(true)
}

fn de_entry(value: &Value) -> Option<Entry> {
    serde_json::from_value(sanitize_entry_numbers(value)).ok()
}

/// `parseSnapshot` — `{version:1, conflicts:[…]}` with per-item validation.
fn parse_snapshot(value: &Value) -> Vec<EntryConflict> {
    let Some(map) = value.as_object() else {
        return Vec::new();
    };
    // Vue `version === 1` — a JS number check; serde_json compares number
    // representations, so an integral float (`1.0`) goes through num_i64.
    if map.get("version").and_then(num_i64) != Some(1) {
        return Vec::new();
    }
    let Some(Value::Array(items)) = map.get("conflicts") else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let obj = item.as_object()?;
            let id = obj.get("id")?.as_str()?.to_string();
            let entry_id = obj.get("entryId")?.as_str()?.to_string();
            let state = EntryConflictState::from_str(obj.get("state")?.as_str()?)?;
            let local_v = obj.get("local")?;
            if !is_entry(local_v) {
                return None;
            }
            let remote_v = obj.get("remote");
            let remote = match remote_v {
                None | Some(Value::Null) => None,
                Some(v) if is_entry(v) => de_entry(v),
                Some(_) => return None,
            };
            let local = de_entry(local_v)?;
            let local_revision = obj.get("localRevision").and_then(num_i64)?;
            let remote_revision = match obj.get("remoteRevision") {
                None | Some(Value::Null) => None,
                Some(v) => Some(num_i64(v)?),
            };
            let detected_at = obj.get("detectedAt").and_then(num_i64)?;
            Some(EntryConflict {
                id,
                entry_id,
                state,
                local,
                remote,
                local_revision,
                remote_revision,
                detected_at,
                resolved_at: obj.get("resolvedAt").and_then(num_i64),
                resolution: obj
                    .get("resolution")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            })
        })
        .collect()
}

/// Serialize to the `{version:1, conflicts:[…]}` snapshot shape.
fn serialize_snapshot(conflicts: &[EntryConflict]) -> Value {
    let items: Vec<Value> = conflicts
        .iter()
        .map(|c| {
            let mut map = Map::new();
            map.insert("id".into(), Value::from(c.id.clone()));
            map.insert("entryId".into(), Value::from(c.entry_id.clone()));
            map.insert("state".into(), Value::from(c.state.as_str()));
            map.insert(
                "local".into(),
                serde_json::to_value(&c.local).unwrap_or(Value::Null),
            );
            map.insert(
                "remote".into(),
                c.remote
                    .as_ref()
                    .and_then(|r| serde_json::to_value(r).ok())
                    .unwrap_or(Value::Null),
            );
            map.insert("localRevision".into(), Value::from(c.local_revision));
            map.insert(
                "remoteRevision".into(),
                c.remote_revision.map(Value::from).unwrap_or(Value::Null),
            );
            map.insert("detectedAt".into(), Value::from(c.detected_at));
            if let Some(v) = c.resolved_at {
                map.insert("resolvedAt".into(), Value::from(v));
            }
            if let Some(v) = &c.resolution {
                map.insert("resolution".into(), Value::from(v.clone()));
            }
            Value::Object(map)
        })
        .collect();
    let mut snapshot = Map::new();
    snapshot.insert("version".into(), Value::from(1));
    snapshot.insert("conflicts".into(), Value::Array(items));
    Value::Object(snapshot)
}
