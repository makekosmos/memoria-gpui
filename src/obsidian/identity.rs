//! Port of `obsidianVault.ts` internals — namespaced image-object ids and the
//! existing-image reconciliation (`findExistingImage` + collision preflights).

use std::collections::HashSet;

use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::content::write_entry_markdown;

use super::images::{
    canonical_path_identity, legacy_image_id, normalize_path, title_from_path, ImageAssetCollision,
};
use super::{
    ObsidianExistingImage, ObsidianImageObjectDraft, ObsidianVaultImageFile,
    ObsidianVaultMarkdownFile,
};

const OBSIDIAN_IMAGE_NAMESPACE: Uuid = Uuid::from_u128(0xcce00dcb_53d2_5dd0_b981_bd4d5eae7888);

/// `imageObjectId` — `image:<uuidv5(vaultKey \0 canonicalRel)>`.
pub(crate) fn image_object_id(vault_identity_key: &str, relative_path: &str) -> String {
    let name = format!(
        "{vault_identity_key}\0{}",
        canonical_path_identity(relative_path)
    );
    format!(
        "image:{}",
        Uuid::new_v5(&OBSIDIAN_IMAGE_NAMESPACE, name.as_bytes())
    )
}

/// `buildImageObjectDraft`.
pub(crate) fn build_image_object_draft(
    image: &ObsidianVaultImageFile,
    image_type_id: &str,
    vault_identity: &str,
    id: &str,
    vault_identity_key: &str,
) -> ObsidianImageObjectDraft {
    let width = image.width.map(|v| json!(v)).unwrap_or(json!(""));
    let height = image.height.map(|v| json!(v)).unwrap_or(json!(""));
    let resolution = match (image.width, image.height) {
        (Some(w), Some(h)) => format!("{w}x{h}"),
        _ => String::new(),
    };
    let mut header_props = Map::new();
    header_props.insert("image".into(), json!(image.file_url));
    header_props.insert("file_name".into(), json!(image.name));
    header_props.insert("mime_type".into(), json!(image.mime_type));
    header_props.insert("size_bytes".into(), json!(image.size_bytes));
    header_props.insert("width".into(), width);
    header_props.insert("height".into(), height);
    header_props.insert("resolution".into(), json!(resolution));
    header_props.insert("source_path".into(), json!(image.path));
    header_props.insert("source_vault".into(), json!(vault_identity));
    header_props.insert("source_vault_key".into(), json!(vault_identity_key));
    header_props.insert(
        "source_relative_path".into(),
        json!(normalize_path(&image.relative_path)),
    );
    header_props.insert(
        "source_relative_path_key".into(),
        json!(canonical_path_identity(&image.relative_path)),
    );
    header_props.insert(
        "legacy_image_id".into(),
        json!(legacy_image_id(&image.relative_path)),
    );
    header_props.insert(
        "alt_text".into(),
        json!(title_from_path(&image.relative_path)),
    );
    ObsidianImageObjectDraft {
        id: id.to_string(),
        title: image.name.clone(),
        type_id: image_type_id.to_string(),
        content_json: write_entry_markdown(&format!("![]({})", image.file_url)),
        header_props,
    }
}

/// `hasConflictingCanonicalId` — same id but a different source identity.
pub(crate) fn has_conflicting_canonical_id(
    existing_images: &[ObsidianExistingImage],
    canonical_id: &str,
    vault_identity_key: &str,
    relative_path: &str,
) -> bool {
    existing_images.iter().any(|candidate| {
        if candidate.id != canonical_id {
            return false;
        }
        match existing_source_props(candidate) {
            Some((vault, path)) => {
                canonical_path_identity(&vault) != vault_identity_key
                    || canonical_path_identity(&path) != canonical_path_identity(relative_path)
            }
            None => false,
        }
    })
}

/// `findExistingImage` — canonical id, then source metadata, then legacy id.
/// Multi-candidate matches are collisions elsewhere; here they count as no
/// match.
pub(crate) fn find_existing_image<'a>(
    existing_images: &'a [ObsidianExistingImage],
    vault_identity_key: &str,
    relative_path: &str,
    legacy_id: &str,
) -> Option<&'a ObsidianExistingImage> {
    let canonical_id = image_object_id(vault_identity_key, relative_path);
    let canonical_matches: Vec<&ObsidianExistingImage> = existing_images
        .iter()
        .filter(|c| c.id == canonical_id)
        .collect();
    if canonical_matches.len() == 1 {
        let candidate = canonical_matches[0];
        // Missing source metadata matches unconditionally; present metadata
        // must agree on the canonical vault+path identity.
        match existing_source_props(candidate) {
            None => return Some(candidate),
            Some((vault, path))
                if canonical_path_identity(&vault) == vault_identity_key
                    && canonical_path_identity(&path) == canonical_path_identity(relative_path) =>
            {
                return Some(candidate);
            }
            _ => {}
        }
    }
    let source_matches: Vec<&ObsidianExistingImage> = existing_images
        .iter()
        .filter(|c| {
            existing_image_source(c).is_some_and(|(key, _)| {
                key == format!(
                    "{vault_identity_key}\0{}",
                    canonical_path_identity(relative_path)
                )
            })
        })
        .collect();
    if source_matches.len() == 1 {
        return Some(source_matches[0]);
    }
    if source_matches.len() > 1 {
        return None;
    }
    let legacy_matches: Vec<&ObsidianExistingImage> = existing_images
        .iter()
        .filter(|c| {
            if c.id != legacy_id
                && c.header_props
                    .get("legacy_image_id")
                    .and_then(Value::as_str)
                    != Some(legacy_id)
            {
                return false;
            }
            match existing_source_props(c) {
                None => true,
                Some((vault, path)) => {
                    canonical_path_identity(&vault) == vault_identity_key
                        && canonical_path_identity(&path) == canonical_path_identity(relative_path)
                }
            }
        })
        .collect();
    (legacy_matches.len() == 1).then(|| legacy_matches[0])
}

/// `findExistingImageCollisions` — duplicate canonical ids / duplicate source
/// metadata among existing objects block the write preflight.
pub(crate) fn find_existing_image_collisions(
    existing_images: &[ObsidianExistingImage],
    incoming_canonical_ids: &HashSet<String>,
    incoming_source_keys: &HashSet<String>,
) -> Vec<ImageAssetCollision> {
    let mut collisions = Vec::new();
    // Insertion-ordered grouping — TS `Map` iterates in first-seen order and
    // the golden fixture pins `paths` order.
    let mut ids: Vec<(&str, Vec<&ObsidianExistingImage>)> = Vec::new();
    for image in existing_images {
        match ids.iter_mut().find(|(id, _)| *id == image.id) {
            Some((_, matches)) => matches.push(image),
            None => ids.push((image.id.as_str(), vec![image])),
        }
    }
    for (id, matches) in &ids {
        if matches.len() < 2 || !incoming_canonical_ids.contains(*id) {
            continue;
        }
        collisions.push(ImageAssetCollision {
            kind: "existing-canonical-id-duplicate".into(),
            key: id.to_string(),
            paths: matches.iter().map(|m| existing_image_path(m)).collect(),
        });
    }
    let mut sources: Vec<(String, Vec<String>)> = Vec::new();
    for image in existing_images {
        if let Some((key, path)) = existing_image_source(image) {
            match sources.iter_mut().find(|(k, _)| *k == key) {
                Some((_, paths)) => paths.push(path),
                None => sources.push((key, vec![path])),
            }
        }
    }
    for (key, matches) in &sources {
        if matches.len() < 2 || !incoming_source_keys.contains(key) {
            continue;
        }
        collisions.push(ImageAssetCollision {
            kind: "existing-source-duplicate".into(),
            key: key.clone(),
            paths: matches.clone(),
        });
    }
    collisions
}

/// `existingImageSource` — `{key, path}` from the vault/path identity props.
fn existing_image_source(image: &ObsidianExistingImage) -> Option<(String, String)> {
    let (vault, relative_path) = existing_source_props(image)?;
    let key = format!(
        "{}\0{}",
        canonical_path_identity(&vault),
        canonical_path_identity(&relative_path)
    );
    Some((key, normalize_path(&relative_path)))
}

fn existing_source_props(image: &ObsidianExistingImage) -> Option<(String, String)> {
    let props = &image.header_props;
    let vault = props
        .get("source_vault_key")
        .or_else(|| props.get("source_vault"))
        .and_then(Value::as_str)?;
    let path = props
        .get("source_relative_path_key")
        .or_else(|| props.get("source_relative_path"))
        .and_then(Value::as_str)?;
    Some((vault.to_string(), path.to_string()))
}

fn existing_image_path(image: &ObsidianExistingImage) -> String {
    existing_image_source(image)
        .map(|(_, p)| p)
        .unwrap_or_else(|| image.id.clone())
}

/// `inferVaultIdentity` — strip the sample entry's relative path off its
/// absolute path (canonical comparison), else the whole path; `""` → `vault`.
pub(crate) fn infer_vault_identity(
    files: &[ObsidianVaultMarkdownFile],
    images: &[ObsidianVaultImageFile],
) -> String {
    let (sample_path, sample_relative) = files
        .first()
        .map(|f| (f.path.as_str(), f.relative_path.as_str()))
        .or_else(|| {
            images
                .first()
                .map(|i| (i.path.as_str(), i.relative_path.as_str()))
        })
        .unwrap_or(("", ""));
    if sample_path.is_empty() {
        return "vault".into();
    }
    let normalized_path = normalize_path(sample_path);
    let relative_path = normalize_path(sample_relative);
    let path_key = canonical_path_identity(&normalized_path);
    let relative_key = canonical_path_identity(&relative_path);
    let is_suffix = path_key == relative_key
        || (path_key.len() > relative_key.len() && path_key.ends_with(&format!("/{relative_key}")));
    if is_suffix {
        // Byte-slice may fail if NFC changed lengths — fall back to the full
        // normalized path rather than guessing a prefix.
        let Some(prefix) = normalized_path
            .len()
            .checked_sub(relative_path.len())
            .and_then(|end| normalized_path.get(..end))
        else {
            return normalized_path;
        };
        let stripped = prefix.trim_end_matches('/');
        return if stripped.is_empty() {
            "vault".into()
        } else {
            stripped.to_string()
        };
    }
    normalized_path
}
