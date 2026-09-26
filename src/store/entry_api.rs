//! Port of `src/lib/kepler-entry-api.ts` — list/load/save/delete/search over
//! the Engine transport, with Vue's filtering and save guards intact.

use serde_json::{json, Value};

use crate::header_props::validate_header_props;
use crate::live_list_filter::should_include_type_in_eden_list_for_live_update;
use crate::mapping::{
    ark_timestamp_to_millis, js_truthy, map_ark_object_summary_to_entry, map_ark_object_to_entry,
    map_entry_to_ark_object, millis_to_ark_timestamp, normalize_entry, parse_header_props_json,
    should_include_object_in_eden_list, DEFAULT_ARK_TYPE_ID,
};
use crate::migration::compatibility_ark_type_ids;
use crate::model::ark::ensure_list;
use crate::model::{
    ArkObjectLink, ArkObjectRecord, ArkObjectSummary, ArkObjectType, DeleteEntryResult, Entry,
    SearchResult,
};
use crate::system_types::is_system_type;

use super::note_type_api::NoteTypeApi;
use super::transport::{ArkBridge, EngineError};

use helpers::*;

mod helpers;
mod save;

/// `createEntryApi` — owns a `NoteTypeApi` and a discovery-availability memo.
pub struct EntryApi<B: ArkBridge> {
    bridge: B,
    pub note_types: NoteTypeApi<B>,
    discovery_available: Option<bool>,
}

impl<B: ArkBridge> EntryApi<B> {
    pub fn new(bridge: B) -> Self {
        let note_types = NoteTypeApi::new(bridge.clone());
        Self {
            bridge,
            note_types,
            discovery_available: None,
        }
    }

    pub fn bridge(&self) -> &B {
        &self.bridge
    }

    /// `discoverObjectTypeIds` — explicit selection short-circuits discovery.
    fn discover_object_type_ids(&mut self, selected: &[String]) -> Vec<String> {
        if !selected.is_empty() {
            self.discovery_available = Some(false);
            return selected.to_vec();
        }
        match self.bridge.rpc("list_object_types", serde_json::json!({})) {
            // Vue: `raw === undefined || raw === null` throws → unavailable.
            Ok(raw) if !raw.is_null() => {
                self.discovery_available = Some(true);
                let discovered: Vec<String> = ensure_list(&raw)
                    .iter()
                    .filter_map(de::<ArkObjectType>)
                    .map(|t| t.id)
                    .collect();
                compatibility_ark_type_ids(selected, &discovered)
            }
            _ => {
                self.discovery_available = Some(false);
                selected.to_vec()
            }
        }
    }

    /// `listObjectsForVisibleTypes`.
    fn list_objects_for_visible_types(&mut self, type_ids: &[String]) -> Vec<ArkObjectRecord> {
        let ids = self.discover_object_type_ids(type_ids);
        if ids.is_empty() && self.discovery_available == Some(false) {
            return self
                .bridge
                .list_objects(DEFAULT_ARK_TYPE_ID)
                .unwrap_or_default()
                .iter()
                .filter_map(de)
                .collect();
        }
        let mut out = Vec::new();
        for type_id in ids {
            if let Ok(items) = self.bridge.list_objects_by_type(&type_id) {
                out.extend(items.iter().filter_map(de));
            }
        }
        dedupe_by_id(out)
    }

    /// `listObjectSummariesForVisibleTypes` — merge summaries with the full
    /// record set on partial failure; fall back to canonical-only.
    fn list_object_summaries_for_visible_types(
        &mut self,
        type_ids: &[String],
    ) -> Vec<ArkObjectSummary> {
        let ids = self.discover_object_type_ids(type_ids);
        if ids.is_empty() {
            let canonical: Vec<ArkObjectSummary> = self
                .bridge
                .list_object_summaries(DEFAULT_ARK_TYPE_ID)
                .unwrap_or_default()
                .iter()
                .filter_map(de)
                .collect();
            if !canonical.is_empty() {
                return canonical;
            }
            return self
                .list_objects_for_visible_types(type_ids)
                .into_iter()
                .map(summary_of)
                .collect();
        }
        let mut merged_summaries: Vec<ArkObjectSummary> = Vec::new();
        let mut partial_failure = false;
        for type_id in &ids {
            match self.bridge.list_object_summaries_by_type(type_id) {
                Ok(items) => merged_summaries.extend(items.iter().filter_map(de)),
                Err(_) => partial_failure = true,
            }
        }
        let flattened = dedupe_by_id(merged_summaries);
        if !flattened.is_empty() && !partial_failure {
            return flattened;
        }
        if partial_failure || flattened.is_empty() {
            let full = self.list_objects_for_visible_types(type_ids);
            let merged = dedupe_by_id(
                flattened
                    .into_iter()
                    .chain(full.into_iter().map(summary_of))
                    .collect(),
            );
            if !merged.is_empty() {
                return merged;
            }
        }
        let canonical: Vec<ArkObjectSummary> = self
            .bridge
            .list_object_summaries(DEFAULT_ARK_TYPE_ID)
            .unwrap_or_default()
            .iter()
            .filter_map(de)
            .collect();
        if !canonical.is_empty() {
            return canonical;
        }
        self.list_objects_for_visible_types(type_ids)
            .into_iter()
            .map(summary_of)
            .collect()
    }

    fn list_all_objects(&mut self) -> Vec<ArkObjectRecord> {
        self.list_objects_for_visible_types(&[])
    }

    /// `loadEntry` — full object + links; deleted objects return None.
    pub fn load_entry(
        &mut self,
        id: &str,
        content_only: bool,
    ) -> Result<Option<Entry>, EngineError> {
        let object = self.bridge.get_object(id)?;
        if object.is_null() {
            return Ok(None);
        }
        let object: ArkObjectRecord = match de(&object) {
            Some(o) => o,
            None => return Ok(None),
        };
        if object.deleted_at.as_ref().map(js_truthy).unwrap_or(false) {
            return Ok(None);
        }
        let links: Vec<ArkObjectLink> = if content_only {
            Vec::new()
        } else {
            // `Promise.all` — a links failure rejects `loadEntry`.
            self.bridge
                .list_object_links()?
                .iter()
                .filter_map(de)
                .collect()
        };
        Ok(Some(map_ark_object_to_entry(&object, &links, None)))
    }

    /// `loadListableEntry`.
    pub fn load_listable_entry(
        &mut self,
        id: &str,
        type_id_hint: Option<&str>,
        visible_type_ids: &[String],
    ) -> Result<Option<Entry>, EngineError> {
        if let Some(hint) = type_id_hint {
            if hint != crate::mapping::SYSTEM_TYPE_COLLECTION_ID
                && !should_include_type_in_eden_list_for_live_update(
                    hint,
                    &Value::Object(Default::default()),
                    visible_type_ids,
                )
            {
                return Ok(None);
            }
        }
        let Some(entry) = self.load_entry(id, false)? else {
            return Ok(None);
        };
        if entry.deleted_at.map(|v| v != 0).unwrap_or(false) {
            return Ok(None);
        }
        let props = parse_header_props_json(entry.header_props_json.as_deref());
        let listable = should_include_type_in_eden_list_for_live_update(
            entry.type_id.as_deref().unwrap_or(""),
            &Value::Object(props),
            visible_type_ids,
        );
        Ok(listable.then_some(entry))
    }

    /// `listEntries` — summaries filtered like the Vue list view.
    pub fn list_entries(&mut self, visible_type_ids: &[String]) -> Result<Vec<Entry>, EngineError> {
        let objects = self.list_object_summaries_for_visible_types(visible_type_ids);
        let mut entries: Vec<Entry> = objects
            .iter()
            .filter(|o| !o.deleted_at.as_ref().map(js_truthy).unwrap_or(false))
            .filter(|o| !is_memoria_metadata(&o.props_json))
            .filter(|o| !is_legacy_dated_journal_object(&o.type_id, &o.title))
            .filter(|o| should_include_object_in_eden_list(&o.type_id, &o.props_json))
            .map(|o| map_ark_object_summary_to_entry(o, &[], None))
            .collect();
        entries.sort_by_key(|e| std::cmp::Reverse(e.updated_at));
        Ok(entries)
    }

    /// `listAllEntries` — full objects + related links.
    pub fn list_all_entries(&mut self) -> Result<Vec<Entry>, EngineError> {
        let objects = self.list_all_objects();
        // Vue: `Promise.all([listAllObjects(), listObjectLinks()])` — a
        // links failure rejects `listAllEntries`.
        let links: Vec<ArkObjectLink> = self
            .bridge
            .list_object_links()?
            .iter()
            .filter_map(de)
            .collect();
        let mut entries: Vec<Entry> = objects
            .iter()
            .filter(|o| !o.deleted_at.as_ref().map(js_truthy).unwrap_or(false))
            .filter(|o| !is_memoria_metadata(&o.props_json))
            .filter(|o| should_include_object_in_eden_list(&o.type_id, &o.props_json))
            .map(|o| map_ark_object_to_entry(o, &links, None))
            .collect();
        entries.sort_by_key(|e| std::cmp::Reverse(e.updated_at));
        Ok(entries)
    }
}

fn summary_of(o: ArkObjectRecord) -> ArkObjectSummary {
    ArkObjectSummary {
        id: o.id,
        type_id: o.type_id,
        title: o.title,
        props_json: o.props_json,
        created_at: o.created_at,
        updated_at: o.updated_at,
        deleted_at: o.deleted_at,
        extra: Default::default(),
    }
}
