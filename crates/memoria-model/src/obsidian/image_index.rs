//! Port of `obsidianVaultImportImages.ts` — the vault image asset index and
//! reference resolution (`buildImageAssetMap`, `resolveImageAsset`,
//! `ImageResolution`, `ImageAssetCollision`). Pure helpers live in `images.rs`.

use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use super::images::{
    fold_path, is_absolute_path, legacy_image_id, normalize_path, normalize_relative_path,
    safe_decode,
};
use super::ObsidianVaultImageFile;
/// `ImageAssetIndex`.
#[derive(Default)]
pub struct ImageAssetIndex {
    pub exact: BTreeMap<String, Vec<ObsidianVaultImageFile>>,
    pub folded: BTreeMap<String, Vec<ObsidianVaultImageFile>>,
    pub basename: BTreeMap<String, Vec<ObsidianVaultImageFile>>,
    pub legacy: BTreeMap<String, Vec<ObsidianVaultImageFile>>,
    pub path_collisions: Vec<ImageAssetCollision>,
    pub invalid_relative_paths: Vec<String>,
}

/// `ImageAssetCollision`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageAssetCollision {
    pub kind: String,
    pub key: String,
    pub paths: Vec<String>,
}

/// `ImageResolution`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageResolution {
    pub reference: String,
    pub status: String,
    pub asset: Option<ObsidianVaultImageFile>,
    pub candidates: Vec<ObsidianVaultImageFile>,
}

impl ImageResolution {
    fn resolved(reference: String, candidates: Vec<ObsidianVaultImageFile>) -> Self {
        Self {
            reference,
            status: "resolved".into(),
            asset: candidates.first().cloned(),
            candidates,
        }
    }

    fn unresolved(reference: &str, status: &str, candidates: Vec<ObsidianVaultImageFile>) -> Self {
        Self {
            reference: reference.to_string(),
            status: status.into(),
            asset: None,
            candidates,
        }
    }
}

/// `buildImageAssetMap` — indexes by exact/folded/basename/legacy keys and
/// preflights path collisions; invalid relative paths are listed, not indexed.
pub fn build_image_asset_map(images: &[ObsidianVaultImageFile]) -> ImageAssetIndex {
    let mut index = ImageAssetIndex::default();
    for image in images {
        if normalize_relative_path("", &image.relative_path).is_none() {
            index
                .invalid_relative_paths
                .push(image.relative_path.clone());
            continue;
        }
        let relative_path = normalize_path(&image.relative_path);
        add(&mut index.exact, relative_path, image);
        add(&mut index.folded, fold_path(&image.relative_path), image);
        add(&mut index.basename, fold_path(&image.name), image);
        add(
            &mut index.legacy,
            legacy_image_id(&image.relative_path),
            image,
        );
    }
    for (key, candidates) in &index.exact {
        if candidates.len() > 1 {
            let mut paths: Vec<String> = candidates
                .iter()
                .map(|c| normalize_path(&c.relative_path))
                .collect();
            paths.sort();
            index.path_collisions.push(ImageAssetCollision {
                kind: "duplicate-relative-path".into(),
                key: key.clone(),
                paths,
            });
        }
    }
    for (key, candidates) in &index.folded {
        let mut paths: Vec<String> = candidates
            .iter()
            .map(|c| normalize_path(&c.relative_path))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        paths.sort();
        if candidates.len() > 1 && paths.len() > 1 {
            index.path_collisions.push(ImageAssetCollision {
                kind: "case-only-relative-path".into(),
                key: key.clone(),
                paths,
            });
        }
    }
    index
}

/// `resolveImageAsset` — legacy `image:<8hex>` refs, external/absolute
/// rejection, then exact → folded → basename (all note-relative, then root).
pub fn resolve_image_asset(
    src: &str,
    source_dir: &str,
    image_assets: &ImageAssetIndex,
) -> ImageResolution {
    static LEGACY_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)^image:([0-9a-f]{8})$").expect("legacy"));
    if let Some(m) = LEGACY_RE.captures(src) {
        let key = format!("image:{}", m[1].to_lowercase());
        let candidates = image_assets.legacy.get(&key).cloned().unwrap_or_default();
        if candidates.len() == 1 {
            return ImageResolution::resolved(src.to_string(), candidates);
        }
        let status = if candidates.len() > 1 {
            "ambiguous"
        } else {
            "missing"
        };
        return ImageResolution::unresolved(src, status, candidates);
    }
    static SCHEME_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)^[a-z]+:").expect("scheme"));
    if SCHEME_RE.is_match(src) {
        static WIN_ABS_RE: LazyLock<regex::Regex> =
            LazyLock::new(|| regex::Regex::new(r"^[a-zA-Z]:[\\/]").expect("win"));
        let status = if WIN_ABS_RE.is_match(src) {
            "invalid"
        } else {
            "external"
        };
        return ImageResolution::unresolved(src, status, Vec::new());
    }

    let decoded = safe_decode(src);
    if is_absolute_path(&decoded) {
        return ImageResolution::unresolved(src, "invalid", Vec::new());
    }
    let normalized = normalize_path(&decoded);
    let Some(relative) = normalize_relative_path(source_dir, &decoded) else {
        return ImageResolution::unresolved(src, "invalid", Vec::new());
    };
    for key in [
        relative.clone(),
        normalized.clone(),
        fold_path(&relative),
        fold_path(&normalized),
    ]
    .into_iter()
    .zip([
        &image_assets.exact,
        &image_assets.exact,
        &image_assets.folded,
        &image_assets.folded,
    ]) {
        let candidates = key.1.get(&key.0).cloned().unwrap_or_default();
        if !candidates.is_empty() {
            return resolution_for_candidates(src, candidates);
        }
    }

    let basename = normalized.rsplit('/').next().unwrap_or(&normalized);
    let candidates = unique(
        &image_assets
            .basename
            .get(&fold_path(basename))
            .cloned()
            .unwrap_or_default(),
    );
    if candidates.len() == 1 {
        return ImageResolution::resolved(src.to_string(), candidates);
    }
    let status = if candidates.len() > 1 {
        "ambiguous"
    } else {
        "missing"
    };
    ImageResolution::unresolved(src, status, candidates)
}

fn resolution_for_candidates(
    reference: &str,
    candidates: Vec<ObsidianVaultImageFile>,
) -> ImageResolution {
    let unique = unique(&candidates);
    if unique.len() == 1 {
        ImageResolution::resolved(reference.to_string(), unique)
    } else {
        ImageResolution::unresolved(reference, "ambiguous", unique)
    }
}

fn add(
    map: &mut BTreeMap<String, Vec<ObsidianVaultImageFile>>,
    key: String,
    image: &ObsidianVaultImageFile,
) {
    // TS dedupes by *object identity* (`values.includes(image)`); every array
    // slot is a distinct record, so entries always push (two structurally
    // identical images still collide — intentional).
    map.entry(key).or_default().push(image.clone());
}

fn unique(values: &[ObsidianVaultImageFile]) -> Vec<ObsidianVaultImageFile> {
    values.to_vec()
}
