//! `kepler-bubble-api.ts` migration half — `migrateBubble`,
//! `migrateLocalBubbles`, `migrateJournalEntries` plus the deterministic
//! `migrationObjectId`/`sameMigratedBubble`/`stableJson` helpers. Re-exported
//! through `crate::store::bubble_api` so call sites keep one import path.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::diary::{bubble_occurrence_millis, normalize_tags, BubbleTimelineNode};
use crate::model::ArkObjectRecord;
use crate::store::transport::{ArkBridge, EngineError};

use super::bubble_api::{bubble_object, engine_err, is_bubble_object, BubbleApi};

impl<B: ArkBridge> BubbleApi<B> {
    /// `migrateBubble` — deterministic `eden-bubble-<sha256[..32]>` id,
    /// read-back verified with key-order-insensitive comparison.
    pub fn migrate_bubble(
        &self,
        namespace: &str,
        source_id: &str,
        source: &BubbleTimelineNode,
    ) -> Result<String, EngineError> {
        if bubble_occurrence_millis(source).is_none() {
            return Err(engine_err("Unresolved legacy bubble date"));
        }
        let id = migration_object_id(namespace, source_id);
        let expected = bubble_object(
            &BubbleTimelineNode {
                id: id.clone(),
                updated_at: source
                    .updated_at
                    .clone()
                    .or_else(|| source.created_at.clone()),
                ..source.clone()
            },
            None,
        )?;
        self.bridge().upsert_object(expected.clone())?;
        let read_back: Option<ArkObjectRecord> =
            serde_json::from_value(self.bridge().get_object(&id)?).ok();
        if !same_migrated_bubble(read_back.as_ref(), &expected) {
            return Err(engine_err("Bubble migration read-back mismatch"));
        }
        Ok(id)
    }
}

/// `startDiary`'s migration pass — `migrateLocalBubbles` (the
/// `memoria-bubble-diary-local-bubbles` blob) then `migrateJournalEntries`
/// (dated legacy journal/`note_obj` entries). Returns `Some(remaining)`
/// when a valid local blob was processed (caller writes it back, or removes
/// the key when empty); `None` when there was no valid blob to touch.
pub fn migrate_diary<B: ArkBridge>(
    api: &BubbleApi<B>,
    entry_api: &mut crate::store::entry_api::EntryApi<B>,
    local_bubbles_json: Option<&Value>,
) -> Result<Option<Vec<Value>>, EngineError> {
    let remaining = local_bubbles_json.and_then(|blob| migrate_local_blob(api, blob));

    // `readJournalSourceEntries` — `listAllEntries` (unfiltered full
    // objects): `listEntries` deliberately excludes dated journal objects
    // from the feed, which is exactly the set the migration must find.
    let mut legacy: Vec<crate::model::Entry> = entry_api
        .list_all_entries()?
        .into_iter()
        .filter(crate::diary::is_legacy_dated_journal_entry)
        .collect();
    legacy.sort_by(|l, r| {
        r.title
            .cmp(&l.title)
            .then_with(|| r.created_at.cmp(&l.created_at))
    });
    let mut deletable: Vec<String> = Vec::new();
    for entry in &legacy {
        // `content_loaded === false` → `window.api.loadEntry` for the body;
        // Vue falls back to the unloaded entry (`?? entry`) on failure.
        let loaded = if entry.content_loaded == Some(false) {
            entry_api
                .load_entry(&entry.id, true)
                .ok()
                .flatten()
                .unwrap_or_else(|| entry.clone())
        } else {
            entry.clone()
        };
        if migrate_journal_entry(api, &loaded) {
            deletable.push(entry.id.clone());
        }
    }
    for id in deletable {
        let _ = entry_api.delete_entry(&id);
    }
    Ok(remaining)
}

/// `migrateLocalBubbles` — each blob source normalizes + resolves an
/// occurrence before `migrateBubble("local-storage-v1", …)`; failures stay in
/// `remaining` (Vue writes them back to the storage key). `None` when the
/// blob isn't a `{version:1, bubbles:[…]}` record — Vue early-returns and
/// leaves the key alone.
pub fn migrate_local_blob<B: ArkBridge>(api: &BubbleApi<B>, blob: &Value) -> Option<Vec<Value>> {
    let record = crate::diary::local_bubbles_record(blob)?;
    let mut remaining: Vec<Value> = Vec::new();
    for source in record
        .get("bubbles")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        let normalized = crate::diary::normalize_local_bubbles(&Value::Array(vec![source.clone()]));
        match normalized.into_iter().next() {
            Some(bubble) if bubble_occurrence_millis(&bubble).is_some() => {
                // `JSON.stringify(source)` — the raw record, not the node.
                let source_id = format!(
                    "{}:{}",
                    bubble.id,
                    serde_json::to_string(&source).unwrap_or_default()
                );
                if api
                    .migrate_bubble("local-storage-v1", &source_id, &bubble)
                    .is_err()
                {
                    remaining.push(source);
                }
            }
            _ => remaining.push(source),
        }
    }
    Some(remaining)
}

/// Per-entry tail of `migrateJournalEntries` — every block bubble must
/// migrate before the source entry becomes deletable. Shared with the demo
/// backend, which feeds its in-memory entry list through the same rules.
pub fn migrate_journal_entry<B: ArkBridge>(
    api: &BubbleApi<B>,
    entry: &crate::model::Entry,
) -> bool {
    let bubbles = crate::diary::create_journal_bubbles_from_entry(entry);
    if bubbles.is_empty() {
        return false;
    }
    bubbles.iter().all(|bubble| {
        api.migrate_bubble(&format!("dated-journal:{}", entry.id), &bubble.id, bubble)
            .is_ok()
    })
}

/// `migrationObjectId` — `eden-bubble-` + first 32 hex chars of
/// SHA-256(`${namespace}\0${sourceId}`).
fn migration_object_id(namespace: &str, source_id: &str) -> String {
    let digest = Sha256::digest(format!("{namespace}\0{source_id}").as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("eden-bubble-{}", &hex[..32])
}

/// `sameMigratedBubble` — id, createdAt, kind, normalized tags and content
/// (key-order-insensitive) must all match.
fn same_migrated_bubble(object: Option<&ArkObjectRecord>, expected: &Value) -> bool {
    let Some(object) = object else { return false };
    if !is_bubble_object(object) {
        return false;
    }
    object.id
        == expected
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
        && serde_json::to_string(&object.created_at).unwrap_or_default()
            == serde_json::to_string(expected.get("createdAt").unwrap_or(&Value::Null))
                .unwrap_or_default()
        && object.props_json.get("bubble_kind")
            == expected.get("propsJson").and_then(|p| p.get("bubble_kind"))
        && serde_json::to_string(&normalize_tags(
            object.props_json.get("tags").unwrap_or(&Value::Null),
        ))
        .unwrap_or_default()
            == serde_json::to_string(
                expected
                    .get("propsJson")
                    .and_then(|p| p.get("tags"))
                    .unwrap_or(&Value::Null),
            )
            .unwrap_or_default()
        && stable_json(&object.content_json)
            == stable_json(expected.get("contentJson").unwrap_or(&Value::Null))
}

/// `stableJson` — serialize with every object's keys sorted (deep), so the
/// comparison ignores Engine key reordering.
fn stable_json(value: &Value) -> String {
    fn sorted(value: &Value) -> Value {
        match value {
            Value::Object(map) => {
                let mut entries: Vec<(&String, &Value)> = map.iter().collect();
                entries.sort_by(|a, b| a.0.cmp(b.0));
                let mut out = Map::new();
                for (k, v) in entries {
                    out.insert(k.clone(), sorted(v));
                }
                Value::Object(out)
            }
            Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
            other => other.clone(),
        }
    }
    serde_json::to_string(&sorted(value)).unwrap_or_default()
}
