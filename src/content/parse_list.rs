//! `parseListBlock` + `parseListItem` — list-level markdown → PM nodes.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::inline::parse_inline_markdown;
use super::parse::{attrs_single, node};

static TASK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*)[-*]\s+\[([ xX])\]\s*(.*)$").expect("task re"));
static BULLET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*)[-*]\s+(.+)$").expect("bullet re"));
static ORDERED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*)(\d+)[.)]\s+(.+)$").expect("ordered re"));

struct ParsedListItem<'a> {
    kind: &'static str,
    text: &'a str,
    checked: bool,
    start: i64,
    indent: usize,
}

fn parse_list_item(line: &str) -> Option<ParsedListItem<'_>> {
    if let Some(m) = TASK_RE.captures(line) {
        return Some(ParsedListItem {
            kind: "taskList",
            text: m.get(3).map(|c| c.as_str()).unwrap_or(""),
            checked: m
                .get(2)
                .map(|c| c.as_str().eq_ignore_ascii_case("x"))
                .unwrap_or(false),
            start: 1,
            indent: m.get(1).map(|c| c.as_str().chars().count()).unwrap_or(0),
        });
    }
    if let Some(m) = BULLET_RE.captures(line) {
        return Some(ParsedListItem {
            kind: "bulletList",
            text: m.get(2).map(|c| c.as_str()).unwrap_or(""),
            checked: false,
            start: 1,
            indent: m.get(1).map(|c| c.as_str().chars().count()).unwrap_or(0),
        });
    }
    if let Some(m) = ORDERED_RE.captures(line) {
        return Some(ParsedListItem {
            kind: "orderedList",
            text: m.get(3).map(|c| c.as_str()).unwrap_or(""),
            checked: false,
            start: m.get(2).and_then(|c| c.as_str().parse().ok()).unwrap_or(1),
            indent: m.get(1).map(|c| c.as_str().chars().count()).unwrap_or(0),
        });
    }
    None
}

/// JS `/^\s*/` captures unicode whitespace; count chars (≈ UTF-16 units
/// for BMP whitespace) not bytes.
fn leading_ws(line: &str) -> usize {
    line.chars().take_while(|c| c.is_whitespace()).count()
}

/// `parseListBlock` — same-kind same-indent runs; deeper indents recurse,
/// indented continuations fold into the item text.
pub(crate) fn parse_list_block(lines: &[&str], start_index: usize) -> Option<(Value, usize)> {
    let first = parse_list_item(lines[start_index])?;
    let mut items: Vec<(String, bool, Vec<Value>)> = Vec::new();
    let mut index = start_index;

    while index < lines.len() {
        let Some(current) = parse_list_item(lines[index]) else {
            break;
        };
        if current.kind != first.kind || current.indent != first.indent {
            break;
        }

        let mut parts = vec![current.text.trim().to_string()];
        let mut children: Vec<Value> = Vec::new();
        index += 1;
        while index < lines.len() {
            let continuation = lines[index];
            if continuation.trim().is_empty() {
                break;
            }
            if let Some(nested_item) = parse_list_item(continuation) {
                if nested_item.indent > current.indent {
                    if let Some((nested, next)) = parse_list_block(lines, index) {
                        children.push(nested);
                        index = next;
                        continue;
                    }
                }
                break;
            }
            if leading_ws(continuation) <= current.indent {
                break;
            }
            parts.push(continuation.trim().to_string());
            index += 1;
        }
        items.push((parts.join(" "), current.checked, children));
    }

    let attrs = if first.kind == "orderedList" && first.start != 1 {
        attrs_single("start", Value::from(first.start))
    } else {
        None
    };
    let content: Vec<Value> = items
        .into_iter()
        .map(|(text, checked, mut item_children)| {
            let item_type = if first.kind == "taskList" {
                "taskItem"
            } else {
                "listItem"
            };
            let item_attrs = if first.kind == "taskList" {
                attrs_single("checked", Value::from(checked))
            } else {
                None
            };
            let mut item_content =
                vec![node("paragraph", None, Some(parse_inline_markdown(&text)))];
            item_content.append(&mut item_children);
            node(item_type, item_attrs, Some(item_content))
        })
        .collect();

    Some((node(first.kind, attrs, Some(content)), index))
}
