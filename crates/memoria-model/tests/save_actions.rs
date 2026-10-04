//! Port of `tests/saveConflict.test.ts` — the `createEdenStoreSaveActions`
//! coordinator. Vue's promise queue only exists for overlapping async calls;
//! the Rust worker is single-threaded, so ordering is exercised through
//! sequential `handle_save` calls — each caller still gets its own result.

use memoria_model::entry_conflicts::EntryConflictState;
use memoria_model::model::{Entry, SaveEntryResult};
use memoria_model::save_actions::{SaveActions, SaveBridge};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

fn entry(id: &str, updated_at: i64) -> Entry {
    Entry {
        id: id.into(),
        title: "Note".into(),
        content_json: "{}".into(),
        updated_at,
        type_id: Some("note_obj".into()),
        ..Default::default()
    }
}

struct FakeBridge {
    calls: Vec<Entry>,
    results: VecDeque<Result<SaveEntryResult, String>>,
    remote: Option<Entry>,
}

impl SaveBridge for FakeBridge {
    fn save_entry(&mut self, entry: &Entry) -> Result<SaveEntryResult, String> {
        self.calls.push(entry.clone());
        self.results.pop_front().expect("canned save result")
    }
    fn load_entry(&mut self, _id: &str) -> Result<Option<Entry>, String> {
        Ok(self.remote.clone())
    }
}

type Recorded = Rc<RefCell<Vec<(Entry, Option<Entry>, EntryConflictState)>>>;

fn actions(
    bridge: Option<FakeBridge>,
) -> (SaveActions<FakeBridge>, Recorded, Rc<RefCell<Vec<String>>>) {
    let recorded: Recorded = Rc::new(RefCell::new(Vec::new()));
    let rechecked: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let recorded_c = recorded.clone();
    let rechecked_c = rechecked.clone();
    let actions = SaveActions {
        state: Default::default(),
        bridge,
        record_conflict: Box::new(move |local, remote, state| {
            recorded_c
                .borrow_mut()
                .push((local.clone(), remote.cloned(), state));
        }),
        recheck_conflicts: Some(Box::new(move |id| {
            rechecked_c.borrow_mut().push(id.to_string())
        })),
    };
    (actions, recorded, rechecked)
}

#[test]
fn missing_bridge_returns_none() {
    let (mut actions, _, _) = actions(None);
    assert_eq!(actions.handle_save(&entry("n1", 20)).unwrap(), None);
}

#[test]
fn stale_save_records_conflict_and_keeps_dirty_state() {
    let remote = entry("n1", 30);
    let bridge = FakeBridge {
        calls: Vec::new(),
        results: VecDeque::from([Ok(SaveEntryResult::failed(
            "stale_entry",
            "Заметка уже была обновлена более новой версией",
        ))]),
        remote: Some(remote.clone()),
    };
    let (mut actions, recorded, _) = actions(Some(bridge));
    actions.state.current_entry = Some(entry("n1", 20));
    actions.state.is_current_entry_dirty = true;
    actions.state.dirty_entry_id = Some("n1".into());
    actions.state.entries = vec![entry("n1", 20)];

    let local = entry("n1", 20);
    let result = actions.handle_save(&local).unwrap().unwrap();

    assert!(!result.ok);
    assert_eq!(result.reason.as_deref(), Some("stale_entry"));
    assert!(actions.state.is_current_entry_dirty);
    assert_eq!(actions.state.dirty_entry_id.as_deref(), Some("n1"));

    let recorded = recorded.borrow();
    assert_eq!(recorded.len(), 1);
    let (conflict_local, conflict_remote, state) = &recorded[0];
    assert_eq!(conflict_local.id, "n1");
    assert_eq!(conflict_remote.as_ref().unwrap().updated_at, 30);
    assert_eq!(*state, EntryConflictState::StaleSave);
}

#[test]
fn sequential_saves_run_in_order_and_return_own_results() {
    let bridge = FakeBridge {
        calls: Vec::new(),
        results: VecDeque::from([
            Ok(SaveEntryResult::failed("stale_entry", "stale")),
            Ok(SaveEntryResult::ok("n1")),
        ]),
        remote: Some(entry("n1", 30)),
    };
    let (mut actions, recorded, rechecked) = actions(Some(bridge));

    // The entry is open and dirty; the stale save must keep it that way.
    actions.state.current_entry = Some(entry("n1", 21));
    actions.state.is_current_entry_dirty = true;
    actions.state.dirty_entry_id = Some("n1".into());

    let first = actions.handle_save(&entry("n1", 20)).unwrap().unwrap();
    assert!(!first.ok);
    assert_eq!(first.reason.as_deref(), Some("stale_entry"));
    assert!(actions.state.is_current_entry_dirty);

    let second = actions.handle_save(&entry("n1", 21)).unwrap().unwrap();
    assert!(second.ok);
    assert_eq!(recorded.borrow().len(), 1);
    assert_eq!(*rechecked.borrow(), vec!["n1".to_string()]);
    assert_eq!(actions.bridge.as_ref().unwrap().calls.len(), 2);
    // The successful save cleared the dirty flags and refreshed currentEntry.
    assert!(!actions.state.is_current_entry_dirty);
    assert_eq!(actions.state.dirty_entry_id, None);
    assert_eq!(actions.state.current_entry.as_ref().unwrap().updated_at, 21);
}

#[test]
fn older_in_flight_write_does_not_clobber_newer_state() {
    let bridge = FakeBridge {
        calls: Vec::new(),
        results: VecDeque::from([Ok(SaveEntryResult::ok("n1"))]),
        remote: None,
    };
    let (mut actions, _, rechecked) = actions(Some(bridge));
    // A newer draft already stamped the entry (an autosave landed meanwhile).
    actions.state.latest_save_timestamps.insert("n1".into(), 99);
    actions.state.entries = vec![entry("n1", 50)];

    let result = actions.handle_save(&entry("n1", 20)).unwrap().unwrap();
    assert!(result.ok);
    // State untouched — the saved revision was already superseded.
    assert_eq!(actions.state.entries[0].updated_at, 50);
    assert!(rechecked.borrow().is_empty());
}

#[test]
fn successful_latest_save_rechecks_conflicts() {
    let bridge = FakeBridge {
        calls: Vec::new(),
        results: VecDeque::from([Ok(SaveEntryResult::ok("n1"))]),
        remote: None,
    };
    let (mut actions, _, rechecked) = actions(Some(bridge));
    let e = entry("n1", 20);
    let result = actions.handle_save(&e).unwrap().unwrap();
    assert!(result.ok);
    assert_eq!(*rechecked.borrow(), vec!["n1".to_string()]);
    assert_eq!(actions.state.entries.len(), 1);
}
