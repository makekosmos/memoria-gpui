//! Port of `obsidianVaultImportTransaction.ts` — write-intent journal +
//! compensating rollback so a crashed/failed import never leaves partial
//! objects. The journal store is a trait; Engine/userData backends plug in
//! behind it (in-memory store for tests).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{Entry, NoteType};

use super::journal_recovery::{
    apply_operation, persist_journal, recover_obsidian_import_journal, rollback,
};

/// `ObsidianImportOperation` — `before`/`after` are serialized `Entry` or
/// `NoteType` payloads (plain JSON, clone-safe across store boundaries).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObsidianImportOperation {
    pub kind: String, // "entry" | "note-type"
    pub id: String,
    pub before: Option<Value>,
    pub after: Value,
}

/// `ObsidianImportJournal`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObsidianImportJournal {
    pub version: u32,
    pub id: String,
    pub status: String, // "applying" | "completed" | "rolled-back"
    pub operations: Vec<ObsidianImportOperation>,
    pub applied: Vec<usize>,
    #[serde(rename = "inFlight")]
    pub in_flight: Option<usize>,
    #[serde(default)]
    pub rolled_back: Vec<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// `ObsidianImportTransactionApi` — `Err(message)` replaces the Vue
/// `{ok:false,message}`/`false` returns.
pub trait ObsidianImportTransactionApi {
    fn save_note_type(&mut self, note_type: &NoteType) -> Result<(), String>;
    fn delete_note_type(&mut self, note_type_id: &str) -> Result<(), String>;
    fn save_entry(&mut self, entry: &Entry) -> Result<(), String>;
    fn delete_entry(&mut self, entry_id: &str) -> Result<(), String>;
}

/// `ObsidianImportJournalStore`.
pub trait ObsidianImportJournalStore {
    fn load(&mut self) -> Result<Option<ObsidianImportJournal>, String>;
    fn save(&mut self, journal: &ObsidianImportJournal) -> Result<(), String>;
    fn clear(&mut self) -> Result<(), String>;
}

/// Parse `version === 1 && Array.isArray(operations)` like `parseJournal`.
pub fn parse_journal(raw: &Value) -> Option<ObsidianImportJournal> {
    if raw.get("version").and_then(Value::as_u64) != Some(1)
        || !raw.get("operations").is_some_and(Value::is_array)
    {
        return None;
    }
    serde_json::from_value(raw.clone()).ok()
}

type BeforeOperationHook<'a> =
    Box<dyn FnMut(&ObsidianImportOperation, usize) -> Result<(), String> + 'a>;
type ProgressHook<'a> = Box<dyn FnMut(usize, usize, &ObsidianImportOperation) + 'a>;
type FailureHook<'a> = Box<dyn FnMut(Option<usize>, &str) + 'a>;
type RollbackHook<'a> = Box<dyn FnMut(&[usize]) + 'a>;
type CommitHook<'a> = Box<dyn FnMut() -> Result<(), String> + 'a>;

/// `runObsidianImportTransaction` options — hooks are optional callbacks.
#[derive(Default)]
pub struct ImportTransactionOptions<'a> {
    pub transaction_id: Option<String>,
    pub is_cancelled: Option<Box<dyn Fn() -> bool + 'a>>,
    pub before_operation: Option<BeforeOperationHook<'a>>,
    pub on_progress: Option<ProgressHook<'a>>,
    pub on_rollback: Option<RollbackHook<'a>>,
    pub on_failure: Option<FailureHook<'a>>,
    pub on_commit: Option<CommitHook<'a>>,
}

/// `runObsidianImportTransaction` — apply each op with an `inFlight` intent
/// checkpoint before the write and an `applied` checkpoint after; any failure
/// rolls back the applied set (and the in-flight write) in reverse order.
pub fn run_obsidian_import_transaction(
    operations: Vec<ObsidianImportOperation>,
    api: &mut dyn ObsidianImportTransactionApi,
    store: &mut dyn ObsidianImportJournalStore,
    options: &mut ImportTransactionOptions<'_>,
) -> Result<ObsidianImportJournal, String> {
    let _ = recover_obsidian_import_journal(api, store);
    let mut journal = ObsidianImportJournal {
        version: 1,
        id: options
            .transaction_id
            .clone()
            .unwrap_or_else(|| format!("obsidian-import:{}", crate::time::now_millis())),
        status: "applying".into(),
        operations,
        applied: Vec::new(),
        in_flight: None,
        rolled_back: Vec::new(),
        error: None,
    };
    if let Err(error) = persist_journal(store, &journal) {
        journal.status = "rolled-back".into();
        journal.error = Some(error.clone());
        let _ = persist_journal(store, &journal);
        return Err(error);
    }

    let mut operation_ark_touched = false;
    let mut current_operation_index: Option<usize> = None;
    let result = (|| -> Result<(), String> {
        for index in 0..journal.operations.len() {
            current_operation_index = Some(index);
            operation_ark_touched = false;
            if options.is_cancelled.as_deref().is_some_and(|c| c()) {
                return Err("Obsidian import cancelled".into());
            }
            if let Some(before) = options.before_operation.as_deref_mut() {
                before(&journal.operations[index], index)?;
            }
            // Persist intent before crossing the ARK boundary — recovery can
            // compensate even a partial upsert.
            journal.in_flight = Some(index);
            persist_journal(store, &journal)?;
            operation_ark_touched = true;
            let operation = journal.operations[index].clone();
            apply_operation(api, &operation, Some(&operation.after))?;
            // Two-phase: persist a staged snapshot, then commit in-memory so
            // a failed checkpoint leaves inFlight intact for compensation.
            let mut staged = journal.clone();
            staged.applied.push(index);
            staged.in_flight = None;
            persist_journal(store, &staged)?;
            journal.applied = staged.applied;
            journal.in_flight = None;
            if let Some(progress) = options.on_progress.as_deref_mut() {
                progress(journal.applied.len(), journal.operations.len(), &operation);
            }
        }
        current_operation_index = None;
        if let Some(commit) = options.on_commit.as_deref_mut() {
            commit()?;
        }
        journal.status = "completed".into();
        let _ = store.save(&journal);
        Ok(())
    })();

    match result {
        Ok(()) => Ok(journal),
        Err(error) => {
            journal.error = Some(error.clone());
            let operation_index = current_operation_index.or(journal.in_flight);
            if let Some(on_failure) = options.on_failure.as_deref_mut() {
                on_failure(operation_index, &error);
            }
            let checkpoint_before_ark =
                error.starts_with("Import journal checkpoint failed:") && !operation_ark_touched;
            if checkpoint_before_ark {
                // This operation never crossed ARK; only earlier applied
                // operations need compensation. Clearing inFlight prevents a
                // false delete when a non-atomic store exposed the intent.
                journal.in_flight = None;
                if let Err(rollback_error) = rollback(api, &mut journal, Some(store)) {
                    journal.error = Some(format!(
                        "{}; rollback: {rollback_error}",
                        journal.error.clone().unwrap_or_default()
                    ));
                    let _ = persist_journal(store, &journal);
                }
                if let Some(on_rollback) = options.on_rollback.as_deref_mut() {
                    on_rollback(&journal.rolled_back);
                }
                return Err(error);
            }
            let _ = persist_journal(store, &journal);
            if let Err(rollback_error) = rollback(api, &mut journal, Some(store)) {
                journal.error = Some(format!(
                    "{}; rollback: {rollback_error}",
                    journal.error.clone().unwrap_or_default()
                ));
                let _ = persist_journal(store, &journal);
            }
            if let Some(on_rollback) = options.on_rollback.as_deref_mut() {
                on_rollback(&journal.rolled_back);
            }
            Err(error)
        }
    }
}

/// In-memory `ObsidianImportJournalStore` — the test seam.
#[derive(Default)]
pub struct MemoryJournalStore {
    pub journal: Option<ObsidianImportJournal>,
    /// Fail the Nth `save` (1-based) — injects checkpoint errors.
    pub fail_on_save: Option<usize>,
    /// Store the Nth `save` then report failure — models a non-atomic
    /// adapter where the stale intent becomes visible before the error.
    pub write_through_fail_on_save: Option<usize>,
    pub(crate) saves: usize,
}

impl ObsidianImportJournalStore for MemoryJournalStore {
    fn load(&mut self) -> Result<Option<ObsidianImportJournal>, String> {
        Ok(self.journal.clone())
    }

    fn save(&mut self, journal: &ObsidianImportJournal) -> Result<(), String> {
        self.saves += 1;
        if self.fail_on_save == Some(self.saves) {
            return Err("injected journal failure".into());
        }
        self.journal = Some(journal.clone());
        if self.write_through_fail_on_save == Some(self.saves) {
            return Err("injected journal failure after write".into());
        }
        Ok(())
    }

    fn clear(&mut self) -> Result<(), String> {
        self.journal = None;
        Ok(())
    }
}
