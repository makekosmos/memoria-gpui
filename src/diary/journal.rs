//! Legacy dated-journal import — `isLegacyDatedJournalEntry`,
//! `journalDate`, `createJournalBubblesFromEntry`. A "legacy" entry is a
//! journal/`note_obj`/untyped object whose title is `YYYY-MM-DD`; each top
//! block of its tiptap doc becomes one bubble keyed `journal-{entry}-{i}`.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::model::Entry;

use super::text::create_bubble_from_content;
use super::{BubbleTimelineNode, DATE_KEY_PATTERN};

/// `SYSTEM_TYPE_JOURNAL_ID` / `SYSTEM_TYPE_NOTE_ID`.
pub const SYSTEM_TYPE_JOURNAL_ID: &str = "system-type-journal";
pub const SYSTEM_TYPE_NOTE_ID: &str = "note_obj";

/// `isLegacyDatedJournalEntry` — dated title, journal/note/None type, not
/// deleted.
pub fn is_legacy_dated_journal_entry(entry: &Entry) -> bool {
    let type_ok = matches!(
        entry.type_id.as_deref(),
        Some(SYSTEM_TYPE_JOURNAL_ID) | Some(SYSTEM_TYPE_NOTE_ID) | None
    );
    type_ok && entry.deleted_at.is_none() && DATE_KEY_PATTERN.is_match(entry.title.trim())
}

/// `journalDate` — `YYYY-MM-DD` title prefix, else the UTC date of
/// `created_at || updated_at` (JS `||` → first non-zero).
fn journal_date(entry: &Entry) -> String {
    static TITLE_DATE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}").expect("title date re"));
    if let Some(caps) = TITLE_DATE.captures(entry.title.trim()) {
        return caps[0].to_string();
    }
    let timestamp = if entry.created_at != 0 {
        entry.created_at
    } else {
        entry.updated_at
    };
    let c = crate::local_time::civil_at(timestamp, 0);
    crate::local_time::civil_date_key(c.year, c.month, c.day)
}

/// `createJournalBubblesFromEntry` — one bubble per non-empty top-level
/// block; `sortKey` is the UTC midnight of the journal date plus the block
/// index (repaired on re-import), `time` carries the date string.
pub fn create_journal_bubbles_from_entry(entry: &Entry) -> Vec<BubbleTimelineNode> {
    if !is_legacy_dated_journal_entry(entry) {
        return Vec::new();
    }
    let date = journal_date(entry);
    let midnight = crate::time::iso_to_millis(&format!("{date}T00:00:00.000Z"));
    let base_sort_key = midnight
        .filter(|ms| *ms != 0)
        .map(|ms| ms as f64)
        .unwrap_or_else(|| {
            if entry.created_at != 0 {
                entry.created_at as f64
            } else {
                entry.updated_at as f64
            }
        });
    let content_json: Value = serde_json::from_str(&entry.content_json).unwrap_or(Value::Null);
    let doc = crate::content::read_entry_tiptap_doc(&content_json);
    let blocks = match doc.get("content") {
        Some(Value::Array(blocks)) => blocks.clone(),
        _ => Vec::new(),
    };
    blocks
        .iter()
        .enumerate()
        .filter_map(|(index, block)| {
            let content_json = super::text::doc_from_block(block);
            let text = super::text::bubble_plain_text(&content_json);
            create_bubble_from_content(
                &content_json,
                &format!("journal-{}-{index}", entry.id),
                Some(&date),
                &date,
                Some(base_sort_key + index as f64),
                &text,
            )
        })
        .collect()
}
