//! Port of `obsidianVault.ts` — per-file draft parsing, the draft pipeline
//! (`createObsidianVaultImportDrafts`), and the two-pass related-import plan.

use std::collections::{HashMap, HashSet};

use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::model::NoteType;

use super::frontmatter::{
    extract_body_wikilinks, extract_frontmatter_wikilinks, extract_header_props,
    frontmatter_string, is_plain_object, normalize_obsidian_title_target, resolve_type_id,
    split_loose_frontmatter,
};
use super::images::{
    canonical_path_identity, extract_markdown_image_refs, rewrite_image_references,
    title_from_path, ImageAssetIndex, ImageResolution,
};
use super::{
    ImportObsidianVaultArgs, ObsidianImportDraft, ObsidianRelatedImportPlan,
    ObsidianVaultImageFile, ObsidianVaultMarkdownFile,
};

const OBSIDIAN_NOTE_NAMESPACE: Uuid = Uuid::from_u128(0xb7c1cc85_3f6d_5f40_8c89_2a0d8f4b4d6b);

/// `createObsidianVaultImportDrafts` — the draft shape the import pipeline
/// consumes (`name`/`entryId`/reference aliases).
#[derive(Clone, Debug)]
pub struct ObsidianVaultImportDraft {
    pub draft: ObsidianImportDraft,
    pub name: String,
    pub entry_id: Option<String>,
    pub title_references: Vec<String>,
    pub image_references: Vec<String>,
}

/// `createObsidianVaultImportDrafts`.
pub fn create_obsidian_vault_import_drafts(
    files: &[ObsidianVaultMarkdownFile],
    images: &[ObsidianVaultImageFile],
    note_types: &[NoteType],
    default_note_type_id: &str,
) -> Vec<ObsidianVaultImportDraft> {
    let vault_identity = files
        .first()
        .map(|f| f.path.replace(&f.relative_path, ""))
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "vault".to_string());
    super::vault::import_obsidian_vault(&ImportObsidianVaultArgs {
        files: files.to_vec(),
        images: images.to_vec(),
        note_types: note_types.to_vec(),
        default_type_id: default_note_type_id.to_string(),
        image_type_id: "image_obj".into(),
        type_id_mapping: Map::new(),
        vault_identity: Some(vault_identity),
        existing_images: Vec::new(),
    })
    .entries
    .into_iter()
    .map(|entry| {
        let name = entry
            .relative_path
            .rsplit('/')
            .next()
            .unwrap_or(&entry.relative_path)
            .to_string();
        ObsidianVaultImportDraft {
            name,
            entry_id: entry.id.clone(),
            title_references: entry.wikilinks.clone(),
            image_references: entry.image_refs.clone(),
            draft: entry,
        }
    })
    .collect()
}

/// `buildObsidianRelatedImportPlan` — `related_notes` moves to a second pass
/// so forward-referenced objects exist before links are written.
pub fn build_obsidian_related_import_plan(
    draft: &ObsidianImportDraft,
    entry_id: &str,
    imported_title_ids: &HashMap<String, String>,
    existing_title_ids: Option<&HashMap<String, String>>,
) -> ObsidianRelatedImportPlan {
    let mut first_pass_header_props = draft.header_props.clone();
    first_pass_header_props.shift_remove("related_notes");

    let mut unresolved_targets = Vec::new();
    let mut seen = HashSet::new();
    let second_pass_related_ids = draft
        .wikilinks
        .iter()
        .map(|w| normalize_obsidian_title_target(w))
        .filter(|t| !t.is_empty())
        .filter_map(|target| {
            let normalized = target.to_lowercase();
            let id = imported_title_ids
                .get(&normalized)
                .or_else(|| existing_title_ids.and_then(|m| m.get(&normalized)));
            match id {
                Some(id) if id != entry_id => Some(id.clone()),
                Some(_) => None,
                None => {
                    unresolved_targets.push(target);
                    None
                }
            }
        })
        .filter(|id| seen.insert(id.clone()))
        .collect();

    ObsidianRelatedImportPlan {
        first_pass_header_props,
        second_pass_related_ids,
        unresolved_targets: (!unresolved_targets.is_empty()).then_some(unresolved_targets),
    }
}

/// `obsidianNoteId` — vault-scoped namespaced id (`note:<uuidv5>`).
pub fn obsidian_note_id(vault_identity: &str, relative_path: &str) -> String {
    let name = format!(
        "{}\0{}",
        canonical_path_identity(vault_identity),
        canonical_path_identity(relative_path)
    );
    format!(
        "note:{}",
        Uuid::new_v5(&OBSIDIAN_NOTE_NAMESPACE, name.as_bytes())
    )
}

/// `parseObsidianFile` — loose frontmatter, type resolution, image rewrite,
/// wikilinks + image refs for the second pass.
pub(crate) fn parse_obsidian_file(
    file: &ObsidianVaultMarkdownFile,
    note_types: &[NoteType],
    default_type_id: &str,
    type_id_mapping: &Map<String, Value>,
    image_assets: &ImageAssetIndex,
    resolutions: &mut Vec<ImageResolution>,
) -> ObsidianImportDraft {
    let (frontmatter, body) = split_loose_frontmatter(&file.content);
    let title = frontmatter
        .get("title")
        .map(frontmatter_string)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| title_from_path(&file.relative_path));
    let eden = frontmatter.get("eden").filter(|v| is_plain_object(v));
    let id = eden
        .and_then(|e| e.get("id"))
        .map(frontmatter_string)
        .filter(|s| !s.is_empty());
    let requested_type_id = [
        eden.and_then(|e| e.get("type")).map(frontmatter_string),
        frontmatter.get("type").map(frontmatter_string),
    ]
    .into_iter()
    .flatten()
    .find(|s| !s.is_empty());
    let import_conflict = [
        eden.and_then(|e| e.get("import_conflict"))
            .map(frontmatter_string),
        frontmatter.get("import_conflict").map(frontmatter_string),
    ]
    .into_iter()
    .flatten()
    .find(|s| !s.is_empty());
    let resolved_type_id = resolve_type_id(&frontmatter, note_types, type_id_mapping);
    let type_id = resolved_type_id
        .clone()
        .unwrap_or_else(|| default_type_id.to_string());
    let body_markdown =
        rewrite_image_references(&body, &file.relative_path, image_assets, resolutions);
    let wikilinks = unique_strings(
        [
            extract_body_wikilinks(&body_markdown),
            extract_frontmatter_wikilinks(&Value::Object(frontmatter.clone())),
        ]
        .concat(),
    );
    let image_refs = unique_strings(extract_markdown_image_refs(&body_markdown));

    let mut header_props = extract_header_props(&frontmatter);
    if let Some(conflict) = import_conflict {
        header_props.insert("import_conflict".into(), json!(conflict));
    }
    if resolved_type_id.is_none() {
        if let Some(requested) = requested_type_id {
            header_props.insert("source_type_id".into(), json!(requested));
        }
    }

    ObsidianImportDraft {
        source_path: file.path.clone(),
        relative_path: file.relative_path.clone(),
        id,
        title,
        type_id,
        body_markdown,
        header_props,
        wikilinks,
        image_refs,
    }
}

fn unique_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter_map(|value| {
            let trimmed = value.trim().to_string();
            (!trimmed.is_empty() && seen.insert(trimmed.clone())).then_some(trimmed)
        })
        .collect()
}
