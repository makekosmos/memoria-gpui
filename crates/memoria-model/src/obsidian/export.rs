//! Port of `obsidianVaultExport.ts` — markdown export file planning:
//! folder-aware note paths, safe base names, asset manifests. Writing itself
//! goes through Engine `filesystem.vault.export` (cortex), not the app FS.

use std::collections::{HashMap, HashSet};

use serde_json::json;

use crate::frontmatter::build_entry_markdown_document;
use crate::model::{Entry, NoteType};

use super::export_assets::{
    rewrite_obsidian_export_header_image_references, rewrite_obsidian_export_image_references,
    ExportAssetContext,
};
use super::export_paths::safe_vault_path_segment;

/// `ObsidianVaultFolder`.
#[derive(Clone, Debug)]
pub struct ObsidianVaultFolder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
}

/// `ObsidianExportFile` — `content` text files; `sourcePath` binary copies.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ObsidianExportFile {
    #[serde(rename = "relativePath")]
    pub relative_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(
        default,
        rename = "sourcePath",
        skip_serializing_if = "Option::is_none"
    )]
    pub source_path: Option<String>,
}

/// `BuildObsidianExportFilesArgs` — `body_markdown`/`title` lookups are
/// closures/maps supplied by the caller; `entries` carry Entry fields.
pub struct BuildObsidianExportFilesArgs<'a> {
    pub entries: Vec<Entry>,
    pub note_types: &'a [NoteType],
    pub body_markdown: Box<dyn Fn(&Entry) -> String + 'a>,
    pub title_lookup: Option<&'a HashMap<String, String>>,
    pub selected_type_ids: Option<HashSet<String>>,
    pub folder_path_by_id: Option<&'a HashMap<String, String>>,
}

/// `buildObsidianExportFiles` — returns the file plan (markdown + copied
/// assets + the uncopyable-asset manifest).
pub fn build_obsidian_export_files(
    args: &BuildObsidianExportFilesArgs<'_>,
) -> Result<Vec<ObsidianExportFile>, String> {
    let note_types_by_id: HashMap<&str, &NoteType> =
        args.note_types.iter().map(|t| (t.id.as_str(), t)).collect();
    let entries_by_id: HashMap<String, Entry> = args
        .entries
        .iter()
        .map(|e| (e.id.clone(), e.clone()))
        .collect();
    let mut context = ExportAssetContext::default();

    let mut files = Vec::new();
    for entry in &args.entries {
        if entry.deleted_at.is_some() {
            continue;
        }
        if let Some(selected) = &args.selected_type_ids {
            if let Some(type_id) = &entry.type_id {
                if !selected.contains(type_id) {
                    continue;
                }
            }
        }
        let relative_path = build_obsidian_export_relative_path(
            entry,
            args.folder_path_by_id,
            &mut context.used_paths,
        );
        let note_type = entry
            .type_id
            .as_deref()
            .and_then(|id| note_types_by_id.get(id))
            .copied();
        let rewritten_header = rewrite_obsidian_export_header_image_references(
            entry,
            note_type,
            &relative_path,
            &entries_by_id,
            &mut context,
        );
        let export_entry = match rewritten_header {
            Some(props_json) => {
                let mut cloned = entry.clone();
                cloned.header_props_json = Some(props_json);
                cloned
            }
            None => entry.clone(),
        };
        let rewritten_body = rewrite_obsidian_export_image_references(
            &(args.body_markdown)(entry),
            &relative_path,
            &mut context,
        );
        files.push(ObsidianExportFile {
            relative_path,
            content: Some(build_entry_markdown_document(
                &export_entry,
                note_type,
                &rewritten_body,
                args.title_lookup,
            )?),
            source_path: None,
        });
    }

    if context.asset_files.is_empty() && context.asset_manifest_entries.is_empty() {
        return Ok(files);
    }
    let mut output = files;
    output.extend(context.asset_files);
    if !context.asset_manifest_entries.is_empty() {
        output.push(ObsidianExportFile {
            relative_path: "assets/obsidian-asset-manifest.json".into(),
            content: serde_json::to_string_pretty(&json!({
                "limitation": "Some asset references cannot be resolved to a safe local source path; those references stay unchanged and are recorded here as export limitations.",
                "assets": context.asset_manifest_entries,
            }))
            .ok(),
            source_path: None,
        });
    }
    Ok(output)
}

/// `buildObsidianFolderPathLookup` — id → slash path; cycles/orphans → no
/// entry (Vue returns `null`, which the caller treats as no folder).
pub fn build_obsidian_folder_path_lookup(
    folders: &[ObsidianVaultFolder],
) -> HashMap<String, String> {
    let folder_by_id: HashMap<&str, &ObsidianVaultFolder> =
        folders.iter().map(|f| (f.id.as_str(), f)).collect();
    let mut path_by_id = HashMap::new();
    let mut resolving = HashSet::new();

    fn resolve(
        folder_id: &str,
        folder_by_id: &HashMap<&str, &ObsidianVaultFolder>,
        path_by_id: &mut HashMap<String, String>,
        resolving: &mut HashSet<String>,
    ) -> Option<String> {
        if let Some(cached) = path_by_id.get(folder_id) {
            return Some(cached.clone());
        }
        let folder = folder_by_id.get(folder_id)?;
        if !resolving.insert(folder_id.to_string()) {
            return None;
        }
        let parent_path = folder
            .parent_id
            .as_deref()
            .and_then(|parent| resolve(parent, folder_by_id, path_by_id, resolving));
        resolving.remove(folder_id);
        let segment = safe_vault_path_segment(&folder.name);
        let resolved = match parent_path {
            Some(parent) => format!("{parent}/{segment}"),
            None => segment,
        };
        path_by_id.insert(folder_id.to_string(), resolved.clone());
        Some(resolved)
    }

    for folder in folders {
        resolve(&folder.id, &folder_by_id, &mut path_by_id, &mut resolving);
    }
    path_by_id
}

/// `exportObsidianVaultMarkdownFiles` — the summary-record convenience path
/// (`bodyMarkdownLookup`/`titleLookup` keyed by entry id).
pub fn export_obsidian_vault_markdown_files(
    entries: Vec<Entry>,
    note_types: &[NoteType],
    body_markdown_lookup: &HashMap<String, String>,
    title_lookup: &HashMap<String, String>,
) -> Result<Vec<ObsidianExportFile>, String> {
    build_obsidian_export_files(&BuildObsidianExportFilesArgs {
        entries,
        note_types,
        body_markdown: Box::new(move |entry| {
            body_markdown_lookup
                .get(&entry.id)
                .cloned()
                .unwrap_or_default()
        }),
        title_lookup: Some(title_lookup),
        selected_type_ids: None,
        folder_path_by_id: None,
    })
}

fn build_obsidian_export_relative_path(
    entry: &Entry,
    folder_path_by_id: Option<&HashMap<String, String>>,
    used_paths: &mut HashSet<String>,
) -> String {
    let base_title = if entry.title.is_empty() {
        &entry.id
    } else {
        &entry.title
    };
    let base_name = safe_markdown_base_name(base_title);
    let folder_path = entry
        .folder_id
        .as_deref()
        .and_then(|id| folder_path_by_id.and_then(|m| m.get(id)))
        .cloned()
        .unwrap_or_default();

    let mut relative_path = if folder_path.is_empty() {
        format!("{base_name}.md")
    } else {
        format!("{folder_path}/{base_name}.md")
    };
    let mut suffix = 2;
    while used_paths.contains(&relative_path.to_lowercase()) {
        let leaf = format!("{base_name}-{suffix}.md");
        relative_path = if folder_path.is_empty() {
            leaf
        } else {
            format!("{folder_path}/{leaf}")
        };
        suffix += 1;
    }
    used_paths.insert(relative_path.to_lowercase());
    relative_path
}

/// `safeMarkdownBaseName` — forbidden chars/control/whitespace → `-`,
/// leading/trailing `[.\s-]` trimmed; `""` → `"eden-object"`.
fn safe_markdown_base_name(value: &str) -> String {
    let mut safe: String = value
        .trim()
        .chars()
        .map(|c| {
            if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || (c as u32) < 32
            {
                '-'
            } else {
                c
            }
        })
        .collect();
    // `\s+`→`-`, `-+`→`-`, `^[.\s-]+|[.\s-]+$`→"".
    let mut folded = String::with_capacity(safe.len());
    for c in safe.chars() {
        if c.is_whitespace() {
            folded.push('-');
        } else {
            folded.push(c);
        }
    }
    safe = folded
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let safe = safe.trim_matches(|c: char| c == '.' || c.is_whitespace() || c == '-');
    if safe.is_empty() {
        "eden-object".to_string()
    } else {
        safe.to_string()
    }
}
