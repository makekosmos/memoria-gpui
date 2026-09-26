//! Port of `src/store/edenStoreSaveActions.ts` — `createEdenStoreSaveActions`.
//!
//! The Vue coordinator serializes saves per entry id through an in-flight
//! promise + a queued draft slot. In Rust the same guarantee comes from the
//! worker owning `SaveActions` on one thread; the queued path is kept for
//! re-entrant `handle_save` calls (same entry while a save is running).
//!
//! `recordConflict`/`recheckConflicts` are caller-injected like the Vue state
//! callbacks — the store wires them to `entry_conflicts` + persistence.

use std::collections::HashMap;

use crate::entry_conflicts::EntryConflictState;
use crate::model::{Entry, SaveEntryResult};

/// `window.api` — the Eden bridge surface used by `handleSave`.
/// `None` results model `!window.api` (the Vue early-return).
pub trait SaveBridge {
    fn save_entry(&mut self, entry: &Entry) -> Result<SaveEntryResult, String>;
    fn load_entry(&mut self, id: &str) -> Result<Option<Entry>, String>;
}

/// The Vue reactive state the save action mutates.
#[derive(Debug, Default)]
pub struct SaveActionState {
    pub current_entry: Option<Entry>,
    pub entries: Vec<Entry>,
    pub is_current_entry_dirty: bool,
    pub dirty_entry_id: Option<String>,
    /// `latestSaveTimestamps` — newest locally stamped `updated_at` per entry.
    pub latest_save_timestamps: HashMap<String, i64>,
    save_coordinators: HashMap<String, SaveCoordinator>,
}

#[derive(Debug, Default)]
struct SaveCoordinator {
    in_flight: bool,
    queued: Option<Entry>,
}

/// `markLatestLocalEntry` — stamps the map when no newer stamp exists.
fn mark_latest_local_entry(state: &mut SaveActionState, entry: &Entry) {
    let previous = state.latest_save_timestamps.get(&entry.id).copied();
    if previous.map(|v| v <= entry.updated_at).unwrap_or(true) {
        state
            .latest_save_timestamps
            .insert(entry.id.clone(), entry.updated_at);
    }
}

/// `createEdenStoreSaveActions` — owns the state + bridge; the caller wires
/// the two callbacks (`record_conflict` keeps `EntryConflictState::StaleSave`
/// explicit like Vue's `Extract<…, "stale-save">`).
pub struct SaveActions<B: SaveBridge> {
    pub state: SaveActionState,
    pub bridge: Option<B>,
    pub record_conflict: Box<dyn FnMut(&Entry, Option<&Entry>, EntryConflictState)>,
    pub recheck_conflicts: Option<Box<dyn FnMut(&str)>>,
}

impl<B: SaveBridge> SaveActions<B> {
    /// `handleSave` — `Ok(None)` when the bridge is missing (`!window.api`).
    pub fn handle_save(&mut self, entry: &Entry) -> Result<Option<SaveEntryResult>, String> {
        if self.bridge.is_none() {
            return Ok(None);
        }

        let coordinator = self
            .state
            .save_coordinators
            .entry(entry.id.clone())
            .or_default();

        // A save for this entry is already running — park the newest draft.
        if coordinator.in_flight {
            coordinator.queued = Some(entry.clone());
            // Sync callers wait inside `run_save_loop`; signal "accepted into
            // the queue" as `Ok(None)` so no caller fabricates success.
            return Ok(None);
        }

        self.run_save_loop(entry)
    }

    /// `runSaveLoop` — persists, then drains the queued draft. Each caller
    /// keeps its own result (the queued outcome never swallows a stale save).
    fn run_save_loop(&mut self, first: &Entry) -> Result<Option<SaveEntryResult>, String> {
        let entry_id = first.id.clone();
        self.state
            .save_coordinators
            .entry(entry_id.clone())
            .or_default()
            .in_flight = true;

        let mut current = first.clone();
        let mut last_queued_at: Option<i64> = None;
        // The caller that started this request must observe its own result —
        // returning the queued result would swallow a stale_entry from the
        // first write and falsely report success to that caller. Queued
        // drafts' results are dropped: re-entrant callers already received
        // `Ok(None)` (`!window.api`-style "accepted") and there are no
        // waiters to resolve in the sync port.
        let mut own_result: Option<Option<SaveEntryResult>> = None;
        let result = loop {
            match self.persist_entry(&current) {
                Ok(result) => {
                    if own_result.is_none() {
                        own_result = Some(result);
                    }
                    let queued = self
                        .state
                        .save_coordinators
                        .get_mut(&entry_id)
                        .and_then(|c| c.queued.take());
                    match queued {
                        None => break Ok(()),
                        Some(next) => {
                            last_queued_at = Some(next.updated_at);
                            current = next;
                        }
                    }
                }
                Err(error) => {
                    // Vue's `catch` drops the queued draft before `finally`.
                    if let Some(c) = self.state.save_coordinators.get_mut(&entry_id) {
                        c.queued = None;
                        c.in_flight = false;
                    }
                    break Err(error);
                }
            }
        };

        // `finally`: tear down the coordinator only when nothing is queued —
        // runs after errors too (Vue's `finally` is unconditional).
        let has_queued = self
            .state
            .save_coordinators
            .get(&entry_id)
            .and_then(|c| c.queued.as_ref())
            .is_some();
        if !has_queued {
            self.state.save_coordinators.remove(&entry_id);
            for stamp in [Some(first.updated_at), last_queued_at]
                .into_iter()
                .flatten()
            {
                if self.state.latest_save_timestamps.get(&entry_id) == Some(&stamp) {
                    self.state.latest_save_timestamps.remove(&entry_id);
                }
            }
        }
        result.map(|_| own_result.unwrap_or_default())
    }

    /// `persistEntry` — stamp, write, then apply the result to state.
    fn persist_entry(&mut self, entry: &Entry) -> Result<Option<SaveEntryResult>, String> {
        mark_latest_local_entry(&mut self.state, entry);
        let bridge = self
            .bridge
            .as_mut()
            .ok_or_else(|| "bridge missing".to_string())?;
        let result = bridge.save_entry(entry)?;

        if !result.ok {
            if result.reason.as_deref() == Some("stale_entry") {
                // `loadEntry` errors are tolerated — the local draft stays
                // durable even while offline or mid-restart.
                let remote = bridge.load_entry(&entry.id).ok().flatten();
                (self.record_conflict)(entry, remote.as_ref(), EntryConflictState::StaleSave);
            }
            return Ok(Some(result));
        }

        // An older write finishing after a newer local edit must not touch
        // the model — the stamp moved past this entry's `updated_at`.
        if self.state.latest_save_timestamps.get(&entry.id) != Some(&entry.updated_at) {
            return Ok(Some(result));
        }

        match self.state.entries.iter().position(|e| e.id == entry.id) {
            Some(idx) => self.state.entries[idx] = entry.clone(),
            None => self.state.entries.insert(0, entry.clone()),
        }
        if self.state.current_entry.as_ref().map(|e| e.id.as_str()) == Some(entry.id.as_str()) {
            self.state.current_entry = Some(entry.clone());
            self.state.is_current_entry_dirty = false;
        }
        if self.state.dirty_entry_id.as_deref() == Some(entry.id.as_str()) {
            self.state.dirty_entry_id = None;
        }

        if let Some(recheck) = self.recheck_conflicts.as_mut() {
            recheck(&entry.id);
        }
        Ok(Some(result))
    }
}
