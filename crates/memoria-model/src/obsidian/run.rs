//! Engine-facing import/export orchestration — the non-UI half of
//! `ExportSettings.vue::importObsidianVaultFolder` / `exportObsidianVault`:
//! open the vault through Engine, plan drafts, then write through the
//! journaled transaction. Vue's progress/importGuard hooks are UI concerns —
//! headless imports have no unsaved editor state to guard.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use crate::mapping::parse_header_props_json;
use crate::model::{Entry, NoteType};
use crate::obsidian::import_apply::apply_import;
pub use crate::obsidian::import_apply::ObsidianImportOutcome;
use crate::obsidian::{import_obsidian_vault, ImportObsidianVaultArgs};
use crate::store::note_type_api::{note_type_record, NoteTypeApi};
use crate::store::transport::{ArkBridge, EngineError};
use crate::store::EntryApi;
use crate::system_types_data::{SYSTEM_TYPE_IMAGE_ID, SYSTEM_TYPE_NOTE_ID};
use crate::vault_ops::EngineVault;

/// `importObsidianVaultFolder` — scan → plan → journal-backed write.
/// `vault_identity` is Engine's stable `vaultKey` (never a raw path).
pub fn import_obsidian_vault_dir<B: ArkBridge>(
    bridge: &B,
    dir: &str,
) -> Result<ObsidianImportOutcome, String> {
    let vault_client = EngineVault::new(bridge.clone());
    let vault = vault_client.open_vault(dir).map_err(engine_message)?;
    let result = plan_and_apply(bridge, &vault);
    let _ = vault_client.close_vault(&vault.root_id);
    result
}

pub(crate) fn engine_message(error: EngineError) -> String {
    format!("{error:?}")
}

fn plan_and_apply<B: ArkBridge>(
    bridge: &B,
    vault: &crate::vault_ops::OpenedVault,
) -> Result<ObsidianImportOutcome, String> {
    let note_types = NoteTypeApi::new(bridge.clone())
        .list_note_types()
        .map_err(engine_message)?;
    let mut entries_api = EntryApi::new(bridge.clone());
    let existing_entries = entries_api.list_all_entries().map_err(engine_message)?;
    let vault_identity = vault.vault_key.clone();

    let existing_images = existing_entries
        .iter()
        .filter(|e| e.type_id.as_deref() == Some(SYSTEM_TYPE_IMAGE_ID))
        .map(|e| crate::obsidian::ObsidianExistingImage {
            id: e.id.clone(),
            header_props: parse_header_props_json(e.header_props_json.as_deref()),
        })
        .collect();

    let imported = import_obsidian_vault(&ImportObsidianVaultArgs {
        files: vault.files.clone(),
        images: vault.images.clone(),
        note_types: note_types.clone(),
        default_type_id: SYSTEM_TYPE_NOTE_ID.to_string(),
        image_type_id: SYSTEM_TYPE_IMAGE_ID.to_string(),
        type_id_mapping: Map::new(),
        vault_identity: Some(vault_identity.clone()),
        existing_images,
    });
    if !imported.image_report.can_write {
        return Err(format!(
            "Preflight images failed ({} collisions); canonical data unchanged.",
            imported.image_report.blocking_collisions.len()
        ));
    }
    let mut outcome = apply_import(
        bridge,
        &imported,
        &existing_entries,
        &note_types,
        &vault_identity,
    )?;
    outcome.image_report = imported.image_report;
    outcome.vault_name = vault.name.clone();
    Ok(outcome)
}

/// `exportObsidianVault` — plan files, then write them under an Engine grant.
pub fn export_obsidian_vault_dir<B: ArkBridge>(
    bridge: &B,
    entries: Vec<Entry>,
    note_types: &[NoteType],
    body_markdown_lookup: &HashMap<String, String>,
    title_lookup: &HashMap<String, String>,
    dir: &str,
) -> Result<u64, String> {
    let files = crate::obsidian::export_obsidian_vault_markdown_files(
        entries,
        note_types,
        body_markdown_lookup,
        title_lookup,
    )?;
    let vault = EngineVault::new(bridge.clone());
    let (root_id, _key) = vault.register_vault(dir).map_err(engine_message)?;
    let result = vault.export_vault(&root_id, &files).map_err(engine_message);
    let _ = vault.close_vault(&root_id);
    result
}

/// `ObsidianImportTransactionApi` over `ArkBridge` — entries/types persist as
/// ARK objects via `upsert_object`/`delete_object` (Vue `window.api` parity).
pub struct BridgeObsidianApi<B> {
    pub bridge: B,
}

impl<B: ArkBridge> crate::obsidian::transaction::ObsidianImportTransactionApi
    for BridgeObsidianApi<B>
{
    fn save_note_type(&mut self, note_type: &NoteType) -> Result<(), String> {
        self.bridge
            .upsert_object(note_type_record(note_type))
            .map(|_| ())
            .map_err(engine_message)
    }

    fn delete_note_type(&mut self, note_type_id: &str) -> Result<(), String> {
        self.bridge
            .delete_object(&format!("memoria:type:{note_type_id}"))
            .map(|_| ())
            .map_err(engine_message)
    }

    fn save_entry(&mut self, entry: &Entry) -> Result<(), String> {
        let object = crate::mapping::map_entry_to_ark_object(entry);
        self.bridge
            .upsert_object(json!({
                "id": object.id,
                "typeId": object.type_id,
                "typeVersion": object.type_version,
                "title": object.title,
                "contentJson": object.content_json,
                "propsJson": object.props_json,
                "createdAt": object.created_at,
                "updatedAt": object.updated_at,
                "deletedAt": object.deleted_at,
            }))
            .map_err(engine_message)?;
        sync_related_links(&self.bridge, entry)
    }

    fn delete_entry(&mut self, entry_id: &str) -> Result<(), String> {
        self.bridge
            .delete_object(entry_id)
            .map(|_| ())
            .map_err(engine_message)
    }
}

/// `syncRelatedLinks` — `related_notes` header prop ⇄ `related` object links.
/// Related ids stay out of `propsJson` (they're links, not props).
fn sync_related_links<B: ArkBridge>(bridge: &B, entry: &Entry) -> Result<(), String> {
    let props = parse_header_props_json(entry.header_props_json.as_deref());
    let related: Vec<String> = props
        .get("related_notes")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let links: Vec<crate::model::ArkObjectLink> = bridge
        .list_object_links()
        .map_err(engine_message)?
        .iter()
        .filter_map(|v| serde_json::from_value(v.clone()).ok())
        .collect();
    let owned: Vec<&crate::model::ArkObjectLink> = links
        .iter()
        .filter(|l| l.source_object_id == entry.id && l.link_type == "related")
        .collect();
    for link in &owned {
        if !related.contains(&link.target_object_id) {
            bridge
                .delete_object_link(&link.id)
                .map_err(engine_message)?;
        }
    }
    for target in &related {
        if owned.iter().any(|l| &l.target_object_id == target) {
            continue;
        }
        bridge
            .upsert_object_link(json!({
                "id": format!("{}:related:{target}", entry.id),
                "sourceObjectId": entry.id,
                "targetObjectId": target,
                "linkType": "related",
                "createdAt": crate::mapping::millis_to_ark_timestamp(Some(entry.updated_at)),
            }))
            .map_err(engine_message)?;
    }
    Ok(())
}
