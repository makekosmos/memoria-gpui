//! `proptest` coverage for the content codec: markdown envelopes are exact,
//! tiptap envelopes normalize idempotently, and neither reader panics on
//! arbitrary trees (Engine data is untrusted).

use memoria_gpui::content::{
    is_entry_tiptap_content, read_entry_markdown, read_entry_tiptap_doc, write_entry_markdown,
    write_entry_tiptap_doc,
};
use proptest::prelude::*;
use serde_json::{json, Value};

/// Arbitrary JSON scalars/arrays/objects plus TipTap-flavoured nodes
/// (`type`/`text`/`attrs`/`marks`/`content`) so the walker sees real shapes.
fn arb_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        ".*".prop_map(Value::from),
    ];
    leaf.prop_recursive(4, 24, 8, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::vec((".*", inner.clone()), 0..6).prop_map(|entries| {
                entries
                    .into_iter()
                    .collect::<serde_json::Map<_, _>>()
                    .into()
            }),
            (
                prop_oneof!["text", "paragraph", "heading", "bulletList", "image"],
                inner.clone(),
                inner.clone(),
            )
                .prop_map(|(node_type, children, text)| json!({
                    "type": node_type,
                    "text": text,
                    "content": children,
                    "attrs": { "level": 1 },
                    "marks": [{ "type": "bold" }],
                })),
        ]
    })
}

proptest! {
    #[test]
    fn markdown_envelope_reads_back_exactly(text in ".*") {
        let wire = write_entry_markdown(&text);
        prop_assert_eq!(read_entry_markdown(&wire), text);
    }

    #[test]
    fn tiptap_envelope_round_trips_idempotently(doc in arb_value()) {
        let first = read_entry_tiptap_doc(&write_entry_tiptap_doc(doc));
        let second = read_entry_tiptap_doc(&write_entry_tiptap_doc(first.clone()));
        prop_assert_eq!(first, second);
    }

    #[test]
    fn readers_never_panic_on_arbitrary_json(value in arb_value()) {
        let _ = read_entry_markdown(&value);
        let _ = read_entry_tiptap_doc(&value);
        let _ = is_entry_tiptap_content(&value);
        // Serialized-string input exercises the JSON.parse path.
        let raw = serde_json::to_string(&value).unwrap();
        let _ = read_entry_markdown(&Value::from(raw));
    }
}
