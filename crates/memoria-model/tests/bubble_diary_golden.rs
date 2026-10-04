//! Golden + property coverage for the diary storage codec and render
//! model. `fixtures/diary-bubbles.json` is a `JSON.stringify`-ordered
//! local-bubbles blob (compact, key order `version`/`journalImported`/
//! `bubbles`, bubble keys `id`/`date`/`time`/`sortKey`/`text`/
//! `contentJson`/`tags`/`kind`) — decode → encode must be byte-identical,
//! and arbitrary JSON must never panic normalize/decode/render.

use memoria_model::diary::{
    decode_local_bubbles_sources, decode_local_bubbles_storage, encode_local_bubbles_storage,
    normalize_local_bubbles, render_blocks, RenderBlock,
};
use proptest::prelude::*;
use serde_json::{json, Value};

const FIXTURE: &str = include_str!("../fixtures/diary-bubbles.json");

#[test]
fn local_bubbles_blob_reencodes_byte_identically() {
    let blob: Value = serde_json::from_str(FIXTURE).unwrap();
    let nodes = decode_local_bubbles_storage(&blob);
    assert_eq!(nodes.len(), 3, "fixture bubbles must all decode");
    let encoded = encode_local_bubbles_storage(&nodes, blob["journalImported"].as_bool().unwrap());
    assert_eq!(
        format!("{encoded}\n"),
        FIXTURE,
        "storage blob must round-trip byte-for-byte (JSON.stringify order)"
    );
}

#[test]
fn render_model_maps_fixture_blocks() {
    let blob: Value = serde_json::from_str(FIXTURE).unwrap();
    let nodes = decode_local_bubbles_storage(&blob);

    let blocks = render_blocks(nodes[0].content_json.as_ref(), &nodes[0].text);
    assert_eq!(blocks.len(), 3);
    match &blocks[0] {
        RenderBlock::Paragraph(runs) => {
            assert_eq!(runs.len(), 2);
            assert!(!runs[0].bold && runs[1].bold);
        }
        other => panic!("expected paragraph, got {other:?}"),
    }
    assert!(matches!(&blocks[1], RenderBlock::Heading { level: 2, .. }));
    match &blocks[2] {
        RenderBlock::TaskList(items) => {
            assert_eq!(items.len(), 2);
            assert!(items[0].0 && !items[1].0);
        }
        other => panic!("expected task list, got {other:?}"),
    }

    // Unknown `kanbanBoard` node degrades to visible text, not a drop.
    let blocks = render_blocks(nodes[1].content_json.as_ref(), &nodes[1].text);
    match blocks.last() {
        Some(RenderBlock::Unknown { type_name, text }) => {
            assert_eq!(type_name, "kanbanBoard");
            assert!(text.contains("колонка А"));
        }
        other => panic!("expected unknown block, got {other:?}"),
    }

    let blocks = render_blocks(nodes[2].content_json.as_ref(), &nodes[2].text);
    assert!(matches!(&blocks[1], RenderBlock::Image { .. }));
    assert!(matches!(&blocks[2], RenderBlock::HorizontalRule));
}

/// Arbitrary JSON scalars/arrays/objects plus tiptap-flavoured nodes.
fn arb_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        // serde_json's float parser can lose a ulp above 2^53; bound to
        // the range where f64 ↔ JSON text round-trips exactly.
        (-9.0e15..9.0e15).prop_map(Value::from),
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
        ]
    })
}

/// Well-formed-enough bubble records (required string fields present) so
/// normalization accepts them and exercises the full codec.
fn arb_bubble_record() -> impl Strategy<Value = Value> {
    (
        ".{1,12}",
        prop::option::of("[0-9]{4}-[0-9]{2}-[0-9]{2}"),
        "([0-9]{2}:[0-9]{2})?",
        prop::option::of(
            (-9_007_199_254_740_992i64..=9_007_199_254_740_992).prop_map(|n| n as f64),
        ),
        ".*",
        prop::option::of(arb_value()),
        prop::collection::vec(".*", 0..4),
        prop_oneof![
            Just("plain"),
            Just("idea"),
            Just("task"),
            Just("highlight"),
            Just("bogus"),
        ],
    )
        .prop_map(
            |(id, date, time, sort_key, text, content_json, tags, kind)| {
                let mut record = serde_json::Map::new();
                record.insert("id".into(), json!(id));
                if let Some(d) = date {
                    record.insert("date".into(), json!(d));
                }
                record.insert("time".into(), json!(time));
                if let Some(sk) = sort_key {
                    record.insert("sortKey".into(), json!(sk));
                }
                record.insert("text".into(), json!(text));
                if let Some(cj) = content_json {
                    record.insert("contentJson".into(), cj);
                }
                record.insert(
                    "tags".into(),
                    Value::Array(tags.into_iter().map(Value::from).collect()),
                );
                record.insert("kind".into(), json!(kind));
                Value::Object(record)
            },
        )
}

proptest! {
    #[test]
    fn storage_decoders_never_panic(value in arb_value()) {
        let _ = decode_local_bubbles_storage(&value);
        let _ = decode_local_bubbles_sources(&value);
        let _ = normalize_local_bubbles(&value);
    }

    /// normalize → encode → decode is a fixed point: the codec's own
    /// output is already canonical, so a second pass changes nothing.
    #[test]
    fn normalize_encode_decode_is_idempotent(
        bubbles in prop::collection::vec(arb_bubble_record(), 0..8),
        journal_imported in any::<bool>(),
    ) {
        let first = normalize_local_bubbles(&Value::Array(bubbles));
        let encoded = encode_local_bubbles_storage(&first, journal_imported);
        let blob: Value = serde_json::from_str(&encoded).unwrap();
        let second = decode_local_bubbles_storage(&blob);
        prop_assert_eq!(first, second);
    }

    #[test]
    fn render_blocks_never_panics(doc in arb_value()) {
        let _ = render_blocks(Some(&doc), "fallback");
        let _ = render_blocks(None, "fallback");
    }
}
