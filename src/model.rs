//! Memoria data models — 1:1 ports of the Vue `Entry` / `NoteType` records and
//! the ARK wire records from `src/lib/kepler-entry-mappers.ts`.
//!
//! `deny_unknown_fields` stays OFF everywhere: unknown wire keys land in
//! `extra` and are written back on save.

pub mod ark;
mod results;

pub use ark::{ArkObjectLink, ArkObjectRecord, ArkObjectSummary, ArkObjectType};
pub use results::*;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Vue `interface Entry` (vite-env.d.ts). Timestamps are epoch millis.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub content_json: String,
    #[serde(default)]
    pub content_loaded: Option<bool>,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
    #[serde(default)]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub type_id: Option<String>,
    #[serde(default)]
    pub header_layout: Option<String>,
    #[serde(default)]
    pub header_props_json: Option<String>,
    #[serde(default)]
    pub schema_version: Option<i64>,
    #[serde(default)]
    pub deleted_at: Option<i64>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `noteTypeFieldSchema` — a single field in a note type definition.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NoteTypeField {
    pub id: String,
    #[serde(default)]
    pub label: String,
    pub kind: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiple: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_object_types: Option<Vec<String>>,
}

/// `headerTemplateSchema`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HeaderTemplate {
    pub kind: String,
    #[serde(
        default,
        rename = "primaryFieldIds",
        skip_serializing_if = "Option::is_none"
    )]
    pub primary_field_ids: Option<Vec<String>>,
    #[serde(
        default,
        rename = "secondaryFieldIds",
        skip_serializing_if = "Option::is_none"
    )]
    pub secondary_field_ids: Option<Vec<String>>,
    #[serde(default, rename = "imageFieldId")]
    pub image_field_id: Option<String>,
}

/// `noteTypeUiSchema` — all fields optional in the parsed payload.
/// `Default` lives in `note_types.rs` (Vue `createDefaultNoteTypeUiSchema`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NoteTypeUiSchema {
    #[serde(default)]
    pub featured_fields: Option<Vec<String>>,
    #[serde(default)]
    pub visible_fields: Option<Vec<String>>,
    #[serde(default)]
    pub hidden_fields: Option<Vec<String>>,
    #[serde(default)]
    pub read_only_fields: Option<Vec<String>>,
    #[serde(default)]
    pub field_order: Option<Vec<String>>,
    #[serde(default)]
    pub header_layout: Option<String>,
    #[serde(default)]
    pub default_layout: Option<String>,
    #[serde(default)]
    pub default_template_id: Option<String>,
    #[serde(default)]
    pub collection_name: Option<String>,
}

/// `noteTypeDefinitionSchema` — `{ fields: [...] }`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NoteTypeDefinition {
    #[serde(default)]
    pub fields: Vec<NoteTypeField>,
}

/// Vue `interface NoteType` — schema/ui-schema are embedded JSON strings.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NoteType {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub schema_json: String,
    #[serde(default)]
    pub header_template_json: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_schema_json: Option<String>,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `ResolvedNoteTypeField` = field + effective visibility flags.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedNoteTypeField {
    pub field: NoteTypeField,
    pub visible: bool,
    pub read_only: bool,
}

/// `NoteTypePresentation`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NoteTypePresentation {
    pub header_layout: String,
    pub featured_fields: Vec<ResolvedNoteTypeField>,
    pub secondary_fields: Vec<ResolvedNoteTypeField>,
    pub description_field: Option<ResolvedNoteTypeField>,
    pub field_order: Vec<String>,
    pub image_field_id: Option<String>,
}

/// Vue `SearchResult` (`{file, line, text, entryId}`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub line: i64,
    #[serde(default)]
    pub text: String,
    #[serde(default, rename = "entryId")]
    pub entry_id: String,
}

/// Vue `SaveEntryResult` union flattened into one record; `reason` values:
/// `duplicate_title`, `invalid_type_metadata`, `invalid_content_json`,
/// `stale_entry`, `content_not_loaded`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SaveEntryResult {
    pub ok: bool,
    #[serde(default, rename = "entryId", skip_serializing_if = "Option::is_none")]
    pub entry_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(
        default,
        rename = "conflictingEntryId",
        skip_serializing_if = "Option::is_none"
    )]
    pub conflicting_entry_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
}

impl SaveEntryResult {
    pub fn ok(entry_id: impl Into<String>) -> Self {
        Self {
            ok: true,
            entry_id: Some(entry_id.into()),
            ..Default::default()
        }
    }

    pub fn failed(reason: &str, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            reason: Some(reason.into()),
            message: Some(message.into()),
            ..Default::default()
        }
    }

    pub fn duplicate_title(
        conflicting_entry_id: impl Into<String>,
        title: impl Into<String>,
        folder_id: Option<String>,
    ) -> Self {
        Self {
            ok: false,
            reason: Some("duplicate_title".into()),
            conflicting_entry_id: Some(conflicting_entry_id.into()),
            title: Some(title.into()),
            folder_id,
            ..Default::default()
        }
    }
}

/// JS truthiness for ARK metadata checks (`!o.deletedAt`, `v ?? null`, …):
/// `null`/`0`/`""`/`false` are falsy, everything else is truthy.
pub fn js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|v| v != 0.0 && !v.is_nan()).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}
