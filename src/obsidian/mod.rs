//! Obsidian vault import/export — ports of `src/lib/obsidianVault*.ts` (M8).
//!
//! All data arrives through the Engine (`filesystem.vault.*` ops in cortex);
//! these modules are pure planning/logic — no OS/FS access (KOS-157).

pub mod asset_sources;
pub mod draft;
pub mod export;
pub mod export_assets;
pub mod export_paths;
pub mod frontmatter;
pub mod header_image;
pub mod identity;
pub mod image_index;
pub mod images;
pub mod import_apply;
pub mod import_entry;
pub mod journal_recovery;
pub mod run;
pub mod transaction;
pub mod vault;

pub use draft::*;
pub use export::*;
pub use export_assets::*;
pub use export_paths::*;
pub use image_index::*;
pub use images::*;
pub use journal_recovery::*;
pub use run::*;
pub use transaction::*;
pub use vault::*;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// `ObsidianVaultMarkdownFile` — a vault file as returned by the Engine
/// `filesystem.vault.open` scan (paths are vault-relative + opaque source).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ObsidianVaultMarkdownFile {
    pub path: String,
    #[serde(rename = "relativePath")]
    pub relative_path: String,
    pub name: String,
    pub content: String,
}

/// `ObsidianVaultImageFile`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObsidianVaultImageFile {
    pub path: String,
    #[serde(rename = "relativePath")]
    pub relative_path: String,
    pub name: String,
    #[serde(rename = "fileUrl")]
    pub file_url: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    #[serde(rename = "sizeBytes")]
    pub size_bytes: u64,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
}

/// `ObsidianImportDraft`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ObsidianImportDraft {
    #[serde(rename = "sourcePath")]
    pub source_path: String,
    #[serde(rename = "relativePath")]
    pub relative_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub title: String,
    #[serde(rename = "typeId")]
    pub type_id: String,
    #[serde(rename = "bodyMarkdown")]
    pub body_markdown: String,
    #[serde(rename = "headerProps")]
    pub header_props: Map<String, Value>,
    pub wikilinks: Vec<String>,
    #[serde(rename = "imageRefs")]
    pub image_refs: Vec<String>,
}

/// `ObsidianImageObjectDraft`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ObsidianImageObjectDraft {
    pub id: String,
    pub title: String,
    #[serde(rename = "typeId")]
    pub type_id: String,
    #[serde(rename = "contentJson")]
    pub content_json: Value,
    #[serde(rename = "headerProps")]
    pub header_props: Map<String, Value>,
}

/// `ObsidianRelatedImportPlan`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ObsidianRelatedImportPlan {
    #[serde(rename = "firstPassHeaderProps")]
    pub first_pass_header_props: Map<String, Value>,
    #[serde(rename = "secondPassRelatedIds")]
    pub second_pass_related_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "unresolvedTargets")]
    pub unresolved_targets: Option<Vec<String>>,
}

/// `ObsidianExistingImage` — an image object already in the ARK.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObsidianExistingImage {
    pub id: String,
    pub header_props: Map<String, Value>,
}

/// `ObsidianImageImportItem`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ObsidianImageImportItem {
    pub id: String,
    #[serde(rename = "relativePath")]
    pub relative_path: String,
    #[serde(rename = "legacyId")]
    pub legacy_id: String,
}

/// `ObsidianImageImportReport`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ObsidianImageImportReport {
    pub resolved: Vec<ImageResolution>,
    pub ambiguous: Vec<ImageResolution>,
    pub missing: Vec<ImageResolution>,
    pub invalid: Vec<ImageResolution>,
    pub collisions: Vec<ImageAssetCollision>,
    pub imported: Vec<ObsidianImageImportItem>,
    pub reused: Vec<ObsidianImageImportItem>,
    pub collided: Vec<ObsidianImageImportItem>,
    pub skipped: Vec<ObsidianImageImportItem>,
    pub failed: Vec<ObsidianImageImportItem>,
    #[serde(rename = "blockingCollisions")]
    pub blocking_collisions: Vec<ImageAssetCollision>,
    #[serde(rename = "invalidImagePaths")]
    pub invalid_image_paths: Vec<String>,
    #[serde(rename = "canWrite")]
    pub can_write: bool,
}

/// `ImportObsidianVaultArgs`.
#[derive(Clone, Debug, Default)]
pub struct ImportObsidianVaultArgs {
    pub files: Vec<ObsidianVaultMarkdownFile>,
    pub images: Vec<ObsidianVaultImageFile>,
    pub note_types: Vec<crate::model::NoteType>,
    pub default_type_id: String,
    pub image_type_id: String,
    pub type_id_mapping: Map<String, Value>,
    pub vault_identity: Option<String>,
    pub existing_images: Vec<ObsidianExistingImage>,
}

/// `ImportObsidianVaultResult`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportObsidianVaultResult {
    pub entries: Vec<ObsidianImportDraft>,
    pub images: Vec<ObsidianImageObjectDraft>,
    #[serde(rename = "imageReport")]
    pub image_report: ObsidianImageImportReport,
}
