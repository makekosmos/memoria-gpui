//! Shared private helpers for `entry_api` (+its `save`/`load` children):
//! `ensureList` serde decode, metadata/journal filters, `dedupeById`.

use serde_json::Value;

use crate::mapping::{memoria_record_kind, SYSTEM_TYPE_JOURNAL_ID, SYSTEM_TYPE_NOTE_ID};
use crate::model::{ArkObjectRecord, ArkObjectSummary};

pub fn de<T: serde::de::DeserializeOwned>(value: &Value) -> Option<T> {
    serde_json::from_value(value.clone()).ok()
}

pub fn is_memoria_metadata(props_json: &Value) -> bool {
    matches!(memoria_record_kind(props_json), Some(Value::String(_)))
}

/// `isLegacyDatedJournalObject` — `YYYY-MM-DD` titles under journal/note type.
pub fn is_legacy_dated_journal_object(type_id: &str, title: &str) -> bool {
    if type_id != SYSTEM_TYPE_JOURNAL_ID && type_id != SYSTEM_TYPE_NOTE_ID {
        return false;
    }
    let t = title.trim();
    t.len() == 10
        && t.as_bytes()[4] == b'-'
        && t.as_bytes()[7] == b'-'
        && t.bytes()
            .enumerate()
            .all(|(i, b)| matches!(i, 4 | 7) || b.is_ascii_digit())
}

pub fn dedupe_by_id<T: HasId>(items: Vec<T>) -> Vec<T> {
    let mut seen = std::collections::HashSet::new();
    items
        .into_iter()
        .filter(|o| seen.insert(o.id().to_string()))
        .collect()
}
pub trait HasId {
    fn id(&self) -> &str;
}
impl HasId for ArkObjectRecord {
    fn id(&self) -> &str {
        &self.id
    }
}
impl HasId for ArkObjectSummary {
    fn id(&self) -> &str {
        &self.id
    }
}
