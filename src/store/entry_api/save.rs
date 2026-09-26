//! `validateEntryTypeMetadata`, `isStaleEntryWrite`, `syncRelatedLinks`,
//! `saveEntry`, `deleteEntry`, `searchEntries` — the mutating half of
//! `kepler-entry-api.ts`.

use super::helpers::de;
use super::*;

impl<B: ArkBridge> EntryApi<B> {
    /// `validateEntryTypeMetadata`.
    fn validate_entry_type_metadata(
        &mut self,
        entry: &Entry,
    ) -> Result<Option<SaveFail>, EngineError> {
        let Some(type_id) = entry.type_id.as_deref() else {
            return Ok(None);
        };
        let Some(note_type) = self.note_types.get_note_type_by_id(type_id)? else {
            return Ok(Some(SaveFail::invalid_type(
                "Тип заметки больше не существует",
            )));
        };
        let parsed: Value =
            match serde_json::from_str(entry.header_props_json.as_deref().unwrap_or("{}")) {
                Ok(v) => v,
                Err(_) => {
                    return Ok(Some(SaveFail::invalid_type(
                        "Верхушка заметки сохранена в неверном формате",
                    )))
                }
            };
        if validate_header_props(Some(&note_type), &parsed).is_err() {
            return Ok(Some(SaveFail::invalid_type(
                "Структура верхушки заметки больше не соответствует типу",
            )));
        }
        Ok(None)
    }

    fn ensure_entry_type_available(&mut self, note_type_id: &str) -> Result<bool, EngineError> {
        if is_system_type(note_type_id) {
            return Ok(true);
        }
        Ok(self.note_types.get_note_type_by_id(note_type_id)?.is_some())
    }

    /// `isStaleEntryWrite` — the stored object moved ahead AND diverges in
    /// content, title or props.
    fn is_stale_entry_write(
        entry: &Entry,
        existing: &ArkObjectRecord,
        next_content: &Value,
    ) -> bool {
        let existing_updated = ark_timestamp_to_millis(&existing.updated_at, 0);
        if existing_updated <= entry.updated_at {
            return false;
        }
        // Vue compares `JSON.stringify` output — key order is significant.
        let next_object = map_entry_to_ark_object(entry);
        let or_empty_obj = |v: &Value| if v.is_null() { json!({}) } else { v.clone() };
        serde_json::to_string(&existing.content_json).unwrap_or_default()
            != serde_json::to_string(next_content).unwrap_or_default()
            || existing.title != next_object.title
            || serde_json::to_string(&or_empty_obj(&existing.props_json)).unwrap_or_default()
                != serde_json::to_string(&or_empty_obj(&next_object.props_json)).unwrap_or_default()
    }

    /// `syncRelatedLinks` — delete removed `related` links, upsert new ones
    /// under `{entry}:related:{target}` ids.
    fn sync_related_links(&mut self, entry: &Entry) -> Result<(), EngineError> {
        let header_props = parse_header_props_json(entry.header_props_json.as_deref());
        let related: Vec<String> = match header_props.get("related_notes") {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
            _ => Vec::new(),
        };
        let existing: Vec<ArkObjectLink> = self
            .bridge
            .list_object_links()?
            .iter()
            .filter_map(de)
            .collect();
        let owned: Vec<&ArkObjectLink> = existing
            .iter()
            .filter(|l| l.source_object_id == entry.id && l.link_type == "related")
            .collect();
        for link in &owned {
            if !related.iter().any(|t| t == &link.target_object_id) {
                self.bridge.delete_object_link(&link.id)?;
            }
        }
        for target in &related {
            if owned.iter().any(|l| &l.target_object_id == target) {
                continue;
            }
            self.bridge.upsert_object_link(serde_json::json!({
                "id": format!("{}:related:{}", entry.id, target),
                "sourceObjectId": entry.id,
                "targetObjectId": target,
                "linkType": "related",
                "createdAt": millis_to_ark_timestamp(Some(entry.updated_at)),
            }))?;
        }
        Ok(())
    }

    /// `saveEntry` — full guard chain; Engine write happens last.
    pub fn save_entry(
        &mut self,
        entry: &Entry,
    ) -> Result<crate::model::SaveEntryResult, EngineError> {
        let mut normalized = normalize_entry(entry);
        if normalized.type_id.is_none() {
            normalized.type_id = Some(DEFAULT_ARK_TYPE_ID.into());
        }
        if normalized.header_layout.is_none() {
            normalized.header_layout = Some("default".into());
        }
        if let Some(fail) = self.validate_entry_type_metadata(&normalized)? {
            return Ok(fail.into_result());
        }
        let type_id = normalized
            .type_id
            .clone()
            .unwrap_or_else(|| DEFAULT_ARK_TYPE_ID.into());
        if !self.ensure_entry_type_available(&type_id)? {
            return Ok(SaveFail::invalid_type("Тип заметки больше не существует").into_result());
        }
        if normalized.content_loaded == Some(false) {
            return Ok(
                SaveFail::new("content_not_loaded", "Тело заметки ещё не загружено").into_result(),
            );
        }
        let parsed_content: Value =
            match serde_json::from_str(if normalized.content_json.is_empty() {
                "{}"
            } else {
                &normalized.content_json
            }) {
                Ok(v) => v,
                Err(_) => {
                    return Ok(SaveFail::new(
                        "invalid_content_json",
                        "Тело заметки сохранено в неверном формате",
                    )
                    .into_result())
                }
            };
        let normalized_title = normalized.title.trim().to_lowercase();
        let existing_objects = self.list_all_objects();
        if let Some(existing) = existing_objects.iter().find(|o| o.id == normalized.id) {
            if Self::is_stale_entry_write(&normalized, existing, &parsed_content) {
                return Ok(SaveFail::new(
                    "stale_entry",
                    "Заметка уже была обновлена более новой версией",
                )
                .into_result());
            }
        }
        let conflicting = existing_objects.iter().find(|o| {
            o.id != normalized.id
                && !o.deleted_at.as_ref().map(js_truthy).unwrap_or(false)
                && o.title.trim().to_lowercase() == normalized_title
        });
        if let Some(conflicting) = conflicting {
            if !normalized_title.is_empty() {
                return Ok(crate::model::SaveEntryResult::duplicate_title(
                    conflicting.id.clone(),
                    normalized.title.clone(),
                    normalized.folder_id.clone(),
                ));
            }
        }
        let ark_object = map_entry_to_ark_object(&normalized);
        self.bridge.upsert_object(serde_json::json!({
            "id": ark_object.id,
            "typeId": ark_object.type_id,
            "typeVersion": ark_object.type_version,
            "title": ark_object.title,
            "contentJson": ark_object.content_json,
            "propsJson": ark_object.props_json,
            "createdAt": ark_object.created_at,
            "updatedAt": ark_object.updated_at,
            "deletedAt": ark_object.deleted_at.unwrap_or(Value::Null),
        }))?;
        self.sync_related_links(&normalized)?;
        Ok(crate::model::SaveEntryResult::ok(normalized.id))
    }

    /// `deleteEntry`.
    pub fn delete_entry(&mut self, entry_id: &str) -> Result<DeleteEntryResult, EngineError> {
        let existing = self.bridge.get_object(entry_id)?;
        let deleted = !existing.is_null()
            && serde_json::from_value::<ArkObjectRecord>(existing.clone())
                .ok()
                .and_then(|o| o.deleted_at)
                .map(|v| !v.is_null())
                .unwrap_or(false);
        if existing.is_null() || deleted {
            return Ok(DeleteEntryResult {
                ok: false,
                reason: Some("entry_not_found".into()),
                message: Some("Заметка не найдена в ARK".into()),
                ..Default::default()
            });
        }
        self.bridge.delete_object(entry_id)?;
        Ok(DeleteEntryResult {
            ok: true,
            entry_id: Some(entry_id.into()),
            ..Default::default()
        })
    }

    /// `searchEntries` — dedupe by `entryId:line:text`.
    pub fn search_entries(&mut self, query: &str) -> Result<Vec<SearchResult>, EngineError> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let results = match self.bridge.search_objects(query) {
            Ok(v) => v.as_array().cloned().unwrap_or_default(),
            Err(_) => return Ok(Vec::new()),
        };
        let mut seen = std::collections::HashSet::new();
        Ok(results
            .iter()
            .filter_map(de::<SearchResult>)
            .filter(|r| seen.insert(format!("{}:{}:{}", r.entry_id, r.line, r.text)))
            .collect())
    }
}

/// Internal save-failure carrier.
pub(crate) struct SaveFail {
    reason: &'static str,
    message: String,
}

impl SaveFail {
    fn new(reason: &'static str, message: &str) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }
    fn invalid_type(message: &str) -> Self {
        Self::new("invalid_type_metadata", message)
    }
    fn into_result(self) -> crate::model::SaveEntryResult {
        crate::model::SaveEntryResult::failed(self.reason, self.message)
    }
}
