//! Port of `tests/obsidianVault.test.ts` — import planning: frontmatter,
//! wikilinks, image resolution/reconciliation, namespaced ids, preflights.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use crate::obsidian::{
    build_obsidian_related_import_plan, create_obsidian_vault_import_drafts, ObsidianImportDraft,
    ObsidianVaultImageFile,
};

use super::{custom_note_type, image, md};

#[test]
fn no_frontmatter_becomes_body_and_filename_title() {
    let drafts = create_obsidian_vault_import_drafts(
        &[md("notes/My Note.md", "First line\n\nSecond line")],
        &[],
        &[custom_note_type()],
        "note_obj",
    );
    assert_eq!(drafts.len(), 1);
    let draft = &drafts[0];
    assert_eq!(draft.draft.source_path, "vault/notes/My Note.md");
    assert_eq!(draft.draft.relative_path, "notes/My Note.md");
    assert_eq!(draft.name, "My Note.md");
    assert_eq!(draft.draft.title, "My Note");
    assert_eq!(draft.draft.type_id, "note_obj");
    assert_eq!(draft.draft.body_markdown, "First line\n\nSecond line");
    assert!(draft.draft.header_props.is_empty());
    assert!(draft.title_references.is_empty() && draft.image_references.is_empty());
}

#[test]
fn wikilinks_from_body_and_frontmatter_with_preserved_header_props() {
    let drafts = create_obsidian_vault_import_drafts(
        &[md(
            "notes/Obsidian Note.md",
            &[
                "---",
                "title: Vault Note",
                "eden:",
                "  id: note-1",
                "  type: custom-note",
                "project: \"Alpha\"",
                "links:",
                "  related:",
                "    - \"[[Front Link]]\"",
                "notes: \"See [[Front Matter Link]]\"",
                "---",
                "",
                "Intro [[Body Link|alias]]",
                "![](attachments/photo.png \"Photo\")",
            ]
            .join("\n"),
        )],
        &[],
        &[custom_note_type()],
        "note_obj",
    );
    assert_eq!(drafts.len(), 1);
    let draft = &drafts[0];
    assert_eq!(draft.entry_id.as_deref(), Some("note-1"));
    assert_eq!(draft.draft.title, "Vault Note");
    assert_eq!(draft.draft.type_id, "custom-note");
    assert_eq!(
        draft.draft.body_markdown,
        "\nIntro [[Body Link|alias]]\n![](attachments/photo.png \"Photo\")"
    );
    assert_eq!(draft.draft.header_props["project"], json!("Alpha"));
    assert_eq!(
        draft.draft.header_props["notes"],
        json!("See [[Front Matter Link]]")
    );
    assert_eq!(
        draft.title_references,
        ["Body Link", "Front Link", "Front Matter Link"]
    );
    assert_eq!(draft.image_references, ["attachments/photo.png"]);
}

/// Vue fixture with explicit mime/dimensions.
fn image_spec(relative_path: &str, mime: &str, w: u32, h: u32) -> ObsidianVaultImageFile {
    let mut file = image(relative_path, "vault");
    file.mime_type = mime.into();
    file.width = Some(w);
    file.height = Some(h);
    file
}

#[test]
fn loose_frontmatter_and_spaced_image_paths() {
    let drafts = create_obsidian_vault_import_drafts(
        &[md(
            "Дневник/2026-06-12.md",
            &[
                "\u{FEFF}---   ",
                "title: 2026-06-12",
                "tags: [\"дневник\", \"важное\"]",
                "aliases: [Сегодня, \"Daily note\"]",
                "Автор: \"[[Я]]\"",
                "...",
                "",
                "Текст [[Project Alpha#Intro|intro]].",
                "![scan](../Медиа/My Image 01.webp \"scan\")",
                "![[Another Image.webp|300x200]]",
            ]
            .join("\n"),
        )],
        &[
            image_spec("Медиа/My Image 01.webp", "image/webp", 800, 600),
            image_spec("Медиа/Another Image.webp", "image/webp", 300, 200),
        ],
        &[custom_note_type()],
        "note_obj",
    );
    let draft = &drafts[0];
    assert_eq!(draft.draft.title, "2026-06-12");
    assert_eq!(draft.draft.type_id, "note_obj");
    assert_eq!(
        draft.draft.header_props["tags"],
        json!(["дневник", "важное"])
    );
    assert_eq!(
        draft.draft.header_props["aliases"],
        json!(["Сегодня", "Daily note"])
    );
    assert_eq!(draft.draft.header_props["Автор"], json!("[[Я]]"));
    assert_eq!(draft.title_references, ["Project Alpha#Intro", "Я"]);
    assert_eq!(
        draft.image_references,
        [
            "file:///vault/%D0%9C%D0%B5%D0%B4%D0%B8%D0%B0/My%20Image%2001.webp",
            "file:///vault/%D0%9C%D0%B5%D0%B4%D0%B8%D0%B0/Another%20Image.webp",
        ]
    );
    assert!(draft
        .draft
        .body_markdown
        .contains("![scan](file:///vault/%D0%9C%D0%B5%D0%B4%D0%B8%D0%B0/My%20Image%2001.webp)"));
    assert!(draft.draft.body_markdown.contains(
        "![Another Image](file:///vault/%D0%9C%D0%B5%D0%B4%D0%B8%D0%B0/Another%20Image.webp)"
    ));
}

#[test]
fn related_plan_waits_for_second_pass() {
    let draft = ObsidianImportDraft {
        title: "A".into(),
        wikilinks: vec!["B#Heading".into(), "Existing.md".into(), "A".into()],
        header_props: [
            ("related_notes".to_string(), json!(["raw-obsidian-title"])),
            ("source_path".to_string(), json!("vault/A.md")),
        ]
        .into_iter()
        .collect(),
        ..Default::default()
    };
    let imported: HashMap<String, String> = [
        ("a".into(), "entry-a".into()),
        ("b".into(), "entry-b".into()),
    ]
    .into_iter()
    .collect();
    let existing: HashMap<String, String> = [("existing".into(), "entry-existing".into())]
        .into_iter()
        .collect();
    let plan = build_obsidian_related_import_plan(&draft, "entry-a", &imported, Some(&existing));
    assert_eq!(
        plan.first_pass_header_props,
        [("source_path".to_string(), json!("vault/A.md"))]
            .into_iter()
            .collect::<Map<String, Value>>()
    );
    assert_eq!(plan.second_pass_related_ids, ["entry-b", "entry-existing"]);
}
