//! Port of `tests/obsidianVault.test.ts` — image asset resolution,
//! collision preflights, and namespaced-id stability.

use std::collections::HashMap;

use serde_json::{json, Map};

use crate::obsidian::{
    build_image_asset_map, build_obsidian_related_import_plan, import_obsidian_vault,
    legacy_image_id, record_obsidian_image_import_failure, ImportObsidianVaultArgs,
    ObsidianExistingImage, ObsidianImportDraft,
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
fn resolves_encoded_and_parent_relative_but_reports_duplicate_basenames() {
    let imported = import_obsidian_vault(&ImportObsidianVaultArgs {
        files: vec![md_at(
            "C:/vault",
            "notes/A.md",
            "![](../project-a/assets/cover.png)\n![](../project-b/assets/My%20Cover.png)\n![](cover.png)",
        )],
        images: vec![
            image("project-a/assets/cover.png", "C:/vault"),
            image("project-b/assets/cover.png", "C:/vault"),
            image("project-b/assets/My Cover.png", "C:/vault"),
        ],
        ..args(vec![], vec![], "C:/vault")
    });
    let body = &imported.entries[0].body_markdown;
    assert!(body.contains("project-a/assets/cover.png"));
    assert!(body.contains("project-b/assets/My%20Cover.png"));
    assert!(body.contains("![](cover.png)"));
    assert_eq!(imported.image_report.resolved.len(), 2);
    assert_eq!(imported.image_report.ambiguous.len(), 1);
    assert!(imported.image_report.collisions.is_empty());
    assert!(!imported.image_report.can_write);
    let candidates: Vec<&str> = imported.image_report.ambiguous[0]
        .candidates
        .iter()
        .map(|i| i.relative_path.as_str())
        .collect();
    assert_eq!(
        candidates,
        ["project-a/assets/cover.png", "project-b/assets/cover.png"]
    );
}

#[test]
fn note_relative_exact_beats_vault_root_exact() {
    let imported = import_obsidian_vault(&ImportObsidianVaultArgs {
        files: vec![md_at("C:/vault", "notes/A.md", "![](assets/cover.png)")],
        images: vec![
            image("notes/assets/cover.png", "C:/vault"),
            image("assets/cover.png", "C:/vault"),
        ],
        ..args(vec![], vec![], "C:/vault")
    });
    assert_eq!(
        imported.image_report.resolved[0]
            .asset
            .as_ref()
            .unwrap()
            .relative_path,
        "notes/assets/cover.png"
    );
    assert!(imported.image_report.ambiguous.is_empty());
    assert!(imported.entries[0]
        .body_markdown
        .contains("C:/vault/notes/assets/cover.png"));
}

#[test]
fn case_only_path_collision_blocks_writes() {
    let imported = import_obsidian_vault(&ImportObsidianVaultArgs {
        files: vec![md_at("C:/vault", "A.md", "![](assets/COVER.png)")],
        images: vec![
            image("assets/cover.png", "C:/vault"),
            image("assets/Cover.png", "C:/vault"),
        ],
        ..args(vec![], vec![], "C:/vault")
    });
    assert_eq!(imported.image_report.ambiguous.len(), 1);
    assert_eq!(
        imported.image_report.collisions[0].kind,
        "case-only-relative-path"
    );
    assert!(!imported.image_report.can_write);
    assert_eq!(imported.entries[0].body_markdown, "![](assets/COVER.png)");
}

#[test]
fn duplicate_relative_paths_block_but_unused_basenames_do_not() {
    let index = build_image_asset_map(&[
        image("a/cover.png", "C:/vault"),
        image("b/cover.png", "C:/vault"),
    ]);
    assert!(index.path_collisions.is_empty());

    let imported = import_obsidian_vault(&ImportObsidianVaultArgs {
        images: vec![
            image("assets/cover.png", "C:/vault"),
            image("assets/cover.png", "C:/vault"),
        ],
        ..args(vec![], vec![], "C:/vault")
    });
    assert_eq!(imported.image_report.collisions.len(), 1);
    assert_eq!(
        imported.image_report.collisions[0].kind,
        "duplicate-relative-path"
    );
    assert_eq!(imported.image_report.collided.len(), 2);
    assert!(!imported.image_report.can_write);

    let imported = import_obsidian_vault(&ImportObsidianVaultArgs {
        images: vec![
            image("project-a/cover.png", "C:/vault"),
            image("project-b/cover.png", "C:/vault"),
        ],
        ..args(vec![], vec![], "C:/vault")
    });
    assert!(imported.image_report.collisions.is_empty());
    assert!(imported.image_report.ambiguous.is_empty());
    assert_eq!(imported.image_report.imported.len(), 2);
    assert!(imported.image_report.can_write);
}

#[test]
fn references_outside_the_vault_are_invalid() {
    let imported = import_obsidian_vault(&ImportObsidianVaultArgs {
        files: vec![md_at(
            "C:/vault",
            "notes/day/A.md",
            "![](../../../../outside.png)",
        )],
        images: vec![image("outside.png", "C:/vault")],
        ..args(vec![], vec![], "C:/vault")
    });
    assert_eq!(imported.image_report.invalid.len(), 1);
    assert_eq!(imported.image_report.skipped.len(), 1);
    assert_eq!(
        imported.entries[0].body_markdown,
        "![](../../../../outside.png)"
    );
}

#[test]
fn absolute_windows_image_path_is_invalid() {
    let imported = import_obsidian_vault(&ImportObsidianVaultArgs {
        files: vec![md_at("C:/vault", "A.md", "![](D:/outside.png)")],
        images: vec![image("outside.png", "D:/")],
        ..args(vec![], vec![], "C:/vault")
    });
    assert_eq!(imported.image_report.invalid.len(), 1);
    assert!(!imported.image_report.can_write);
}

#[test]
fn failure_reporting_covers_reused_and_skipped() {
    let asset = image("assets/cover.png", "C:/vault");
    let mut reused = import_obsidian_vault(&ImportObsidianVaultArgs {
        images: vec![asset.clone()],
        existing_images: vec![ObsidianExistingImage {
            id: "image:legacy".into(),
            header_props: [
                ("source_vault".into(), json!("C:/vault")),
                ("source_relative_path".into(), json!("assets/cover.png")),
                (
                    "legacy_image_id".into(),
                    json!(legacy_image_id(&asset.relative_path)),
                ),
            ]
            .into_iter()
            .collect(),
        }],
        ..args(vec![], vec![], "C:/vault")
    });
    record_obsidian_image_import_failure(&mut reused.image_report, &reused.images[0].id);
    assert_eq!(reused.image_report.failed, reused.image_report.reused);

    let mut skipped = import_obsidian_vault(&ImportObsidianVaultArgs {
        images: vec![image("../outside.png", "C:/vault")],
        ..args(vec![], vec![], "C:/vault")
    });
    record_obsidian_image_import_failure(&mut skipped.image_report, &skipped.images[0].id);
    assert_eq!(skipped.image_report.failed, skipped.image_report.skipped);
}

#[test]
fn unresolved_wikilinks_do_not_block() {
    let draft = ObsidianImportDraft {
        title: "A".into(),
        wikilinks: vec!["Missing note".into()],
        ..Default::default()
    };
    let plan = build_obsidian_related_import_plan(&draft, "a", &HashMap::new(), None);
    assert!(plan.second_pass_related_ids.is_empty());
    assert_eq!(plan.unresolved_targets, Some(vec!["Missing note".into()]));
}

#[test]
fn unknown_source_type_falls_back_and_stays_visible() {
    let imported = import_obsidian_vault(&ImportObsidianVaultArgs {
        files: vec![md_at(
            "C:/vault",
            "a.md",
            "---\ntype: custom_type\n---\nbody",
        )],
        ..args(vec![], vec![], "C:/vault")
    });
    assert_eq!(imported.entries[0].type_id, "note_obj");
    assert_eq!(
        imported.entries[0].header_props["source_type_id"],
        json!("custom_type")
    );
}
