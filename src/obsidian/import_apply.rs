//! `importObsidianVaultFolder`'s operation-building loop — drafts/images →
//! journaled `ObsidianImportOperation`s, then `runObsidianImportTransaction`.
//! Vue `ExportSettings.vue` parity minus the UI guards/progress hooks.

use std::collections::{HashMap, HashSet};

use serde_json::{json, Map, Value};

use crate::mapping::parse_header_props_json;
use crate::model::{Entry, NoteType};
use crate::obsidian::import_entry::{
    existing_entry_for_import, normalized_props_json, title_key, to_entry,
};
use crate::obsidian::run::BridgeObsidianApi;
use crate::obsidian::transaction::{
    run_obsidian_import_transaction, ImportTransactionOptions, MemoryJournalStore,
    ObsidianImportOperation,
};
use crate::obsidian::{
    build_obsidian_related_import_plan, obsidian_note_id, ImportObsidianVaultResult,
    ObsidianImageImportReport, ObsidianImportDraft,
};
use crate::store::transport::ArkBridge;
use crate::system_types_data::{SYSTEM_TYPE_IMAGE, SYSTEM_TYPE_IMAGE_ID, SYSTEM_TYPE_NOTE_ID};
use crate::time::now_millis;

/// Import report — the `report` object Vue renders in settings.
#[derive(Clone, Debug, Default)]
pub struct ObsidianImportOutcome {
    pub created: Vec<String>,
    pub updated: Vec<String>,
    pub skipped: Vec<String>,
    /// `(sourcePath, target)` wikilinks that resolved to nothing.
    pub unresolved: Vec<(String, String)>,
    pub image_report: ObsidianImageImportReport,
    pub vault_name: String,
}

/// `importObsidianVaultFolder`'s apply loop — operation order matches Vue:
/// note-type ensure → entry writes (draft order) → related-notes second pass →
/// image objects.
pub(super) fn apply_import<B: ArkBridge>(
    bridge: &B,
    imported: &ImportObsidianVaultResult,
    existing_entries: &[Entry],
    note_types: &[NoteType],
    vault_identity: &str,
) -> Result<ObsidianImportOutcome, String> {
    let now = now_millis();
    let note_types_by_id: HashMap<&str, &NoteType> =
        note_types.iter().map(|t| (t.id.as_str(), t)).collect();
    let existing_title_ids: HashMap<String, String> = existing_entries
        .iter()
        .map(|e| (title_key(&e.title), e.id.clone()))
        .collect();

    let mut operations = vec![ObsidianImportOperation {
        kind: "note-type".into(),
        id: SYSTEM_TYPE_IMAGE.id.clone(),
        before: note_types_by_id
            .get(SYSTEM_TYPE_IMAGE.id.as_str())
            .and_then(|t| serde_json::to_value(t).ok()),
        after: serde_json::to_value(&*SYSTEM_TYPE_IMAGE).unwrap_or(Value::Null),
    }];
    let mut outcome = ObsidianImportOutcome::default();
    let mut planned_ids: HashMap<String, String> = HashMap::new();
    let mut skipped_sources: HashSet<String> = HashSet::new();
    let mut title_owners: HashSet<String> = HashSet::new();
    let mut imported_title_ids: HashMap<String, String> = HashMap::new();
    let mut scheduled: Vec<(String, Entry, Option<NoteType>)> = Vec::new();

    for draft in &imported.entries {
        let existing = existing_entry_for_import(draft, vault_identity, existing_entries);
        let id = existing
            .map(|e| e.id.clone())
            .or_else(|| draft.id.clone())
            .unwrap_or_else(|| obsidian_note_id(vault_identity, &draft.relative_path));
        let key = title_key(&draft.title);
        let decision = draft
            .header_props
            .get("import_conflict")
            .and_then(Value::as_str)
            .filter(|v| *v == "create-copy" || *v == "replace")
            .unwrap_or("skip");
        let copy_id = || {
            format!(
                "{}:copy",
                obsidian_note_id(vault_identity, &draft.relative_path)
            )
        };
        if title_owners.contains(&key) {
            if decision == "skip" {
                skipped_sources.insert(draft.relative_path.clone());
                outcome.skipped.push(draft.source_path.clone());
                continue;
            }
            planned_ids.insert(
                draft.relative_path.clone(),
                if decision == "create-copy" {
                    copy_id()
                } else {
                    id.clone()
                },
            );
        }
        let same_title = existing_entries.iter().find(|e| {
            title_key(&e.title) == key && Some(e.id.as_str()) != existing.map(|x| x.id.as_str())
        });
        if let Some(same) = same_title {
            let target = existing.map(|e| e.id.clone()).unwrap_or_else(|| id.clone());
            if same.id != target {
                if decision == "skip" {
                    skipped_sources.insert(draft.relative_path.clone());
                    outcome.skipped.push(draft.source_path.clone());
                    continue;
                }
                planned_ids.insert(
                    draft.relative_path.clone(),
                    if decision == "replace" {
                        same.id.clone()
                    } else {
                        copy_id()
                    },
                );
            }
        }
        title_owners.insert(key);

        let note_type = note_types_by_id
            .get(draft.type_id.as_str())
            .or_else(|| note_types_by_id.get(SYSTEM_TYPE_NOTE_ID))
            .copied()
            .cloned();
        let entry_id = planned_ids.get(&draft.relative_path).cloned().unwrap_or(id);
        let entry = to_entry(
            draft,
            entry_id,
            existing,
            note_type.as_ref(),
            vault_identity,
            now,
        );
        imported_title_ids.insert(title_key(&draft.title), entry.id.clone());
        if existing.is_some() {
            outcome.updated.push(draft.source_path.clone());
        } else {
            outcome.created.push(draft.source_path.clone());
        }
        operations.push(ObsidianImportOperation {
            kind: "entry".into(),
            id: entry.id.clone(),
            before: existing.and_then(|e| serde_json::to_value(e).ok()),
            after: serde_json::to_value(&entry).unwrap_or(Value::Null),
        });
        scheduled.push((draft.relative_path.clone(), entry, note_type));
    }

    // Second pass — `related_notes` links resolve once every imported object
    // has an id; the linked entry rewrites the same object (`before` = pass 1).
    let drafts_by_relative: HashMap<&str, &ObsidianImportDraft> = imported
        .entries
        .iter()
        .map(|d| (d.relative_path.as_str(), d))
        .collect();
    for (relative_path, entry, note_type) in &scheduled {
        if skipped_sources.contains(relative_path) {
            continue;
        }
        let draft = drafts_by_relative[relative_path.as_str()];
        let plan = build_obsidian_related_import_plan(
            draft,
            &entry.id,
            &imported_title_ids,
            Some(&existing_title_ids),
        );
        outcome.unresolved.extend(
            plan.unresolved_targets
                .unwrap_or_default()
                .into_iter()
                .map(|target| (draft.source_path.clone(), target)),
        );
        if plan.second_pass_related_ids.is_empty() {
            continue;
        }
        let mut props = parse_header_props_json(entry.header_props_json.as_deref());
        props.insert("related_notes".into(), json!(plan.second_pass_related_ids));
        let linked = Entry {
            header_props_json: Some(normalized_props_json(note_type.as_ref(), props)),
            updated_at: now,
            ..entry.clone()
        };
        operations.push(ObsidianImportOperation {
            kind: "entry".into(),
            id: linked.id.clone(),
            before: serde_json::to_value(entry).ok(),
            after: serde_json::to_value(&linked).unwrap_or(Value::Null),
        });
    }

    for image in &imported.images {
        let source_path = image
            .header_props
            .get("source_path")
            .and_then(Value::as_str)
            .unwrap_or(&image.title)
            .to_string();
        let existing = existing_entries.iter().find(|e| e.id == image.id);
        let entry = Entry {
            id: image.id.clone(),
            title: image.title.clone(),
            content_json: serde_json::to_string(&image.content_json).unwrap_or_default(),
            created_at: existing.map(|e| e.created_at).unwrap_or(now),
            updated_at: now,
            folder_id: None,
            type_id: Some(SYSTEM_TYPE_IMAGE_ID.to_string()),
            header_layout: None,
            header_props_json: Some(normalized_props_json(
                Some(&SYSTEM_TYPE_IMAGE),
                image.header_props.clone(),
            )),
            schema_version: Some(1),
            deleted_at: None,
            content_loaded: Some(true),
            extra: Map::new(),
        };
        if existing.is_some() {
            outcome.updated.push(source_path);
        } else {
            outcome.created.push(source_path);
        }
        operations.push(ObsidianImportOperation {
            kind: "entry".into(),
            id: image.id.clone(),
            before: existing.and_then(|e| serde_json::to_value(e).ok()),
            after: serde_json::to_value(&entry).unwrap_or(Value::Null),
        });
    }

    let mut api = BridgeObsidianApi {
        bridge: bridge.clone(),
    };
    let mut store = MemoryJournalStore::default();
    let mut options = ImportTransactionOptions::default();
    run_obsidian_import_transaction(operations, &mut api, &mut store, &mut options)
        .map(|_| outcome)
        .map_err(|e| format!("Import failed and rolled back: {e}"))
}
