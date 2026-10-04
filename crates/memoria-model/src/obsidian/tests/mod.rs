//! Ports of `tests/obsidianVault.test.ts`, `obsidianVault.export.test.ts`, and
//! `obsidianVaultImportTransaction.test.ts` — same fixtures, same assertions.

mod export;
mod export_assets;
mod fake_ark;
mod golden;
mod import;
mod import_ids;
mod import_images;
mod run;
mod run_e2e;
mod transaction;
mod transaction_recovery;

use crate::model::NoteType;
use crate::obsidian::{ObsidianVaultImageFile, ObsidianVaultMarkdownFile};

/// `makeNoteType` from `obsidianVault.helpers.ts`.
pub(super) fn make_note_type(id: &str, name: &str, slug: &str) -> NoteType {
    NoteType {
        id: id.to_string(),
        name: name.to_string(),
        slug: slug.to_string(),
        schema_json: serde_json::to_string(&serde_json::json!({ "fields": [] })).unwrap(),
        header_template_json: serde_json::to_string(&serde_json::json!({
            "kind": "default",
            "primaryFieldIds": [],
            "secondaryFieldIds": [],
            "imageFieldId": null
        }))
        .unwrap(),
        ui_schema_json: Some("{}".into()),
        ..Default::default()
    }
}

pub(super) fn custom_note_type() -> NoteType {
    make_note_type("custom-note", "Custom Note", "custom-note")
}

/// `makeNoteType` override shape — schema/header_template passed through.
pub(super) fn note_type(
    id: &str,
    name: &str,
    slug: &str,
    schema_json: &str,
    header_template_json: &str,
) -> NoteType {
    let mut t = make_note_type(id, name, slug);
    t.schema_json = schema_json.to_string();
    t.header_template_json = header_template_json.to_string();
    t
}

/// Image-object note type used by the Vue export fixtures.
pub(super) fn image_note_type() -> NoteType {
    note_type(
        "image_obj",
        "Image",
        "image",
        &serde_json::to_string(&serde_json::json!({
            "fields": [{
                "id": "image",
                "label": "Image",
                "kind": "image",
                "required": true,
                "visible": true
            }]
        }))
        .unwrap(),
        &serde_json::to_string(&serde_json::json!({
            "kind": "default",
            "primaryFieldIds": ["image"],
            "secondaryFieldIds": ["file_name"],
            "imageFieldId": "image"
        }))
        .unwrap(),
    )
}

/// Entry fixture — the Vue `entries` literals.
pub(super) fn entry(id: &str, title: &str, type_id: &str) -> crate::model::Entry {
    crate::model::Entry {
        id: id.into(),
        title: title.into(),
        type_id: Some(type_id.into()),
        content_json: "{}".into(),
        header_props_json: Some("{}".into()),
        schema_version: Some(1),
        ..Default::default()
    }
}

/// `encodeURI` — leaves `/` unescaped, percent-encodes everything else that
/// isn't an ASCII mark (space → `%20`, Cyrillic → UTF-8 `%XX` pairs).
pub(super) fn encode_uri(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' | b'/'
            )
        {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// `image(relativePath, root)` — the Vue fixture (`file:///root/rel` fileUrl).
pub(super) fn image(relative_path: &str, root: &str) -> ObsidianVaultImageFile {
    let name = relative_path.rsplit('/').next().unwrap_or(relative_path);
    ObsidianVaultImageFile {
        path: format!("{root}/{relative_path}"),
        relative_path: relative_path.to_string(),
        name: name.to_string(),
        file_url: format!("file:///{root}/{}", encode_uri(relative_path)),
        mime_type: "image/png".into(),
        size_bytes: 100,
        width: Some(10),
        height: Some(10),
    }
}

/// `md(path?, relativePath, name, content)` — `path` = `vault/{relative}`.
pub(super) fn md(relative_path: &str, content: &str) -> ObsidianVaultMarkdownFile {
    let name = relative_path.rsplit('/').next().unwrap_or(relative_path);
    ObsidianVaultMarkdownFile {
        path: format!("vault/{relative_path}"),
        relative_path: relative_path.to_string(),
        name: name.to_string(),
        content: content.to_string(),
    }
}

pub(super) fn md_at(root: &str, relative_path: &str, content: &str) -> ObsidianVaultMarkdownFile {
    ObsidianVaultMarkdownFile {
        path: format!("{root}/{relative_path}"),
        ..md(relative_path, content)
    }
}

/// `buildObsidianFolderPathLookup` fixture — Projects/2026 Alpha.
pub(super) fn folders() -> std::collections::HashMap<String, String> {
    crate::obsidian::build_obsidian_folder_path_lookup(&[
        crate::obsidian::ObsidianVaultFolder {
            id: "root".into(),
            name: "Projects".into(),
            parent_id: None,
        },
        crate::obsidian::ObsidianVaultFolder {
            id: "child".into(),
            name: "2026 Alpha".into(),
            parent_id: Some("root".into()),
        },
    ])
}
