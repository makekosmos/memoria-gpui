//! Port of `src/lib/charCount.ts` — character count for the zen-mode counter.
//!
//! Counts text-node characters in Unicode code points (`chars()`, matching
//! `Array.from` semantics — an emoji is 1, not 2 UTF-16 units), plus one
//! character per leaf-block boundary and one per hard break.

use serde_json::Value;

const LEAF_BLOCK_TYPES: &[&str] = &[
    "paragraph",
    "heading",
    "codeBlock",
    "code_block",
    "horizontalRule",
    "horizontal_rule",
];
const HARD_BREAK_TYPES: &[&str] = &["hardBreak", "hard_break"];

/// `countCharsInProseMirrorDoc` — null on absent/malformed JSON.
pub fn count_chars_in_prose_mirror_doc(json: Option<&str>) -> Option<usize> {
    let json = json.filter(|s| !s.is_empty())?;
    let doc = serde_json::from_str::<Value>(json).ok()?;
    Some(count_chars_in_prose_mirror_node(&doc))
}

/// `countCharsInProseMirrorNode` — walks the raw JSON tree.
pub fn count_chars_in_prose_mirror_node(node: &Value) -> usize {
    let mut total = 0usize;
    let mut leaf_blocks_seen = 0usize;
    walk(node, &mut total, &mut leaf_blocks_seen);
    total
}

fn walk(node: &Value, total: &mut usize, leaf_blocks_seen: &mut usize) {
    if !node.is_object() {
        return;
    }
    let node_type = node.get("type").and_then(Value::as_str);

    if node_type == Some("text") {
        if let Some(text) = node.get("text").and_then(Value::as_str) {
            *total += text.chars().count();
        }
        return;
    }
    if let Some(t) = node_type {
        if HARD_BREAK_TYPES.contains(&t) {
            *total += 1;
            return;
        }
        if LEAF_BLOCK_TYPES.contains(&t) {
            if *leaf_blocks_seen > 0 {
                *total += 1;
            }
            *leaf_blocks_seen += 1;
        }
    }
    if let Some(Value::Array(children)) = node.get("content") {
        for child in children {
            walk(child, total, leaf_blocks_seen);
        }
    }
}
