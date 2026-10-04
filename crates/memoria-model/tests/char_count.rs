//! Port of `tests/charCount.test.ts` — zen-mode counter counting.

use memoria_model::char_count::{
    count_chars_in_prose_mirror_doc, count_chars_in_prose_mirror_node,
};
use serde_json::{json, Value};

fn doc(content: Value) -> String {
    serde_json::to_string(&json!({ "type": "doc", "content": content })).unwrap()
}

fn p(content: Value) -> Value {
    json!({ "type": "paragraph", "content": content })
}

fn t(text: &str) -> Value {
    json!({ "type": "text", "text": text })
}

fn count(json: &str) -> Option<usize> {
    count_chars_in_prose_mirror_doc(Some(json))
}

#[test]
fn null_and_empty_and_invalid_json_return_none() {
    assert_eq!(count_chars_in_prose_mirror_doc(None), None);
    assert_eq!(count_chars_in_prose_mirror_doc(Some("")), None);
    assert_eq!(count("{not json"), None);
}

#[test]
fn empty_doc_and_empty_paragraph_count_zero() {
    assert_eq!(count(&doc(json!([]))), Some(0));
    assert_eq!(count(&doc(json!([p(json!([]))]))), Some(0));
}

#[test]
fn single_paragraph_counts_scalars() {
    assert_eq!(count(&doc(json!([p(json!([t("hello")]))]))), Some(5));
    assert_eq!(count(&doc(json!([p(json!([t("привет")]))]))), Some(6));
}

#[test]
fn paragraph_breaks_count_as_newlines() {
    // hello\nworld = 11, three paragraphs = a\nb\nc = 5.
    let two = doc(json!([p(json!([t("hello")])), p(json!([t("world")]))]));
    assert_eq!(count(&two), Some(11));
    let three = doc(json!([
        p(json!([t("a")])),
        p(json!([t("b")])),
        p(json!([t("c")]))
    ]));
    assert_eq!(count(&three), Some(5));
}

#[test]
fn empty_paragraph_between_content_counts_two_separators() {
    // a\n\nb = 4.
    let json = doc(json!([
        p(json!([t("a")])),
        p(json!([])),
        p(json!([t("b")]))
    ]));
    assert_eq!(count(&json), Some(4));
}

#[test]
fn hard_break_counts_one() {
    let json = doc(json!([p(json!([t("a"), { "type": "hardBreak" }, t("b")]))]));
    assert_eq!(count(&json), Some(3));
    let snake = doc(json!([p(
        json!([t("a"), { "type": "hard_break" }, t("b")])
    )]));
    assert_eq!(count(&snake), Some(3));
}

#[test]
fn emoji_surrogate_pairs_count_as_one() {
    assert_eq!(count(&doc(json!([p(json!([t("😀")]))]))), Some(1));
    assert_eq!(count(&doc(json!([p(json!([t("hi 😀")]))]))), Some(4));
    assert_eq!(count(&doc(json!([p(json!([t("👋🚀🔥")]))]))), Some(3));
}

#[test]
fn heading_plus_paragraph_counts_the_separator() {
    let json = doc(json!([
        { "type": "heading", "attrs": { "level": 1 }, "content": [t("Заголовок")] },
        p(json!([t("текст")])),
    ]));
    assert_eq!(count(&json), Some(15));
}

#[test]
fn lists_and_containers_add_no_own_newlines() {
    let list = doc(json!([{
        "type": "bulletList",
        "content": [
            { "type": "listItem", "content": [p(json!([t("first")]))] },
            { "type": "listItem", "content": [p(json!([t("second")]))] },
        ],
    }]));
    assert_eq!(count(&list), Some(12));

    let quote = doc(json!([
        p(json!([t("intro")])),
        { "type": "blockquote", "content": [p(json!([t("quoted")]))] },
        p(json!([t("outro")])),
    ]));
    assert_eq!(count(&quote), Some(18));
}

#[test]
fn code_block_counts_as_leaf_block() {
    let json = doc(json!([
        p(json!([t("before")])),
        { "type": "codeBlock", "content": [t("const x = 1;")] },
        p(json!([t("after")])),
    ]));
    assert_eq!(count(&json), Some(25));
}

#[test]
fn multiple_text_nodes_in_one_paragraph_sum_without_newlines() {
    let json = doc(json!([p(json!([
        t("hello "),
        { "type": "text", "text": "world", "marks": [{ "type": "bold" }] },
    ]))]));
    assert_eq!(count(&json), Some(11));
}

#[test]
fn node_counter_works_on_parsed_values() {
    let node = json!({
        "type": "doc",
        "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "raw" }] }],
    });
    assert_eq!(count_chars_in_prose_mirror_node(&node), 3);
}
