//! Port of `obsidianVault.ts` — vault import planning and image
//! reconciliation. `identity.rs` owns namespaced ids + existing-image
//! matching; `draft.rs` owns per-file parsing and the related-import plan.

use std::collections::HashSet;

use super::draft::parse_obsidian_file;
use super::identity::{
    build_image_object_draft, find_existing_image, find_existing_image_collisions,
    has_conflicting_canonical_id, image_object_id, infer_vault_identity,
};
use super::images::{
    build_image_asset_map, canonical_path_identity, legacy_image_id, normalize_path,
    ImageAssetCollision, ImageResolution,
};
use super::{
    ImportObsidianVaultArgs, ImportObsidianVaultResult, ObsidianImageImportItem,
    ObsidianImageImportReport,
};

/// `importObsidianVault`.
pub fn import_obsidian_vault(args: &ImportObsidianVaultArgs) -> ImportObsidianVaultResult {
    let image_assets = build_image_asset_map(&args.images);
    let mut resolutions: Vec<ImageResolution> = Vec::new();
    let entries = args
        .files
        .iter()
        .map(|file| {
            parse_obsidian_file(
                file,
                &args.note_types,
                &args.default_type_id,
                &args.type_id_mapping,
                &image_assets,
                &mut resolutions,
            )
        })
        .collect();

    let vault_identity = normalize_path(
        args.vault_identity
            .clone()
            .unwrap_or_else(|| infer_vault_identity(&args.files, &args.images))
            .as_str(),
    );
    let vault_identity_key = canonical_path_identity(&vault_identity);
    let legacy_collisions: HashSet<String> = image_assets
        .legacy
        .values()
        .filter(|candidates| candidates.len() > 1)
        .flat_map(|candidates| {
            candidates
                .iter()
                .map(|c| normalize_path(&c.relative_path))
                .collect::<Vec<_>>()
        })
        .collect();

    let mut imported = Vec::new();
    let mut reused = Vec::new();
    let mut collided = Vec::new();
    let mut skipped = Vec::new();
    let failed = Vec::new();
    let mut id_collisions = Vec::new();
    let incoming_canonical_ids: HashSet<String> = args
        .images
        .iter()
        .map(|image| image_object_id(&vault_identity_key, &normalize_path(&image.relative_path)))
        .collect();
    let incoming_source_keys: HashSet<String> = args
        .images
        .iter()
        .map(|image| {
            format!(
                "{vault_identity_key}\0{}",
                canonical_path_identity(&image.relative_path)
            )
        })
        .collect();
    let existing_collisions = find_existing_image_collisions(
        &args.existing_images,
        &incoming_canonical_ids,
        &incoming_source_keys,
    );

    let mut images = Vec::new();
    for image in &args.images {
        let relative_path = normalize_path(&image.relative_path);
        let legacy_id = legacy_image_id(&image.relative_path);
        let canonical_id = image_object_id(&vault_identity_key, &relative_path);
        let item = ObsidianImageImportItem {
            id: canonical_id.clone(),
            relative_path: relative_path.clone(),
            legacy_id: legacy_id.clone(),
        };
        let path_collision = image_assets
            .path_collisions
            .iter()
            .any(|collision| collision.paths.contains(&relative_path));
        let invalid_path = image_assets
            .invalid_relative_paths
            .contains(&image.relative_path);
        let legacy_collision = legacy_collisions.contains(&relative_path);
        let object_id_collision = has_conflicting_canonical_id(
            &args.existing_images,
            &canonical_id,
            &vault_identity_key,
            &relative_path,
        );
        if object_id_collision {
            id_collisions.push(ImageAssetCollision {
                kind: "object-id-collision".into(),
                key: canonical_id.clone(),
                paths: vec![relative_path.clone()],
            });
        }
        let matched = if path_collision || invalid_path || legacy_collision || object_id_collision {
            None
        } else {
            find_existing_image(
                &args.existing_images,
                &vault_identity_key,
                &relative_path,
                &legacy_id,
            )
        };
        let id = matched.map(|m| m.id.clone()).unwrap_or(canonical_id);
        images.push(build_image_object_draft(
            image,
            &args.image_type_id,
            &vault_identity,
            &id,
            &vault_identity_key,
        ));
        let bucketed = ObsidianImageImportItem { id, ..item };
        if path_collision || legacy_collision || object_id_collision {
            collided.push(bucketed);
        } else if invalid_path {
            skipped.push(bucketed);
        } else if matched.is_some() {
            reused.push(bucketed);
        } else {
            imported.push(bucketed);
        }
    }

    let mut blocking_collisions = image_assets.path_collisions.clone();
    blocking_collisions.extend(id_collisions);
    blocking_collisions.extend(existing_collisions);
    for resolution in &resolutions {
        if resolution.status != "missing" && resolution.status != "invalid" {
            continue;
        }
        skipped.push(ObsidianImageImportItem {
            id: String::new(),
            relative_path: resolution.reference.clone(),
            legacy_id: String::new(),
        });
    }

    let can_write = blocking_collisions.is_empty()
        && image_assets.invalid_relative_paths.is_empty()
        && !resolutions
            .iter()
            .any(|r| r.status == "ambiguous" || r.status == "invalid");

    ImportObsidianVaultResult {
        entries,
        images,
        image_report: ObsidianImageImportReport {
            resolved: resolutions
                .iter()
                .filter(|r| r.status == "resolved")
                .cloned()
                .collect(),
            ambiguous: resolutions
                .iter()
                .filter(|r| r.status == "ambiguous")
                .cloned()
                .collect(),
            missing: resolutions
                .iter()
                .filter(|r| r.status == "missing")
                .cloned()
                .collect(),
            invalid: resolutions
                .iter()
                .filter(|r| r.status == "invalid")
                .cloned()
                .collect(),
            collisions: image_assets.path_collisions,
            imported,
            reused,
            collided,
            skipped,
            failed,
            blocking_collisions,
            invalid_image_paths: image_assets.invalid_relative_paths,
            can_write,
        },
    }
}

/// `recordObsidianImageImportFailure` — failures are tracked across all
/// buckets (imported/reused/collided/skipped), deduped by id+path.
pub fn record_obsidian_image_import_failure(
    report: &mut ObsidianImageImportReport,
    image_id: &str,
) {
    let item = report
        .imported
        .iter()
        .chain(report.reused.iter())
        .chain(report.collided.iter())
        .chain(report.skipped.iter())
        .find(|candidate| candidate.id == image_id)
        .cloned();
    if let Some(item) = item {
        if !report
            .failed
            .iter()
            .any(|c| c.id == item.id && c.relative_path == item.relative_path)
        {
            report.failed.push(item);
        }
    }
}
