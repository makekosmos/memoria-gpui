//! Port of `tests/obsidianVault.test.ts` — namespaced ids, legacy
//! reconciliation, and existing-object collision preflights.

use std::collections::HashSet;

use serde_json::{json, Map, Value};

use crate::obsidian::{
    canonical_path_identity, import_obsidian_vault, legacy_image_id, stable_id_from_path,
    ImportObsidianVaultArgs, ObsidianExistingImage,
};

use super::{custom_note_type, image, md_at};

fn args(
    files: Vec<crate::obsidian::ObsidianVaultMarkdownFile>,
    images: Vec<crate::obsidian::ObsidianVaultImageFile>,
    vault_identity: &str,
) -> ImportObsidianVaultArgs {
    ImportObsidianVaultArgs {
        files,
        images,
        note_types: vec![custom_note_type()],
        default_type_id: "note_obj".into(),
        image_type_id: "image_obj".into(),
        type_id_mapping: Map::new(),
        vault_identity: Some(vault_identity.into()),
        existing_images: Vec::new(),
    }
}
#[test]
fn namespaced_ids_are_stable_per_vault() {
    let make = |vault: &str| {
        import_obsidian_vault(&ImportObsidianVaultArgs {
            images: vec![image("assets/cover.png", "C:/vault")],
            ..args(vec![], vec![], vault)
        })
    };
    let first = make("C:/vault-a");
    let retry = make("C:/vault-a");
    let other = make("C:/vault-b");
    assert_eq!(first.images[0].id, retry.images[0].id);
    assert_ne!(first.images[0].id, other.images[0].id);
    assert!(first.images[0].id.starts_with("image:"));
    assert_eq!(
        first.images[0].header_props["source_vault"],
        json!("C:/vault-a")
    );
    assert_eq!(
        first.images[0].header_props["source_relative_path"],
        json!("assets/cover.png")
    );
    assert_eq!(
        first.images[0].header_props["legacy_image_id"],
        json!(format!("image:{}", stable_id_from_path("assets/cover.png")))
    );
}

#[test]
fn windows_case_identity_is_canonical() {
    let make = |vault: &str| {
        import_obsidian_vault(&ImportObsidianVaultArgs {
            images: vec![image("Assets\\Cover.png", "C:/Vault")],
            ..args(vec![], vec![], vault)
        })
    };
    let upper = make("C:/Vault");
    let lower = make("c:\\vault");
    assert_eq!(
        canonical_path_identity("C:\\Vault\\Assets\\Cover.png"),
        canonical_path_identity("c:/vault/assets/cover.png")
    );
    assert_eq!(upper.images[0].id, lower.images[0].id);
}

#[test]
fn legacy_ids_reconcile_and_rewrite() {
    let asset = image("assets/cover.png", "C:/vault");
    let old_id = legacy_image_id(&asset.relative_path);
    let imported = import_obsidian_vault(&ImportObsidianVaultArgs {
        files: vec![md_at("C:/vault", "A.md", &format!("![]({old_id})"))],
        images: vec![asset.clone()],
        existing_images: vec![ObsidianExistingImage {
            id: old_id.clone(),
            header_props: Map::new(),
        }],
        ..args(vec![], vec![], "C:/vault")
    });
    assert_eq!(imported.images[0].id, old_id);
    assert_eq!(imported.image_report.reused.len(), 1);
    assert_eq!(imported.image_report.reused[0].id, old_id);
    assert_eq!(
        imported.image_report.reused[0].relative_path,
        "assets/cover.png"
    );
    assert_eq!(
        imported.entries[0].body_markdown,
        format!("![cover]({})", asset.file_url)
    );
}

#[test]
fn a_claimed_legacy_object_is_not_reused_by_another_vault() {
    let asset = image("assets/cover.png", "C:/vault");
    let old_id = legacy_image_id(&asset.relative_path);
    let first = import_obsidian_vault(&ImportObsidianVaultArgs {
        images: vec![asset.clone()],
        existing_images: vec![ObsidianExistingImage {
            id: old_id.clone(),
            header_props: Map::new(),
        }],
        ..args(vec![], vec![], "C:/vault-a")
    });
    let second = import_obsidian_vault(&ImportObsidianVaultArgs {
        images: vec![image("assets/cover.png", "C:/vault-b")],
        existing_images: vec![ObsidianExistingImage {
            id: first.images[0].id.clone(),
            header_props: first.images[0].header_props.clone(),
        }],
        ..args(vec![], vec![], "C:/vault-b")
    });
    assert_eq!(first.images[0].id, old_id);
    assert_ne!(second.images[0].id, old_id);
    assert_eq!(second.image_report.imported.len(), 1);
}

#[test]
fn unsafe_image_paths_are_preflight_skipped() {
    let imported = import_obsidian_vault(&ImportObsidianVaultArgs {
        images: vec![image("../outside.png", "C:/vault")],
        ..args(vec![], vec![], "C:/vault")
    });
    assert_eq!(
        imported.image_report.invalid_image_paths,
        ["../outside.png"]
    );
    assert_eq!(imported.image_report.skipped.len(), 1);
    assert!(!imported.image_report.can_write);
}

#[test]
fn colliding_32bit_legacy_ids_do_not_merge() {
    let collision = ["assets/7pvu.png", "assets/a3ea.png"];
    assert_eq!(
        stable_id_from_path(collision[0]),
        stable_id_from_path(collision[1])
    );
    let imported = import_obsidian_vault(&ImportObsidianVaultArgs {
        images: collision.iter().map(|p| image(p, "C:/vault")).collect(),
        ..args(vec![], vec![], "C:/vault")
    });
    let ids: HashSet<&str> = imported.images.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids.len(), 2);
    let legacy: HashSet<&Value> = imported
        .images
        .iter()
        .map(|i| i.header_props.get("legacy_image_id").unwrap())
        .collect();
    assert_eq!(legacy.len(), 1);
}

#[test]
fn canonical_object_id_collision_blocks() {
    let asset = image("assets/cover.png", "C:/vault");
    let planned = import_obsidian_vault(&ImportObsidianVaultArgs {
        images: vec![asset.clone()],
        ..args(vec![], vec![], "C:/vault")
    });
    let imported = import_obsidian_vault(&ImportObsidianVaultArgs {
        images: vec![asset],
        existing_images: vec![ObsidianExistingImage {
            id: planned.images[0].id.clone(),
            header_props: [
                ("source_vault".into(), json!("C:/other-vault")),
                ("source_relative_path".into(), json!("assets/cover.png")),
            ]
            .into_iter()
            .collect(),
        }],
        ..args(vec![], vec![], "C:/vault")
    });
    assert_eq!(
        imported.image_report.blocking_collisions[0].kind,
        "object-id-collision"
    );
    assert_eq!(
        imported.image_report.blocking_collisions[0].key,
        planned.images[0].id
    );
    assert!(!imported.image_report.can_write);
}

#[test]
fn duplicate_existing_canonical_ids_block() {
    let asset = image("assets/cover.png", "C:/vault");
    let planned = import_obsidian_vault(&ImportObsidianVaultArgs {
        images: vec![asset.clone()],
        ..args(vec![], vec![], "C:/vault")
    });
    let imported = import_obsidian_vault(&ImportObsidianVaultArgs {
        images: vec![asset],
        existing_images: vec![
            ObsidianExistingImage {
                id: planned.images[0].id.clone(),
                header_props: Map::new(),
            },
            ObsidianExistingImage {
                id: planned.images[0].id.clone(),
                header_props: Map::new(),
            },
        ],
        ..args(vec![], vec![], "C:/vault")
    });
    assert!(imported
        .image_report
        .blocking_collisions
        .iter()
        .any(|c| c.kind == "existing-canonical-id-duplicate" && c.key == planned.images[0].id));
    assert!(!imported.image_report.can_write);
}

#[test]
fn duplicate_existing_source_metadata_blocks() {
    let imported = import_obsidian_vault(&ImportObsidianVaultArgs {
        images: vec![image("assets/cover.png", "C:/vault")],
        existing_images: vec![
            ObsidianExistingImage {
                id: "image:first".into(),
                header_props: [
                    ("source_vault_key".into(), json!("c:/vault")),
                    ("source_relative_path_key".into(), json!("assets/cover.png")),
                ]
                .into_iter()
                .collect(),
            },
            ObsidianExistingImage {
                id: "image:second".into(),
                header_props: [
                    ("source_vault".into(), json!("C:/VAULT")),
                    ("source_relative_path".into(), json!("Assets\\Cover.png")),
                ]
                .into_iter()
                .collect(),
            },
        ],
        ..args(vec![], vec![], "C:/vault")
    });
    assert!(imported
        .image_report
        .blocking_collisions
        .iter()
        .any(|c| c.kind == "existing-source-duplicate"));
    assert!(!imported.image_report.can_write);
}
