//! Port of `tests/bubbleDiaryModel.test.ts` (Vue SoT @7ccbb9f) —
//! journal import, draft parsing, occurrence labels, thread normalization.

use memoria_model::content::write_entry_tiptap_doc;
use memoria_model::diary::{
    create_draft_bubble, create_journal_bubbles_from_entry, decode_local_bubbles_storage,
    is_legacy_dated_journal_entry, normalize_bubble_threads, parse_bubble_draft, BubbleKind,
    BubbleTimelineNode, ReplyLink,
};
use memoria_model::local_time::local_ms;
use memoria_model::model::Entry;
use serde_json::{json, Value};

fn entry(content: Value) -> Entry {
    Entry {
        id: "entry-1".into(),
        title: "2021-01-21".into(),
        content_json: serde_json::to_string(&write_entry_tiptap_doc(content)).unwrap_or_default(),
        content_loaded: Some(true),
        created_at: 1_611_219_600_000, // Date.UTC(2021, 0, 21, 9, 0)
        updated_at: 1_611_219_600_000,
        type_id: Some("note_obj".into()),
        header_layout: Some("default".into()),
        header_props_json: Some("{}".into()),
        schema_version: Some(1),
        deleted_at: None,
        ..Default::default()
    }
}

fn para(text: &str) -> Value {
    json!({ "type": "paragraph", "content": [{ "type": "text", "text": text }] })
}

#[test]
fn imports_dated_legacy_note_obj_entries_as_diary_bubbles() {
    // Regression: 2026-07-03 — старые дневниковые заметки могли быть note_obj.
    let legacy = entry(json!({
        "type": "doc",
        "content": [para("старый дневниковый текст")],
    }));
    assert!(is_legacy_dated_journal_entry(&legacy));
    let bubbles = create_journal_bubbles_from_entry(&legacy);
    assert_eq!(bubbles.len(), 1);
    assert_eq!(bubbles[0].id, "journal-entry-1-0");
    assert_eq!(bubbles[0].date.as_deref(), Some("2021-01-21"));
    assert_eq!(bubbles[0].time, "2021-01-21");
    assert_eq!(bubbles[0].text, "старый дневниковый текст");
}

#[test]
fn sorts_later_legacy_blocks_above_earlier_and_drops_empty() {
    let legacy = entry(json!({
        "type": "doc",
        "content": [
            para("первая"),
            { "type": "paragraph" },
            para("вторая"),
        ],
    }));
    let bubbles = create_journal_bubbles_from_entry(&legacy);
    assert_eq!(
        bubbles.iter().map(|b| b.text.as_str()).collect::<Vec<_>>(),
        ["первая", "вторая"]
    );
    let midnight = memoria_model::time::iso_to_millis("2021-01-21T00:00:00.000Z").unwrap() as f64;
    assert_eq!(
        bubbles.iter().map(|b| b.sort_key).collect::<Vec<_>>(),
        [Some(midnight), Some(midnight + 2.0)]
    );
}

#[test]
fn repairs_sort_order_for_already_imported_legacy_bubbles() {
    let decoded = decode_local_bubbles_storage(&json!({
        "version": 1,
        "bubbles": [{
            "id": "journal-entry-1-2",
            "date": "2021-01-21",
            "time": "2021-01-21",
            "sortKey": 1_611_206_400_000f64 - 2.0,
            "text": "вторая",
            "tags": [],
            "kind": "plain",
        }],
    }));
    let midnight = memoria_model::time::iso_to_millis("2021-01-21T00:00:00.000Z").unwrap() as f64;
    assert_eq!(decoded[0].sort_key, Some(midnight + 2.0));
}

#[test]
fn draft_trims_and_collapses_lines() {
    let (text, _) = parse_bubble_draft("  первая   строка  \n\n\n  вторая   строка   ");
    assert_eq!(text, "первая строка\nвторая строка");
}

#[test]
fn drops_empty_tiptap_blocks_from_rendered_draft_content() {
    // `new Date(Date.UTC(2026, 6, 1, 12, 0))` — offset arg is irrelevant to
    // the asserted fields (text/content only).
    let bubble = create_draft_bubble(
        &json!({
            "type": "doc",
            "content": [para("первая"), { "type": "paragraph" }],
        }),
        Some("первая\n\n"),
        1_782_982_800_000,
        0,
    )
    .expect("draft");
    assert_eq!(bubble.text, "первая");
    let content = bubble.content_json.as_ref().unwrap()["content"]
        .as_array()
        .unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["content"][0]["text"], "первая");
}

#[test]
fn tag_extraction_dedupes_case_insensitively() {
    let (text, tags) = parse_bubble_draft("Текст #Tag #tag #сделать");
    assert_eq!(text, "Текст");
    assert_eq!(tags, ["tag", "сделать"]);
}

#[test]
fn occurrence_labels_use_local_calendar_boundaries() {
    use memoria_model::diary::occurrence_label_from_ms;
    // `new Date(2026, 0, 1, 0, 15)` — host-local civil time.
    let now = local_ms(2026, 1, 1, 0, 15);
    assert_eq!(
        occurrence_label_from_ms(local_ms(2026, 1, 1, 0, 1), now),
        "00:01"
    );
    assert_eq!(
        occurrence_label_from_ms(local_ms(2025, 12, 31, 23, 59), now),
        "Вчера, 23:59"
    );
    assert_eq!(
        occurrence_label_from_ms(local_ms(2025, 7, 2, 9, 5), now),
        "2 июл 2025, 09:05"
    );

    let same_year_now = local_ms(2026, 7, 10, 12, 0);
    assert_eq!(
        occurrence_label_from_ms(local_ms(2026, 6, 1, 8, 7), same_year_now),
        "1 июн, 08:07"
    );
}

#[test]
fn label_flips_to_yesterday_after_local_midnight() {
    use memoria_model::diary::occurrence_label_from_ms;
    let occurrence = local_ms(2026, 7, 10, 23, 58);
    assert_eq!(
        occurrence_label_from_ms(occurrence, local_ms(2026, 7, 10, 23, 59)),
        "23:58"
    );
    assert_eq!(
        occurrence_label_from_ms(occurrence, local_ms(2026, 7, 11, 0, 1)),
        "Вчера, 23:58"
    );
}

fn node(id: &str, sort_key: i64) -> BubbleTimelineNode {
    BubbleTimelineNode {
        id: id.into(),
        time: "00:00".into(),
        sort_key: Some(sort_key as f64),
        text: id.into(),
        tags: Vec::new(),
        kind: BubbleKind::Plain,
        ..Default::default()
    }
}

fn link(id: &str, source: &str, target: &str) -> ReplyLink {
    ReplyLink {
        id: id.into(),
        source_object_id: source.into(),
        target_object_id: target.into(),
    }
}

#[test]
fn groups_roots_newest_first_and_replies_oldest_first() {
    let (bubbles, invalid) = normalize_bubble_threads(
        vec![
            node("root", 10),
            node("reply-2", 12),
            node("reply-1", 11),
            node("new-root", 20),
        ],
        &[link("l2", "reply-2", "root"), link("l1", "reply-1", "root")],
    );
    assert!(invalid.is_empty());
    let pairs: Vec<(&str, Option<&str>)> = bubbles
        .iter()
        .map(|b| (b.id.as_str(), b.parent_id.as_deref()))
        .collect();
    assert_eq!(
        pairs,
        [
            ("new-root", None),
            ("root", None),
            ("reply-1", Some("root")),
            ("reply-2", Some("root")),
        ]
    );
}

#[test]
fn malformed_deeper_and_cyclic_relationships_stay_visible_as_roots() {
    let (bubbles, invalid) = normalize_bubble_threads(
        vec![node("a", 1), node("b", 2), node("c", 3), node("orphan", 4)],
        &[
            link("ab", "a", "b"),
            link("ba", "b", "a"),
            link("cb", "c", "b"),
            link("missing", "orphan", "gone"),
        ],
    );
    assert_eq!(bubbles.len(), 4);
    assert!(bubbles.iter().all(|b| b.parent_id.is_none()));
    let invalid: std::collections::HashSet<&str> = invalid.iter().map(String::as_str).collect();
    assert_eq!(invalid, ["ab", "ba", "cb", "missing"].into_iter().collect());
}
