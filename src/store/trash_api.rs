//! Port of `src/lib/kepler-trash-storage.ts` — trash listing, restore,
//! permanent delete and vault size accounting over the ARK bridge.

use serde_json::Value;

use crate::mapping::{
    js_truthy, map_ark_object_to_entry, millis_to_ark_timestamp, DEFAULT_ARK_TYPE_ID,
};
use crate::migration::compatibility_ark_type_ids;
use crate::model::ark::ensure_list;
use crate::model::{
    ArkObjectLink, ArkObjectRecord, ArkObjectType, DeleteEntryResult, Entry, VaultStorageInfo,
};

use super::transport::{ArkBridge, EngineError};

fn de<T: serde::de::DeserializeOwned>(value: &Value) -> Option<T> {
    serde_json::from_value(value.clone()).ok()
}

/// JS string `.length` counts UTF-16 code units.
fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `createTrashStorageApi`.
pub struct TrashStorageApi<B: ArkBridge> {
    bridge: B,
    discovery_available: Option<bool>,
}

impl<B: ArkBridge> TrashStorageApi<B> {
    pub fn new(bridge: B) -> Self {
        Self {
            bridge,
            discovery_available: None,
        }
    }

    /// `listAllObjects` — discovery query first, `list_objects` fallback when
    /// the host does not expose `list_object_types`.
    fn list_all_objects(&mut self) -> Vec<ArkObjectRecord> {
        match self.bridge.rpc("list_object_types", serde_json::json!({})) {
            Ok(raw) if !raw.is_null() => {
                self.discovery_available = Some(true);
                let discovered: Vec<String> = ensure_list(&raw)
                    .iter()
                    .filter_map(de::<ArkObjectType>)
                    .map(|t| t.id)
                    .collect();
                let mut objects = Vec::new();
                for type_id in compatibility_ark_type_ids(&[], &discovered) {
                    if let Ok(items) = self.bridge.list_objects_by_type(&type_id) {
                        objects.extend(items.iter().filter_map(de));
                    }
                }
                dedupe(objects)
            }
            _ => {
                self.discovery_available = Some(false);
                self.bridge
                    .list_objects(DEFAULT_ARK_TYPE_ID)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(de)
                    .collect()
            }
        }
    }

    /// `listTrashEntries` — deleted objects only, newest trashed first. A
    /// `list_object_links` failure propagates like the Vue `Promise.all`.
    pub fn list_trash_entries(&mut self) -> Result<Vec<Entry>, EngineError> {
        let objects = self.list_all_objects();
        let links: Vec<ArkObjectLink> = self
            .bridge
            .list_object_links()?
            .iter()
            .filter_map(de)
            .collect();
        let mut entries: Vec<Entry> = objects
            .iter()
            .filter(|o| o.deleted_at.as_ref().map(js_truthy).unwrap_or(false))
            .map(|o| map_ark_object_to_entry(o, &links, None))
            .collect();
        entries.sort_by_key(|e| std::cmp::Reverse(e.deleted_at.unwrap_or(0)));
        Ok(entries)
    }

    /// `restoreEntry` — clears `deletedAt` and bumps `updatedAt`.
    pub fn restore_entry(&mut self, entry_id: &str) -> Result<DeleteEntryResult, EngineError> {
        let existing = self.bridge.get_object(entry_id)?;
        if existing.is_null() {
            return Ok(DeleteEntryResult {
                ok: false,
                entry_id: Some(entry_id.into()),
                ..Default::default()
            });
        }
        let object: ArkObjectRecord =
            serde_json::from_value(existing).map_err(|_| EngineError::Malformed)?;
        self.bridge.upsert_object(serde_json::json!({
            "id": object.id,
            "typeId": object.type_id,
            "title": object.title,
            "contentJson": object.content_json,
            "propsJson": object.props_json,
            "createdAt": object.created_at,
            "updatedAt": millis_to_ark_timestamp(None),
            "deletedAt": null,
        }))?;
        Ok(DeleteEntryResult {
            ok: true,
            entry_id: Some(entry_id.into()),
            ..Default::default()
        })
    }

    /// `permanentDeleteEntry`.
    pub fn permanent_delete_entry(
        &mut self,
        entry_id: &str,
    ) -> Result<DeleteEntryResult, EngineError> {
        self.bridge.delete_object(entry_id)?;
        Ok(DeleteEntryResult {
            ok: true,
            entry_id: Some(entry_id.into()),
            ..Default::default()
        })
    }

    /// `purgeExpiredTrash` — no expiry in the current Vue contract.
    pub fn purge_expired_trash(&self) -> (bool, usize) {
        (true, 0)
    }

    /// `getVaultStorageInfo`.
    pub fn get_vault_storage_info(&mut self) -> VaultStorageInfo {
        let objects = self.list_all_objects();
        let active: Vec<&ArkObjectRecord> = objects
            .iter()
            .filter(|o| !o.deleted_at.as_ref().map(js_truthy).unwrap_or(false))
            .collect();
        let trashed: Vec<&ArkObjectRecord> = objects
            .iter()
            .filter(|o| o.deleted_at.as_ref().map(js_truthy).unwrap_or(false))
            .collect();
        let size_of = |o: &ArkObjectRecord| {
            let content = if o.content_json.is_null() {
                js_len("\"\"")
            } else {
                js_len(&serde_json::to_string(&o.content_json).unwrap_or_default())
            };
            content + js_len(&o.title)
        };
        let text_bytes: usize = active.iter().map(|o| size_of(o)).sum();
        let trash_bytes: usize = trashed.iter().map(|o| size_of(o)).sum();
        VaultStorageInfo {
            text_bytes,
            trash_bytes,
            db_bytes: text_bytes + trash_bytes,
            vault_bytes: text_bytes + trash_bytes,
            entry_count: active.len(),
            trash_count: trashed.len(),
        }
    }

    /// `getDiskFreeSpace` — the Vue stub returns 0.
    pub fn get_disk_free_space(&self) -> i64 {
        0
    }
}

fn dedupe(objects: Vec<ArkObjectRecord>) -> Vec<ArkObjectRecord> {
    let mut seen = std::collections::HashSet::new();
    objects
        .into_iter()
        .filter(|o| seen.insert(o.id.clone()))
        .collect()
}
