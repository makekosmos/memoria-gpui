//! Port of `obsidianVaultExportAssetPaths.ts` — export target path planning:
//! safe segments, `file:`/`kosmos-local-image:`/absolute source handling,
//! collision-free path allocation, and note-relative link computation.

use std::collections::HashSet;
use std::sync::LazyLock;

use super::asset_sources::{
    canonical_local_asset_key, extract_local_asset_path, resolve_copyable_local_asset_source_path,
};

/// `ObsidianExportAssetPlan`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObsidianExportAssetPlan {
    pub target_relative_path: String,
    pub source_path: Option<String>,
    pub asset_key: Option<String>,
}

/// `safeVaultPathSegment` — Windows-forbidden chars → `-`, control chars →
/// `-`, dot/space-only edges trimmed; `""` → `"folder"`.
pub fn safe_vault_path_segment(value: &str) -> String {
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
    // `.+$` / `^.+` and whitespace folding are separate passes in TS.
    while safe.ends_with('.') {
        safe.pop();
    }
    let safe = safe.trim_start_matches('.');
    let mut folded = String::with_capacity(safe.len());
    let mut pending_space = false;
    for c in safe.chars() {
        if c.is_whitespace() {
            pending_space = !folded.is_empty();
        } else {
            if pending_space {
                folded.push(' ');
                pending_space = false;
            }
            folded.push(c);
        }
    }
    let folded = folded.trim().to_string();
    if folded.is_empty() {
        "folder".to_string()
    } else {
        folded
    }
}

/// `deriveObsidianExportAssetPlan` — `null` for remote/empty/unresolvable
/// sources; otherwise a target path + copyable `sourcePath` + dedupe key.
pub fn derive_obsidian_export_asset_plan(
    source: &str,
    preferred_file_name: Option<&str>,
    mime_type: Option<&str>,
) -> Option<ObsidianExportAssetPlan> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return None;
    }
    static REMOTE_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)^(https?|mailto|data):").expect("remote"));
    if REMOTE_RE.is_match(trimmed) {
        return None;
    }
    let raw_path = extract_local_asset_path(trimmed)?;
    let normalized = normalize_export_path(&raw_path);
    if normalized.is_empty() {
        return None;
    }
    let path_parts: Vec<&str> = normalized.split('/').filter(|p| !p.is_empty()).collect();
    if path_parts.is_empty() {
        return None;
    }
    let source_path = resolve_copyable_local_asset_source_path(trimmed);
    Some(ObsidianExportAssetPlan {
        target_relative_path: build_asset_target_relative_path(
            trimmed,
            &path_parts,
            preferred_file_name,
            mime_type,
        ),
        asset_key: source_path.as_deref().map(canonical_local_asset_key),
        source_path,
    })
}

/// `allocateUniqueExportPath` — `name.ext` → `name-2.ext`…, `ru`-lowercased
/// dedupe; throws past 1000 candidates.
pub fn allocate_unique_export_path(
    relative_path: &str,
    used_paths: &mut HashSet<String>,
) -> Result<String, String> {
    let key = relative_path.to_lowercase();
    if !used_paths.contains(&key) {
        used_paths.insert(key);
        return Ok(relative_path.to_string());
    }
    let (folder, file_name) = match relative_path.rfind('/') {
        Some(index) => (&relative_path[..index + 1], &relative_path[index + 1..]),
        None => ("", relative_path),
    };
    let dot_index = file_name.rfind('.').filter(|i| *i > 0);
    let (stem, ext) = match dot_index {
        Some(index) => (&file_name[..index], &file_name[index..]),
        None => (file_name, ""),
    };
    for suffix in 2..1000usize {
        let candidate = format!("{folder}{stem}-{suffix}{ext}");
        let candidate_key = candidate.to_lowercase();
        if !used_paths.contains(&candidate_key) {
            used_paths.insert(candidate_key);
            return Ok(candidate);
        }
    }
    Err("[kepler-shell] Markdown vault export asset path collision could not be resolved".into())
}

/// `relativePathBetween` — `fromDir` → `toPath` with `..` hops (`""` → `.`).
pub fn relative_path_between(from_dir: &str, to_path: &str) -> String {
    let from_parts: Vec<&str> = from_dir.split('/').filter(|p| !p.is_empty()).collect();
    let to_parts: Vec<&str> = to_path.split('/').filter(|p| !p.is_empty()).collect();
    let mut shared = 0;
    while shared < from_parts.len()
        && shared < to_parts.len()
        && from_parts[shared] == to_parts[shared]
    {
        shared += 1;
    }
    let up = from_parts.len() - shared;
    let down = &to_parts[shared..];
    let mut out: Vec<&str> = vec![".."; up];
    out.extend(down.iter());
    if out.is_empty() {
        ".".to_string()
    } else {
        out.join("/")
    }
}

fn build_asset_target_relative_path(
    source: &str,
    path_parts: &[&str],
    preferred_file_name: Option<&str>,
    mime_type: Option<&str>,
) -> String {
    static ABS_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"(?i)^(file:|kosmos-local-image:)|^[A-Za-z]:[\\/]|^/|^\\\\")
            .expect("abs")
    });
    let is_absolute_path = ABS_RE.is_match(source);
    let source_leaf = path_parts.last().copied().unwrap_or("asset");
    let preferred_leaf = file_name_from_path_like(preferred_file_name.unwrap_or_default());

    if !preferred_leaf.is_empty() {
        return format!(
            "assets/{}",
            finalize_asset_file_name(&preferred_leaf, Some(source_leaf), mime_type)
        );
    }
    if is_absolute_path {
        return format!(
            "assets/{}",
            finalize_asset_file_name(source_leaf, None, mime_type)
        );
    }
    let mut parts: Vec<String> = Vec::with_capacity(path_parts.len());
    for (index, part) in path_parts.iter().enumerate() {
        if index == path_parts.len() - 1 {
            parts.push(finalize_asset_file_name(part, None, mime_type));
        } else {
            parts.push(safe_vault_path_segment(part));
        }
    }
    format!("assets/{}", parts.join("/"))
}

fn finalize_asset_file_name(
    candidate: &str,
    fallback_extension_source: Option<&str>,
    mime_type: Option<&str>,
) -> String {
    let leaf = file_name_from_path_like(candidate);
    let safe_candidate = safe_vault_path_segment(if leaf.is_empty() { "asset" } else { &leaf });
    if !file_extension(&safe_candidate).is_empty() {
        return safe_candidate;
    }
    let fallback_extension = fallback_extension_source
        .map(file_extension)
        .filter(|e| !e.is_empty())
        .unwrap_or_else(|| image_extension_from_mime(mime_type));
    if fallback_extension.is_empty() {
        safe_candidate
    } else {
        format!("{safe_candidate}{fallback_extension}")
    }
}

fn file_name_from_path_like(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    trimmed
        .replace('\\', "/")
        .split('/')
        .rfind(|p| !p.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn file_extension(file_name: &str) -> &str {
    match file_name.rfind('.') {
        Some(index) if index > 0 && index < file_name.len() - 1 => &file_name[index..],
        _ => "",
    }
}

fn image_extension_from_mime(mime_type: Option<&str>) -> &'static str {
    match mime_type.map(|m| m.trim().to_lowercase()).as_deref() {
        Some("image/jpeg") | Some("image/jpg") => ".jpg",
        Some("image/png") => ".png",
        Some("image/webp") => ".webp",
        Some("image/gif") => ".gif",
        Some("image/avif") => ".avif",
        _ => "",
    }
}
/// `normalizePath` (export variant — same `.`/`..` folding as the
/// import-side helper).
pub(crate) fn normalize_export_path(value: &str) -> String {
    super::images::normalize_path(value)
}
