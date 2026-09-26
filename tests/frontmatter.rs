//! `markdownFrontmatter.ts` edge cases from the parity review.

use memoria_gpui::frontmatter::build_entry_markdown_document;
use memoria_gpui::model::Entry;

#[test]
fn empty_type_id_falls_back_like_js_or() {
    // `entry.type_id || "note_obj"` — "" is falsy in JS, so the frontmatter
    // `type` field never emits an empty string.
    let entry = Entry {
        id: "n1".into(),
        title: "Заметка".into(),
        type_id: Some("".into()),
        ..Default::default()
    };
    let doc = build_entry_markdown_document(&entry, None, "тело", None).unwrap();
    // Top-level `type` uses `||` → "note_obj"; `eden.type` uses `??` → ""
    // stays (nullish ≠ falsy), matching Vue byte-for-byte.
    assert!(doc.contains("\ntitle: Заметка\ntype: note_obj"), "{doc}");
    assert!(doc.contains("eden:\n  id: n1\n  type: "), "{doc}");
}
