//! Port of `tests/obsidianVault.export.test.ts` — asset copying, manifest
//! entries, and header-image reference rewrites.

use std::collections::HashSet;

use serde_json::json;

use crate::obsidian::{build_obsidian_export_files, BuildObsidianExportFilesArgs};

use super::{custom_note_type, entry, folders, image_note_type, make_note_type, note_type};
#[test]
fn copies_local_assets_rewrites_refs_and_manifests_unresolved() {
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
        body_markdown: Box::new(|entry| {
            if entry.id == "entry-a" {
                "Note body\n\n![](file:///C:/vault/attachments/diagram.png)\n![[media/photo 1.webp|cover]]\n![](../../../../evil.png)"
                    .to_string()
            } else {
                format!("Body {}", entry.id)
            }
        }),
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
            "Projects/2026 Alpha/Сводка-2.md",
            "assets/diagram.png",
            "assets/obsidian-asset-manifest.json",
        ]
    );
    let content = files[0].content.as_deref().unwrap();
    assert!(content.contains("../../assets/diagram.png"));
    assert!(content.contains("![[media/photo 1.webp|cover]]"));
    assert!(content.contains("![](../../../../evil.png)"));
    assert_eq!(files[2].relative_path, "assets/diagram.png");
    assert_eq!(
        files[2].source_path.as_deref(),
        Some("file:///C:/vault/attachments/diagram.png")
    );

    let manifest: serde_json::Value =
        serde_json::from_str(files[3].content.as_deref().unwrap()).unwrap();
    assert!(manifest["limitation"]
        .as_str()
        .unwrap()
        .contains("safe local source path"));
    let assets = manifest["assets"].as_array().unwrap();
    assert_eq!(assets.len(), 2);
    assert_eq!(assets[0]["source"], json!("media/photo 1.webp"));
    assert_eq!(assets[0]["sourcePath"], json!(null));
    assert_eq!(
        assets[0]["noteRelativePath"],
        json!("Projects/2026 Alpha/Сводка.md")
    );
    assert_eq!(
        assets[0]["targetRelativePath"],
        json!("assets/media/photo 1.webp")
    );
    assert_eq!(assets[0]["rewrittenRelativePath"], json!(null));
    assert_eq!(assets[0]["copyable"], json!(false));
    assert!(assets[0]["limitation"]
        .as_str()
        .unwrap()
        .contains("kept unchanged"));
    assert_eq!(assets[1]["source"], json!("../../../../evil.png"));
    assert_eq!(assets[1]["targetRelativePath"], json!("assets/evil.png"));
}

#[test]
fn copies_raw_windows_image_paths_and_preserves_extensions() {
    let folder_paths = folders();
    let raw_path = r"C:\vault\attachments\My Image 01.webp";
    let mut a = entry("entry-a", "Сводка", "custom-note");
    a.folder_id = Some("child".into());
    let files = build_obsidian_export_files(&BuildObsidianExportFilesArgs {
        entries: vec![a],
        note_types: &[custom_note_type()],
        body_markdown: Box::new(move |_| format!("Note body\n\n![]({raw_path})")),
        title_lookup: None,
        selected_type_ids: None,
        folder_path_by_id: Some(&folder_paths),
    })
    .unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f.relative_path.as_str()).collect();
    assert_eq!(
        paths,
        ["Projects/2026 Alpha/Сводка.md", "assets/My Image 01.webp"]
    );
    assert!(files[0]
        .content
        .as_deref()
        .unwrap()
        .contains("../../assets/My Image 01.webp"));
    assert_eq!(files[1].relative_path, "assets/My Image 01.webp");
    assert_eq!(files[1].source_path.as_deref(), Some(raw_path));
    assert!(!files
        .iter()
        .any(|f| f.relative_path == "assets/obsidian-asset-manifest.json"));
}

#[test]
fn exports_image_objects_as_assets_from_header_metadata() {
    let source_path = r"C:\eden\assets\stored-image";
    let mut image_entry = entry("image-1", "Vacation Photo", "image_obj");
    image_entry.header_props_json = Some(
        serde_json::to_string(&json!({
            "image": "kosmos-local-image://file/C%3A%5Ceden%5Cassets%5Cstored-image",
            "file_name": "Vacation Photo.JPG",
            "mime_type": "image/jpeg",
            "source_path": source_path,
        }))
        .unwrap(),
    );
    let files = build_obsidian_export_files(&BuildObsidianExportFilesArgs {
        entries: vec![image_entry],
        note_types: &[image_note_type()],
        body_markdown: Box::new(|_| String::new()),
        title_lookup: None,
        selected_type_ids: None,
        folder_path_by_id: None,
    })
    .unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f.relative_path.as_str()).collect();
    assert_eq!(paths, ["Vacation-Photo.md", "assets/Vacation Photo.JPG"]);
    assert!(files[0]
        .content
        .as_deref()
        .unwrap()
        .contains("image: \"assets/Vacation Photo.JPG\""));
    assert_eq!(files[1].relative_path, "assets/Vacation Photo.JPG");
    assert_eq!(files[1].source_path.as_deref(), Some(source_path));
    assert!(!files
        .iter()
        .any(|f| f.relative_path == "assets/obsidian-asset-manifest.json"));
}

#[test]
fn preserves_asset_extensions_from_inline_image_metadata() {
    let source_path = r"C:\eden\photos\portrait-source";
    let mut image_entry = entry("image-2", "Portrait", "image_obj");
    image_entry.header_props_json = Some(
        serde_json::to_string(&json!({
            "image": {
                "source_path": source_path,
                "file_name": "Portrait",
                "mime_type": "image/png",
            },
        }))
        .unwrap(),
    );
    let files = build_obsidian_export_files(&BuildObsidianExportFilesArgs {
        entries: vec![image_entry],
        note_types: &[image_note_type()],
        body_markdown: Box::new(|_| String::new()),
        title_lookup: None,
        selected_type_ids: None,
        folder_path_by_id: None,
    })
    .unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f.relative_path.as_str()).collect();
    assert_eq!(paths, ["Portrait.md", "assets/Portrait.png"]);
    assert!(files[0]
        .content
        .as_deref()
        .unwrap()
        .contains("image: assets/Portrait.png"));
    assert_eq!(files[1].relative_path, "assets/Portrait.png");
    assert_eq!(files[1].source_path.as_deref(), Some(source_path));
    assert!(!files
        .iter()
        .any(|f| f.relative_path == "assets/obsidian-asset-manifest.json"));
}

#[test]
fn rewrites_header_image_refs_when_image_objects_excluded() {
    let source_path = r"C:\eden\images\Profile Portrait.png";
    let person_type = note_type(
        "person_obj",
        "Person",
        "person",
        &serde_json::to_string(&json!({
            "fields": [{
                "id": "photo",
                "label": "Photo",
                "kind": "image",
                "required": false,
                "visible": true
            }]
        }))
        .unwrap(),
        &serde_json::to_string(&json!({
            "kind": "default",
            "primaryFieldIds": [],
            "secondaryFieldIds": ["photo"],
            "imageFieldId": "photo"
        }))
        .unwrap(),
    );
    let mut person = entry("person-1", "Ada", "person_obj");
    person.header_props_json = Some(serde_json::to_string(&json!({ "photo": "image-1" })).unwrap());
    let mut image_entry = entry("image-1", "Profile Portrait", "image_obj");
    image_entry.header_props_json = Some(
        serde_json::to_string(&json!({
            "image": "file:///C:/eden/images/Profile%20Portrait.png",
            "file_name": "Profile Portrait.png",
            "mime_type": "image/png",
            "source_path": source_path,
        }))
        .unwrap(),
    );
    let files = build_obsidian_export_files(&BuildObsidianExportFilesArgs {
        entries: vec![person, image_entry],
        note_types: &[person_type, image_note_type()],
        body_markdown: Box::new(|_| String::new()),
        title_lookup: None,
        selected_type_ids: Some(HashSet::from(["person_obj".into()])),
        folder_path_by_id: None,
    })
    .unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f.relative_path.as_str()).collect();
    assert_eq!(paths, ["Ada.md", "assets/Profile Portrait.png"]);
    assert!(files[0]
        .content
        .as_deref()
        .unwrap()
        .contains("photo: \"assets/Profile Portrait.png\""));
    assert_eq!(files[1].relative_path, "assets/Profile Portrait.png");
    assert_eq!(files[1].source_path.as_deref(), Some(source_path));
    assert!(!files
        .iter()
        .any(|f| f.relative_path == "assets/obsidian-asset-manifest.json"));
}
