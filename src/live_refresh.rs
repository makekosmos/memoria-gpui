//! Port of `src/store/liveRefresh.ts` — decide whether a remote change should
//! be applied to the open editor. Guards: identical content (self-echo) and
//! a dirty editor (don't clobber user input).

use serde_json::{Map, Value};

use crate::content::read_entry_markdown;
use crate::model::Entry;

/// `RemoteEntryDecision`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteEntryDecision {
    Apply,
    SkipSameContent,
    SkipDirty,
}

/// `ShouldApplyRemoteEntryParams` — `current_entry` is the full local baseline
/// (title/type/header/folder/deleted/schema/created + `content_json`).
pub struct RemoteEntryParams<'a> {
    pub fresh: &'a Entry,
    pub current_content_json: &'a str,
    pub current_entry: Option<&'a Entry>,
    pub is_editor_dirty: bool,
}

/// `isOlderRemoteEntry` — true when the remote revision predates the open one.
pub fn is_older_remote_entry(fresh: &Entry, current: &Entry) -> bool {
    fresh.updated_at < current.updated_at
}

/// `shouldApplyRemoteEntry` — content-equality guard, then dirty guard.
pub fn should_apply_remote_entry(params: &RemoteEntryParams<'_>) -> RemoteEntryDecision {
    let fresh = params.fresh;
    let current_content: Value =
        serde_json::from_str(params.current_content_json).unwrap_or(Value::Null);

    if params.current_entry.is_none()
        && read_entry_markdown(&fresh.content_json_value()) == read_entry_markdown(&current_content)
    {
        return RemoteEntryDecision::SkipSameContent;
    }

    let fresh_fp = entry_visible_fingerprint_parts(
        &fresh.title,
        fresh.type_id.as_deref(),
        fresh.header_layout.as_deref(),
        fresh.header_props_json.as_deref(),
        &fresh.content_json,
        fresh.folder_id.as_deref(),
        fresh.deleted_at,
        fresh.schema_version,
        fresh.created_at,
    );
    let fallback;
    let current = match params.current_entry {
        Some(entry) => entry,
        None => {
            fallback = Entry {
                title: fresh.title.clone(),
                type_id: fresh.type_id.clone(),
                header_layout: fresh.header_layout.clone(),
                header_props_json: fresh.header_props_json.clone(),
                content_json: params.current_content_json.to_string(),
                folder_id: fresh.folder_id.clone(),
                deleted_at: fresh.deleted_at,
                schema_version: fresh.schema_version,
                created_at: fresh.created_at,
                ..Default::default()
            };
            &fallback
        }
    };
    let current_fp = entry_visible_fingerprint(current);

    if fresh_fp == current_fp {
        return RemoteEntryDecision::SkipSameContent;
    }
    if params.is_editor_dirty {
        return RemoteEntryDecision::SkipDirty;
    }
    RemoteEntryDecision::Apply
}

/// `canonicalize` — recursively sort object keys. NOTE: JS `localeCompare`
/// orders mixed-case keys differently than codepoint order; Memoria keys are
/// lowercase/camelCase ASCII, so `sort` (byte order) matches in practice and
/// is only ever compared against itself in this crate.
fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(canonicalize).collect()),
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = Map::new();
            for key in keys {
                out.insert(key.clone(), canonicalize(&map[key]));
            }
            Value::Object(out)
        }
        scalar => scalar.clone(),
    }
}

/// `parseJson` — `JSON.parse(value || "null")`; on failure the raw string
/// (empty/absent ⇒ null, matching `value || null` fallback).
fn parse_json(value: Option<&str>) -> Value {
    match value {
        Some(s) if !s.is_empty() => {
            serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.to_string()))
        }
        _ => Value::Null,
    }
}

/// `entryVisibleFingerprint` over explicit parts.
#[allow(clippy::too_many_arguments)]
pub fn entry_visible_fingerprint_parts(
    title: &str,
    type_id: Option<&str>,
    header_layout: Option<&str>,
    header_props_json: Option<&str>,
    content_json: &str,
    folder_id: Option<&str>,
    deleted_at: Option<i64>,
    schema_version: Option<i64>,
    created_at: i64,
) -> String {
    let mut map = Map::new();
    map.insert("title".into(), Value::from(title));
    map.insert(
        "type_id".into(),
        type_id.map(Value::from).unwrap_or(Value::Null),
    );
    map.insert(
        "header_layout".into(),
        header_layout.map(Value::from).unwrap_or(Value::Null),
    );
    map.insert("header_props".into(), parse_json(header_props_json));
    map.insert("content".into(), parse_json(Some(content_json)));
    map.insert(
        "folder_id".into(),
        folder_id.map(Value::from).unwrap_or(Value::Null),
    );
    map.insert(
        "deleted_at".into(),
        deleted_at.map(Value::from).unwrap_or(Value::Null),
    );
    map.insert(
        "schema_version".into(),
        schema_version.map(Value::from).unwrap_or(Value::Null),
    );
    map.insert("created_at".into(), Value::from(created_at));
    serde_json::to_string(&canonicalize(&Value::Object(map))).unwrap_or_default()
}

/// `entryVisibleFingerprint(entry)`.
pub fn entry_visible_fingerprint(entry: &Entry) -> String {
    entry_visible_fingerprint_parts(
        &entry.title,
        entry.type_id.as_deref(),
        entry.header_layout.as_deref(),
        entry.header_props_json.as_deref(),
        &entry.content_json,
        entry.folder_id.as_deref(),
        entry.deleted_at,
        entry.schema_version,
        entry.created_at,
    )
}

impl Entry {
    /// `content_json` parsed to a `Value` (Null when absent/invalid).
    pub fn content_json_value(&self) -> Value {
        serde_json::from_str(&self.content_json).unwrap_or(Value::Null)
    }
}
