//! Port of `src/store/entryConflicts.ts` — pure conflict-state logic plus a
//! bridge-backed snapshot (the localStorage fallback is a Vue host detail;
//! here `ConflictStore` bridges are the only persistence surface).

use crate::model::Entry;
use crate::time::now_millis;

pub const ENTRY_CONFLICTS_FILE: &str = "memoria-entry-conflicts.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryConflictState {
    RemoteUpdated,
    RemoteDeleted,
    StaleSave,
    Merged,
    Resolved,
}

impl EntryConflictState {
    pub(crate) fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "remote-updated" => Self::RemoteUpdated,
            "remote-deleted" => Self::RemoteDeleted,
            "stale-save" => Self::StaleSave,
            "merged" => Self::Merged,
            "resolved" => Self::Resolved,
            _ => return None,
        })
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RemoteUpdated => "remote-updated",
            Self::RemoteDeleted => "remote-deleted",
            Self::StaleSave => "stale-save",
            Self::Merged => "merged",
            Self::Resolved => "resolved",
        }
    }
    pub fn is_closed(&self) -> bool {
        matches!(self, Self::Merged | Self::Resolved)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntryConflict {
    pub id: String,
    pub entry_id: String,
    pub state: EntryConflictState,
    pub local: Entry,
    pub remote: Option<Entry>,
    pub local_revision: i64,
    pub remote_revision: Option<i64>,
    pub detected_at: i64,
    pub resolved_at: Option<i64>,
    pub resolution: Option<String>,
}

/// `conflictFreshness` — (revision, time) tuple.
fn conflict_freshness(conflict: &EntryConflict) -> (i64, i64) {
    (
        conflict
            .remote_revision
            .unwrap_or(0)
            .max(conflict.local_revision),
        conflict.detected_at.max(conflict.resolved_at.unwrap_or(0)),
    )
}

pub(crate) fn is_at_least_as_fresh(candidate: &EntryConflict, current: &EntryConflict) -> bool {
    conflict_freshness(candidate) >= conflict_freshness(current)
}

/// `unresolvedEntryConflicts`.
pub fn unresolved_entry_conflicts(conflicts: &[EntryConflict]) -> Vec<EntryConflict> {
    conflicts
        .iter()
        .filter(|c| !c.state.is_closed())
        .cloned()
        .collect()
}

/// `upsertEntryConflict` — reuses a pending conflict for the entry; stale
/// remote notifications never roll back `remoteRevision`.
pub fn upsert_entry_conflict(
    current: &[EntryConflict],
    local: &Entry,
    remote: Option<&Entry>,
    state: EntryConflictState,
    now: i64,
) -> Vec<EntryConflict> {
    let existing = current
        .iter()
        .find(|c| c.entry_id == local.id && !c.state.is_closed());
    if let (Some(_existing), Some(remote), Some(remote_revision)) =
        (existing, remote, existing.and_then(|e| e.remote_revision))
    {
        if remote.updated_at < remote_revision {
            return current.to_vec();
        }
    }
    let next = EntryConflict {
        id: existing
            .map(|e| e.id.clone())
            .unwrap_or_else(|| format!("{}:{}", local.id, now)),
        entry_id: local.id.clone(),
        state,
        local: existing
            .map(|e| e.local.clone())
            .unwrap_or_else(|| local.clone()),
        remote: remote.cloned(),
        local_revision: existing
            .map(|e| e.local_revision)
            .unwrap_or(local.updated_at),
        remote_revision: remote
            .map(|r| r.updated_at)
            .or_else(|| existing.and_then(|e| e.remote_revision)),
        detected_at: existing.map(|e| e.detected_at).unwrap_or(now),
        resolved_at: None,
        resolution: None,
    };
    let mut out: Vec<EntryConflict> = current
        .iter()
        .filter(|c| existing.map(|e| c.id != e.id.as_str()).unwrap_or(true))
        .cloned()
        .collect();
    out.push(next);
    out
}

/// `markEntryConflictResolved`.
pub fn mark_entry_conflict_resolved(
    current: &[EntryConflict],
    conflict_id: &str,
    resolution: Option<&str>,
    state: EntryConflictState,
    now: i64,
) -> Vec<EntryConflict> {
    current
        .iter()
        .map(|c| {
            if c.id == conflict_id {
                let mut next = c.clone();
                next.state = state;
                next.resolution = resolution.map(str::to_string);
                next.resolved_at = Some(now);
                next
            } else {
                c.clone()
            }
        })
        .collect()
}

/// `updateEntryConflictLocal`.
pub fn update_entry_conflict_local(current: &[EntryConflict], entry: &Entry) -> Vec<EntryConflict> {
    current
        .iter()
        .map(|c| {
            if c.entry_id == entry.id && !c.state.is_closed() {
                let mut next = c.clone();
                next.local = entry.clone();
                next.local_revision = entry.updated_at;
                next
            } else {
                c.clone()
            }
        })
        .collect()
}

/// `conflictForEntry`.
pub fn conflict_for_entry<'a>(
    conflicts: &'a [EntryConflict],
    entry_id: &str,
) -> Option<&'a EntryConflict> {
    conflicts
        .iter()
        .find(|c| c.entry_id == entry_id && !c.state.is_closed())
}

/// Convenience `now`.
pub fn now() -> i64 {
    now_millis()
}
