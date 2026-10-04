//! Journal application + recovery for `obsidianVaultImportTransaction.ts` —
//! `applyOperation`, `rollback`, `recoverObsidianImportJournal`. The journal
//! shape and the run loop live in `transaction.rs`.

use serde_json::Value;

use crate::model::{Entry, NoteType};

use super::transaction::{
    ObsidianImportJournal, ObsidianImportJournalStore, ObsidianImportOperation,
    ObsidianImportTransactionApi,
};

pub(crate) fn operation_value(kind: &str, value: &Value) -> Result<OperationValue, String> {
    match kind {
        "entry" => serde_json::from_value::<Entry>(value.clone())
            .map(OperationValue::Entry)
            .map_err(|e| format!("Import journal entry value: {e}")),
        "note-type" => serde_json::from_value::<NoteType>(value.clone())
            .map(OperationValue::NoteType)
            .map_err(|e| format!("Import journal type value: {e}")),
        _ => Err(format!("Import journal operation kind: {kind}")),
    }
}

pub(crate) enum OperationValue {
    Entry(Entry),
    NoteType(NoteType),
}

pub(crate) fn apply_operation(
    api: &mut dyn ObsidianImportTransactionApi,
    operation: &ObsidianImportOperation,
    value: Option<&Value>,
) -> Result<(), String> {
    match operation.kind.as_str() {
        "entry" => match value {
            Some(value) => {
                let OperationValue::Entry(entry) = operation_value("entry", value)? else {
                    unreachable!()
                };
                api.save_entry(&entry)
                    .map_err(|m| format!("Import entry write failed: {}: {m}", operation.id))
            }
            None => api
                .delete_entry(&operation.id)
                .map_err(|m| format!("Import entry delete failed: {}: {m}", operation.id)),
        },
        "note-type" => match value {
            Some(value) => {
                let OperationValue::NoteType(note_type) = operation_value("note-type", value)?
                else {
                    unreachable!()
                };
                api.save_note_type(&note_type)
                    .map_err(|m| format!("Import type write failed: {}: {m}", operation.id))
            }
            None => api
                .delete_note_type(&operation.id)
                .map_err(|m| format!("Import type delete failed: {}: {m}", operation.id)),
        },
        other => Err(format!("Import journal operation kind: {other}")),
    }
}

pub(crate) fn rollback(
    api: &mut dyn ObsidianImportTransactionApi,
    journal: &mut ObsidianImportJournal,
    mut store: Option<&mut dyn ObsidianImportJournalStore>,
) -> Result<(), String> {
    let mut indexes: Vec<usize> = journal.applied.clone();
    if let Some(in_flight) = journal.in_flight {
        // A crash can happen after the API side effect and before `applied`
        // was persisted — compensate the in-flight operation too.
        indexes.push(in_flight);
    }
    indexes.sort_unstable_by(|a, b| b.cmp(a));
    indexes.dedup();
    for index in indexes {
        let Some(operation) = journal.operations.get(index) else {
            continue;
        };
        apply_operation(api, operation, operation.before.as_ref())?;
        journal.rolled_back.push(index);
        journal.applied.retain(|candidate| *candidate != index);
        if journal.in_flight == Some(index) {
            journal.in_flight = None;
        }
        if let Some(store) = store.as_deref_mut() {
            store.save(journal)?;
        }
    }
    journal.status = "rolled-back".into();
    journal.applied.clear();
    journal.in_flight = None;
    if let Some(store) = store {
        store.save(journal)?;
    }
    Ok(())
}

pub(crate) fn persist_journal(
    store: &mut dyn ObsidianImportJournalStore,
    journal: &ObsidianImportJournal,
) -> Result<(), String> {
    store
        .save(journal)
        .map_err(|e| format!("Import journal checkpoint failed: {e}"))
}

/// `recoverObsidianImportJournal` — a leftover `applying` journal rolls back.
pub fn recover_obsidian_import_journal(
    api: &mut dyn ObsidianImportTransactionApi,
    store: &mut dyn ObsidianImportJournalStore,
) -> Result<Option<ObsidianImportJournal>, String> {
    let journal = store.load()?;
    let Some(mut journal) = journal else {
        return Ok(None);
    };
    if journal.status != "applying" {
        return Ok(Some(journal));
    }
    if let Err(error) = rollback(api, &mut journal, Some(store)) {
        journal.error = Some(error.clone());
        let _ = persist_journal(store, &journal);
        return Err(error);
    }
    Ok(Some(journal))
}
