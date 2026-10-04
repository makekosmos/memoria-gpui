//! Port of `tests/obsidianVault.export.test.ts` — export plans: safe
//! filenames, folder structure, asset copying, manifests, dedupe.

use std::collections::{HashMap, HashSet};

use serde_json::json;

use crate::obsidian::{
    build_obsidian_export_files, export_obsidian_vault_markdown_files, BuildObsidianExportFilesArgs,
};

use super::{custom_note_type, entry, folders, make_note_type};

#[test]
fn safe_filename_and_frontmatter_via_document_builder() {
    let mut entry = entry("entry-1", "My: Plan/2026", "custom-note");
    entry.header_props_json = Some(
        serde_json::to_string(&json!({
            "summary": "Draft",
            "related_notes": ["entry-2"],
        }))
        .unwrap(),
    );
    entry.schema_version = Some(2);
    let files = export_obsidian_vault_markdown_files(
        vec![entry],
        &[custom_note_type()],
        &HashMap::from([("entry-1".into(), "Hello body".into())]),
        &HashMap::from([("entry-2".into(), "Second Note".into())]),
    )
    .unwrap();
    assert_eq!(files.len(), 1);
    let content = files[0].content.as_deref().unwrap();
    assert_eq!(files[0].relative_path, "My-Plan-2026.md");
    assert!(content.contains("---\n"));
    assert!(content.contains("title: \"My: Plan/2026\""));
    assert!(content.contains("type: custom-note"));
    assert!(content.contains("summary: Draft"));
    assert!(content.contains("links:"));
    assert!(content.contains("- \"[[entry-2|Second Note]]\""));
    assert!(content.contains("Hello body"));
}

#[test]
fn preserves_folder_structure_and_excludes_unselected_types() {
    let folder_paths = folders();
    let mut a = entry("entry-a", "Сводка", "custom-note");
    a.folder_id = Some("child".into());
    let mut b = entry("entry-b", "Сводка", "custom-note");
    b.folder_id = Some("child".into());
    let mut task = entry("task-1", "Сводка", "task_obj");
    task.folder_id = Some("child".into());
    let files = build_obsidian_export_files(&BuildObsidianExportFilesArgs {
        entries: vec![a, b, task],
        note_types: &[
            custom_note_type(),
            make_note_type("task_obj", "Task", "task"),
        ],
        body_markdown: Box::new(|entry| format!("Body {}", entry.id)),
        title_lookup: None,
        selected_type_ids: Some(HashSet::from(["custom-note".into()])),
        folder_path_by_id: Some(&folder_paths),
    })
    .unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f.relative_path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "Projects/2026 Alpha/Сводка.md",
            "Projects/2026 Alpha/Сводка-2.md"
        ]
    );
    assert!(files.iter().all(|f| !f.relative_path.contains("task")));
    assert!(files[0]
        .content
        .as_deref()
        .unwrap()
        .contains("Body entry-a"));
    assert!(files[1]
        .content
        .as_deref()
        .unwrap()
        .contains("Body entry-b"));
}

#[test]
fn dedupes_copied_asset_targets_on_filename_collisions() {
    let files = build_obsidian_export_files(&BuildObsidianExportFilesArgs {
        entries: vec![
            entry("entry-a", "A", "custom-note"),
            entry("entry-b", "B", "custom-note"),
        ],
        note_types: &[custom_note_type()],
        body_markdown: Box::new(|entry| {
            if entry.id == "entry-a" {
                "![](file:///C:/vault/a/diagram.png)".to_string()
            } else {
                "![](file:///C:/vault/b/diagram.png)".to_string()
            }
        }),
        title_lookup: None,
        selected_type_ids: None,
        folder_path_by_id: None,
    })
    .unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f.relative_path.as_str()).collect();
    assert_eq!(
        paths,
        ["A.md", "B.md", "assets/diagram.png", "assets/diagram-2.png"]
    );
    assert!(files[0]
        .content
        .as_deref()
        .unwrap()
        .contains("assets/diagram.png"));
    assert!(files[1]
        .content
        .as_deref()
        .unwrap()
        .contains("assets/diagram-2.png"));
    assert_eq!(
        files[2].source_path.as_deref(),
        Some("file:///C:/vault/a/diagram.png")
    );
    assert_eq!(
        files[3].source_path.as_deref(),
        Some("file:///C:/vault/b/diagram.png")
    );
}
