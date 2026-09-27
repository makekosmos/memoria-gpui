//! Port of `obsidianVaultExportAssets.ts` — rewrites note image references
//! (body + image-type header fields) to copied asset paths and records
//! uncopyable references in the export manifest.

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

use crate::model::{Entry, NoteType};

use super::export::ObsidianExportFile;
use super::export_paths::{
    allocate_unique_export_path, derive_obsidian_export_asset_plan, relative_path_between,
};
use super::header_image::{
    export_image_field_ids, parse_entry_header_props_json_loose,
    resolve_header_image_asset_reference, rewrite_header_image_value,
};
use super::images::{parse_markdown_image_target, title_from_path, IMAGE_MARKDOWN_RE};

/// `ObsidianVaultAssetManifestEntry`.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct ObsidianVaultAssetManifestEntry {
    pub source: String,
    #[serde(rename = "sourcePath")]
    pub source_path: Option<String>,
    #[serde(rename = "noteRelativePath")]
    pub note_relative_path: String,
    #[serde(rename = "targetRelativePath")]
    pub target_relative_path: String,
    #[serde(rename = "rewrittenRelativePath")]
    pub rewritten_relative_path: Option<String>,
    pub copyable: bool,
    pub limitation: Option<String>,
}

/// Shared mutable state for one export run. `asset_files` keeps TS `Map`
/// insertion order; `asset_files_by_source` maps asset keys to indexes.
#[derive(Default)]
pub struct ExportAssetContext {
    pub used_paths: HashSet<String>,
    pub asset_files: Vec<ObsidianExportFile>,
    pub asset_files_by_source: HashMap<String, usize>,
    pub asset_manifest_entries: Vec<ObsidianVaultAssetManifestEntry>,
    pub seen_asset_manifest_keys: HashSet<String>,
}

/// `rewriteObsidianExportHeaderImageReferences` — returns the rewritten
/// `header_props_json` string, or `None` when nothing changed.
pub fn rewrite_obsidian_export_header_image_references(
    entry: &Entry,
    note_type: Option<&NoteType>,
    note_relative_path: &str,
    entries_by_id: &HashMap<String, Entry>,
    context: &mut ExportAssetContext,
) -> Option<String> {
    let header_props = parse_entry_header_props_json_loose(entry.header_props_json.as_deref())?;
    let image_field_ids = export_image_field_ids(entry, note_type);
    if image_field_ids.is_empty() {
        return None;
    }

    let mut rewritten_header_props: Option<Map<String, Value>> = None;
    for field_id in &image_field_ids {
        if !header_props.contains_key(field_id) {
            continue;
        }
        let Some(reference) = resolve_header_image_asset_reference(
            field_id,
            &header_props[field_id],
            &header_props,
            entries_by_id,
        ) else {
            continue;
        };
        let Some(registered) = register_obsidian_export_asset_reference(
            &reference.source,
            note_relative_path,
            reference.preferred_file_name.as_deref(),
            reference.mime_type.as_deref(),
            context,
        ) else {
            continue;
        };
        let props = rewritten_header_props.get_or_insert_with(|| header_props.clone());
        props.insert(
            field_id.clone(),
            rewrite_header_image_value(&header_props[field_id], &registered.rewritten),
        );
    }
    rewritten_header_props
        .map(|props| serde_json::to_string(&Value::Object(props)).unwrap_or_default())
}

/// `rewriteObsidianExportImageReferences` — rewrites `![]()` / `![[…]]`
/// references that map to copyable local assets; others keep the raw match.
pub fn rewrite_obsidian_export_image_references(
    markdown: &str,
    note_relative_path: &str,
    context: &mut ExportAssetContext,
) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut cursor = 0;
    for m in IMAGE_MARKDOWN_RE.captures_iter(markdown) {
        let whole = m.get(0).expect("full match");
        out.push_str(&markdown[cursor..whole.start()]);
        cursor = whole.end();
        let alt = m.get(1).map(|g| g.as_str()).unwrap_or_default();
        let raw_target = m
            .get(2)
            .or_else(|| m.get(3))
            .map(|g| g.as_str())
            .unwrap_or_default();
        let source = parse_markdown_image_target(raw_target);
        let rewritten = register_obsidian_export_asset_reference(
            &source,
            note_relative_path,
            None,
            None,
            context,
        )
        .map(|registered| {
            let label = if alt.is_empty() {
                title_from_path(&registered.target)
            } else {
                alt.to_string()
            };
            format!("![{}]({})", label.trim(), registered.rewritten)
        });
        out.push_str(rewritten.as_deref().unwrap_or(whole.as_str()));
    }
    out.push_str(&markdown[cursor..]);
    out
}

pub(crate) struct RegisteredAsset {
    pub(crate) target: String,
    pub(crate) rewritten: String,
}

/// `registerObsidianExportAssetReference` — plans the target path, dedupes on
/// the canonical source key, manifests non-copyable sources.
pub(crate) fn register_obsidian_export_asset_reference(
    source: &str,
    note_relative_path: &str,
    preferred_file_name: Option<&str>,
    mime_type: Option<&str>,
    context: &mut ExportAssetContext,
) -> Option<RegisteredAsset> {
    let note_dir = note_relative_path
        .rfind('/')
        .map(|i| &note_relative_path[..i])
        .unwrap_or_default();
    let plan = derive_obsidian_export_asset_plan(source, preferred_file_name, mime_type)?;

    let Some(copyable_source_path) = plan.source_path else {
        let manifest_key = format!("{source}\0{note_dir}\0{}", plan.target_relative_path);
        if context.seen_asset_manifest_keys.insert(manifest_key) {
            context.asset_manifest_entries.push(ObsidianVaultAssetManifestEntry {
                source: source.to_string(),
                source_path: None,
                note_relative_path: note_relative_path.to_string(),
                target_relative_path: plan.target_relative_path,
                rewritten_relative_path: None,
                copyable: false,
                limitation: Some(
                    "The reference could not be resolved to a safe local source path, so the original Markdown reference was kept unchanged."
                        .into(),
                ),
            });
        }
        return None;
    };

    let asset_key = plan
        .asset_key
        .unwrap_or_else(|| copyable_source_path.clone());
    let existing_index = context.asset_files_by_source.get(&asset_key).copied();
    let target_relative_path = match existing_index {
        Some(index) => context.asset_files[index].relative_path.clone(),
        None => {
            allocate_unique_export_path(&plan.target_relative_path, &mut context.used_paths).ok()?
        }
    };
    let rewritten_relative_path = relative_path_between(note_dir, &target_relative_path);
    if existing_index.is_none() {
        context
            .asset_files_by_source
            .insert(asset_key, context.asset_files.len());
        context.asset_files.push(ObsidianExportFile {
            relative_path: target_relative_path.clone(),
            content: None,
            source_path: Some(copyable_source_path),
        });
    }
    Some(RegisteredAsset {
        target: target_relative_path,
        rewritten: rewritten_relative_path,
    })
}
