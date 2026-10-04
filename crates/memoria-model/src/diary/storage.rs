//! `bubbleDiaryModel.ts` storage section — `normalizeLocalBubbles`,
//! `normalizeBubbleDateKey`/`normalizeBubbleSortKey`/`normalizeTiptapDoc`,
//! `encodeLocalBubblesStorage`/`decodeLocalBubblesStorage`. The blob is the
//! `memoria-bubble-diary-local-bubbles` JSON payload kept by the Vue diary.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};

use super::text::plain_text_to_tiptap_doc;
use super::{normalize_bubble_kind, BubbleTimelineNode, DATE_KEY_PATTERN};

/// `normalizeBubbleDateKey` on `Value`s — `date` wins, else `time` may carry
/// a `YYYY-MM-DD` (journal bubbles store the date there).
pub fn normalize_bubble_date_key(date: &Value, time: &Value) -> Option<String> {
    for value in [date, time] {
        if let Some(s) = value.as_str() {
            if DATE_KEY_PATTERN.is_match(s) {
                return Some(s.to_string());
            }
        }
    }
    None
}

/// Same check on plain `&str`s (timeline callers).
pub fn normalize_bubble_date_key_str(date: Option<&str>, time: Option<&str>) -> Option<String> {
    for s in [date, time].into_iter().flatten() {
        if DATE_KEY_PATTERN.is_match(s) {
            return Some(s.to_string());
        }
    }
    None
}

/// `normalizeBubbleSortKey` — journal ids (`journal-…-<i>`) get
/// UTC-midnight + index; otherwise a finite number wins; otherwise the
/// date's UTC midnight; else none.
pub fn normalize_bubble_sort_key(
    value: &Value,
    date: &Value,
    time: &Value,
    id: &Value,
) -> Option<f64> {
    static JOURNAL_INDEX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^journal-.+-(\d+)$").expect("journal idx re"));
    let date_key = normalize_bubble_date_key(date, time);
    let journal_index = id
        .as_str()
        .and_then(|s| JOURNAL_INDEX.captures(s))
        .and_then(|caps| caps[1].parse::<f64>().ok())
        .filter(|n| n.is_finite());
    if let (Some(key), Some(index)) = (date_key.as_deref(), journal_index) {
        return crate::time::iso_to_millis(&format!("{key}T00:00:00.000Z"))
            .map(|ms| ms as f64 + index);
    }
    if let Some(n) = value.as_f64() {
        if n.is_finite() {
            return Some(n);
        }
    }
    let key = date_key?;
    crate::time::iso_to_millis(&format!("{key}T00:00:00.000Z")).map(|ms| ms as f64)
}

/// `normalizeTiptapDoc` — keep a `{type:"doc"}` record verbatim (unknown
/// nodes/attrs survive), else fall back to a plain-text doc.
pub fn normalize_tiptap_doc(value: &Value, fallback_text: &str) -> Value {
    if value.get("type").and_then(Value::as_str) == Some("doc") {
        return value.clone();
    }
    plain_text_to_tiptap_doc(fallback_text)
}

/// `normalizeLocalBubbles` — structural validation; dropped fields
/// (`createdAt`/`updatedAt`/`parentId`) do not survive, matching Vue.
pub fn normalize_local_bubbles(value: &Value) -> Vec<BubbleTimelineNode> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|node| {
            let record = node.as_object()?;
            let id = record.get("id")?.as_str()?;
            let time = record.get("time")?.as_str()?;
            let text = record.get("text")?.as_str()?;
            let tags_raw = record.get("tags")?.as_array()?;
            let tags: Vec<String> = tags_raw
                .iter()
                .filter_map(|t| t.as_str().map(str::to_string))
                .collect();
            let null = Value::Null;
            let date_v = record.get("date").unwrap_or(&null);
            let time_v = record.get("time").unwrap_or(&null);
            Some(BubbleTimelineNode {
                id: id.to_string(),
                date: normalize_bubble_date_key(date_v, time_v),
                time: time.to_string(),
                sort_key: normalize_bubble_sort_key(
                    record.get("sortKey").unwrap_or(&null),
                    date_v,
                    time_v,
                    record.get("id").unwrap_or(&null),
                ),
                text: text.to_string(),
                content_json: Some(normalize_tiptap_doc(
                    record.get("contentJson").unwrap_or(&null),
                    text,
                )),
                tags,
                kind: record
                    .get("kind")
                    .map(normalize_bubble_kind)
                    .unwrap_or_default(),
                ..Default::default()
            })
        })
        .collect()
}

/// `decodeLocalBubblesStorage` — `{version:1, bubbles:[…]}` only.
pub fn decode_local_bubbles_storage(value: &Value) -> Vec<BubbleTimelineNode> {
    let Some(record) = value.as_object() else {
        return Vec::new();
    };
    if record.get("version").and_then(Value::as_u64) != Some(super::LOCAL_BUBBLES_STORAGE_VERSION) {
        return Vec::new();
    }
    normalize_local_bubbles(record.get("bubbles").unwrap_or(&Value::Null))
}

/// The raw storage record — `Some` only for `{version:1, bubbles:[…]}`;
/// anything else is an invalid/absent blob that callers must leave
/// untouched (Vue `migrateLocalBubbles` early-returns on it).
pub fn local_bubbles_record(value: &Value) -> Option<&Map<String, Value>> {
    let record = value.as_object()?;
    if record.get("version").and_then(Value::as_u64) != Some(super::LOCAL_BUBBLES_STORAGE_VERSION) {
        return None;
    }
    record.get("bubbles")?.as_array()?;
    Some(record)
}

/// Raw `bubbles` items of a versioned blob — the `record.bubbles` array the
/// migration loop iterates (raw sources, not normalized nodes).
pub fn decode_local_bubbles_sources(value: &Value) -> Vec<Value> {
    local_bubbles_record(value)
        .and_then(|record| record.get("bubbles"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// `encodeLocalBubblesStorage` — `JSON.stringify` key order:
/// `version`, `journalImported`, `bubbles`.
pub fn encode_local_bubbles_storage(
    bubbles: &[BubbleTimelineNode],
    journal_imported: bool,
) -> String {
    let mut map = Map::new();
    map.insert(
        "version".into(),
        Value::from(super::LOCAL_BUBBLES_STORAGE_VERSION),
    );
    map.insert("journalImported".into(), Value::from(journal_imported));
    map.insert(
        "bubbles".into(),
        Value::Array(
            bubbles
                .iter()
                .map(|b| serde_json::to_value(b).unwrap_or(Value::Null))
                .collect(),
        ),
    );
    serde_json::to_string(&Value::Object(map)).unwrap_or_default()
}

/// `new Set(value.filter(…string).map(trim→lowercase).filter(Boolean))` —
/// the `normalizeTags` shared by the API layer (`ru` locale is equivalent to
/// default Unicode lowercase for ASCII tags).
pub fn normalize_tags(value: &Value) -> Vec<String> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    items
        .iter()
        .filter_map(Value::as_str)
        .map(|tag| tag.trim().to_lowercase())
        .filter(|tag| !tag.is_empty())
        .filter(|tag| seen.insert(tag.clone()))
        .collect()
}
