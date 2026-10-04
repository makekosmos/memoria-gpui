//! Port of `tests/content.test.ts` — `editor-content/content.ts` codec.

use memoria_model::content::{
    is_entry_tiptap_content, is_markdown_content, is_readable_entry_content,
    legacy_prose_mirror_to_text, markdown_to_tiptap_doc, read_entry_markdown,
    read_entry_tiptap_doc, tiptap_doc_to_markdown, write_entry_markdown, write_entry_tiptap_doc,
};
use serde_json::{json, Value};

#[test]
fn markdown_reads_exact_text_and_writes_envelope() {
    let text = "# Заголовок\n\n- [ ] задача\n\nстрока  ";
    let content = write_entry_markdown(text);

    assert_eq!(
        content,
        json!({ "type": "markdown", "version": 1, "text": text })
    );
    assert!(is_markdown_content(&content));
    assert_eq!(read_entry_markdown(&content), text);
    let serialized = serde_json::to_string(&content).unwrap();
    assert_eq!(read_entry_markdown(&Value::from(serialized)), text);
    assert_eq!(
        write_entry_markdown(&read_entry_markdown(&content))["text"],
        Value::from(text)
    );
}

#[test]
fn invalid_and_empty_content_reads_empty_markdown() {
    for value in [
        Value::Null,
        Value::from(""),
        Value::from("{not-json"),
        json!({}),
        json!({ "type": "unknown" }),
    ] {
        assert_eq!(read_entry_markdown(&value), "");
    }
    assert!(!is_readable_entry_content(&Value::from("{not-json")));
    assert!(is_readable_entry_content(&write_entry_markdown("")));
}

#[test]
fn legacy_prosemirror_extracts_text_and_media_markdown() {
    let legacy = json!({
        "type": "doc",
        "content": [
            { "type": "heading", "attrs": { "level": 2 },
              "content": [{ "type": "text", "text": "Привет" }] },
            { "type": "paragraph", "content": [
                { "type": "text", "text": "строка", "marks": [{ "type": "strong" }] },
                { "type": "hardBreak" },
                { "type": "text", "text": "после",
                  "marks": [{ "type": "link", "attrs": { "href": "https://example.com" } }] },
            ]},
            { "type": "codeBlock", "attrs": { "language": "ts" },
              "content": [{ "type": "text", "text": "const x = 1;" }] },
            { "type": "image", "attrs": { "src": "file:///vault/pic.png" } },
            { "type": "unknownWrapper",
              "content": [{ "type": "text", "text": "nested" }] },
        ],
    });

    let markdown = legacy_prose_mirror_to_text(&legacy);
    assert!(markdown.contains("## Привет"));
    assert!(markdown.contains("**строка**\n[после](https://example.com)"));
    assert!(markdown.contains("```ts\nconst x = 1;\n```"));
    assert!(markdown.contains("![](file:///vault/pic.png)"));
    assert!(markdown.contains("nested"));
}

#[test]
fn image_only_legacy_content_preserves_source_attributes() {
    let value = json!({
        "type": "doc",
        "content": [{ "type": "image", "attrs": { "url": "https://x/y.png" } }],
    });
    assert_eq!(legacy_prose_mirror_to_text(&value), "![](https://x/y.png)");
}

#[test]
fn markdown_migrates_to_tiptap_and_reads_back() {
    let markdown = [
        "# Title",
        "",
        "- [x] done",
        "- [ ] next",
        "",
        "1. first",
        "",
        "> quote",
        "",
        "```",
        "code",
        "```",
    ]
    .join("\n");

    let doc = markdown_to_tiptap_doc(&markdown);
    let content = write_entry_tiptap_doc(doc.clone());

    assert_eq!(content["type"], Value::from("tiptap"));
    let serialized = serde_json::to_string(&content).unwrap();
    assert!(is_entry_tiptap_content(&Value::from(serialized)));
    assert_eq!(read_entry_tiptap_doc(&write_entry_markdown(&markdown)), doc);
    assert!(read_entry_markdown(&content).contains("- [x] done"));
    assert!(tiptap_doc_to_markdown(&doc).contains("```"));
}

#[test]
fn markdown_round_trips_marks_links_code_images_lists_quotes_fences() {
    let markdown = [
        "## Heading with **bold** and *italic*",
        "",
        "Paragraph with [link](https://example.com), `code`, and \
         ![alt](https://cdn.test/img.png \"caption\")",
        "",
        "- bullet with ~~strike~~",
        "3. ordered",
        "- [x] task",
        "",
        "> quoted **text**",
        "",
        "```ts",
        "const value = 1;",
        "```",
    ]
    .join("\n");

    let round_trip = tiptap_doc_to_markdown(&markdown_to_tiptap_doc(&markdown));

    assert!(round_trip.contains("Paragraph with [link](https://example.com), `code`, and"));
    assert!(round_trip.contains("![alt](https://cdn.test/img.png \"caption\")"));
    assert!(round_trip.contains("- bullet with ~~strike~~"));
    assert!(round_trip.contains("```ts\nconst value = 1;\n```"));
}

#[test]
fn inline_image_becomes_block_image_node() {
    let doc = markdown_to_tiptap_doc("before ![alt](https://cdn.test/img.png) after");
    let content = doc["content"].as_array().unwrap();
    let types: Vec<&str> = content.iter().filter_map(|n| n["type"].as_str()).collect();
    assert_eq!(types, ["paragraph", "image", "paragraph"]);
    assert_eq!(content[0]["content"][0]["text"], Value::from("before "));
    assert_eq!(content[2]["content"][0]["text"], Value::from(" after"));
}

#[test]
fn paragraph_line_breaks_become_hard_breaks() {
    let doc = read_entry_tiptap_doc(&write_entry_markdown("alpha\nbeta"));
    assert_eq!(
        doc["content"][0],
        json!({
            "type": "paragraph",
            "content": [
                { "type": "text", "text": "alpha" },
                { "type": "hardBreak" },
                { "type": "text", "text": "beta" },
            ],
        })
    );
}

#[test]
fn nested_lists_preserve_child_indentation() {
    let markdown = "- parent\n  - child\n- sibling";
    assert_eq!(
        tiptap_doc_to_markdown(&markdown_to_tiptap_doc(markdown)),
        markdown
    );
}

#[test]
fn code_fences_expand_when_code_contains_triple_backticks() {
    let doc = json!({
        "type": "doc",
        "content": [{
            "type": "codeBlock",
            "content": [{ "type": "text", "text": "before\n```\nafter" }],
        }],
    });

    let markdown = tiptap_doc_to_markdown(&doc);
    assert_eq!(markdown, "````\nbefore\n```\nafter\n````");
    assert_eq!(read_entry_tiptap_doc(&write_entry_markdown(&markdown)), doc);
}

#[test]
fn tiptap_doc_with_marks_and_images_reads_to_markdown() {
    let doc = json!({
        "type": "doc",
        "content": [
            { "type": "paragraph", "content": [
                { "type": "text", "text": "bold", "marks": [{ "type": "bold" }] },
                { "type": "text", "text": " " },
                { "type": "text", "text": "link",
                  "marks": [{ "type": "link", "attrs": { "href": "https://example.com" } }] },
            ]},
            { "type": "image",
              "attrs": { "src": "https://cdn.test/pic.png", "alt": "pic" } },
        ],
    });

    assert_eq!(
        read_entry_markdown(&write_entry_tiptap_doc(doc)),
        "**bold** [link](https://example.com)\n\n![pic](https://cdn.test/pic.png)"
    );
}

#[test]
fn inline_code_fence_grows_for_inner_backtick_runs() {
    // KOS-249: `wrapInlineCode` always used `` `` `` — code text containing a
    // `` `` `` run emitted "`` a``b ``", which re-parses as nested spans and
    // corrupts the document on the next read.
    let doc = json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [
                { "type": "text", "text": "use " },
                { "type": "text", "text": "a``b", "marks": [{ "type": "code" }] },
                { "type": "text", "text": " ok" },
            ]
        }],
    });

    let markdown = tiptap_doc_to_markdown(&doc);
    assert_eq!(markdown, "use ``` a``b ``` ok");
    assert_eq!(read_entry_tiptap_doc(&write_entry_markdown(&markdown)), doc);
}
