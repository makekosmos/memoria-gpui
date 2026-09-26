//! `markdownToTiptapDoc` + `parseMarkdownBlocks` — block-level markdown → PM
//! nodes. List handling lives in `parse_list.rs`.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};

use super::inline::parse_inline_markdown;
use super::parse_list::parse_list_block;

static FENCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(`{3,})([^`]*)\s*$").expect("fence re"));
static HEADING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(#{1,6})\s+(.+?)\s*$").expect("heading re"));
static HR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:---+|\*\*\*+|___+)\s*$").expect("hr re"));
static QUOTE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*>\s?(.*)$").expect("quote re"));

/// Build a PM node preserving Vue key order (`type`, `attrs?`, `content?`,
/// `text?`, `marks?` — absent keys stay absent).
pub(crate) fn node(
    node_type: &str,
    attrs: Option<Map<String, Value>>,
    content: Option<Vec<Value>>,
) -> Value {
    let mut map = Map::new();
    map.insert("type".into(), Value::from(node_type));
    if let Some(attrs) = attrs {
        map.insert("attrs".into(), Value::Object(attrs));
    }
    if let Some(content) = content {
        map.insert("content".into(), Value::Array(content));
    }
    Value::Object(map)
}

pub(crate) fn text_node(text: &str, marks: Option<Vec<Value>>) -> Value {
    let mut map = Map::new();
    map.insert("type".into(), Value::from("text"));
    map.insert("text".into(), Value::from(text));
    if let Some(marks) = marks.filter(|m| !m.is_empty()) {
        map.insert("marks".into(), Value::Array(marks));
    }
    Value::Object(map)
}

pub(crate) fn attrs_single(key: &str, value: Value) -> Option<Map<String, Value>> {
    let mut map = Map::new();
    map.insert(key.into(), value);
    Some(map)
}

/// `markdownToTiptapDoc` — CR/LF normalized; empty markdown → one empty
/// paragraph.
pub fn markdown_to_tiptap_doc(md: &str) -> Value {
    let normalized = md.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let content = parse_markdown_blocks(&lines);
    let content = if content.is_empty() {
        vec![node("paragraph", None, None)]
    } else {
        content
    };
    let mut map = Map::new();
    map.insert("type".into(), Value::from("doc"));
    map.insert("content".into(), Value::Array(content));
    Value::Object(map)
}

fn parse_markdown_blocks(lines: &[&str]) -> Vec<Value> {
    let mut content: Vec<Value> = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();
    let mut index = 0;

    macro_rules! flush_paragraph {
        () => {
            if !paragraph.is_empty() {
                let mut inline: Vec<Value> = Vec::new();
                let last = paragraph.len() - 1;
                for (i, line) in paragraph.drain(..).enumerate() {
                    inline.extend(parse_inline_markdown(&line));
                    if i != last {
                        inline.push(node("hardBreak", None, None));
                    }
                }
                if !inline.is_empty() {
                    push_paragraph_or_image_blocks(&mut content, inline);
                }
            }
        };
    }

    while index < lines.len() {
        let line = lines[index];

        if line.trim().is_empty() {
            flush_paragraph!();
            index += 1;
            continue;
        }

        if let Some(fence) = FENCE_RE.captures(line) {
            flush_paragraph!();
            let fence_marker = fence.get(1).map(|m| m.as_str()).unwrap_or("```");
            let language = fence.get(2).map(|m| m.as_str().trim()).unwrap_or("");
            let language = (!language.is_empty()).then(|| language.to_string());
            let mut code_lines: Vec<&str> = Vec::new();
            index += 1;
            while index < lines.len() && lines[index].trim() != fence_marker {
                code_lines.push(lines[index]);
                index += 1;
            }
            if index < lines.len() && lines[index].trim() == fence_marker {
                index += 1;
            }
            let attrs = language.and_then(|l| attrs_single("language", Value::from(l)));
            let inner =
                (!code_lines.is_empty()).then(|| vec![text_node(&code_lines.join("\n"), None)]);
            content.push(node("codeBlock", attrs, inner));
            continue;
        }

        if let Some(heading) = HEADING_RE.captures(line) {
            flush_paragraph!();
            let level = heading.get(1).map(|m| m.as_str().len()).unwrap_or(1);
            let body = heading.get(2).map(|m| m.as_str()).unwrap_or("");
            content.push(node(
                "heading",
                attrs_single("level", Value::from(level)),
                Some(parse_inline_markdown(body)),
            ));
            index += 1;
            continue;
        }

        if HR_RE.is_match(line) {
            flush_paragraph!();
            content.push(node("horizontalRule", None, None));
            index += 1;
            continue;
        }

        if line.trim_start().starts_with('>') {
            flush_paragraph!();
            let mut quote_lines: Vec<String> = Vec::new();
            while index < lines.len() {
                let quote_line = lines[index];
                if quote_line.trim().is_empty() {
                    quote_lines.push(String::new());
                    index += 1;
                    continue;
                }
                match QUOTE_RE.captures(quote_line) {
                    Some(m) => {
                        quote_lines.push(m.get(1).map(|c| c.as_str()).unwrap_or("").to_string());
                        index += 1;
                    }
                    None => break,
                }
            }
            let refs: Vec<&str> = quote_lines.iter().map(String::as_str).collect();
            content.push(node("blockquote", None, Some(parse_markdown_blocks(&refs))));
            continue;
        }

        if let Some(list_block) = parse_list_block(lines, index) {
            flush_paragraph!();
            content.push(list_block.0);
            index = list_block.1;
            continue;
        }

        paragraph.push(line.to_string());
        index += 1;
    }

    flush_paragraph!();
    content
}

/// `pushParagraphOrImageBlocks` — bare image nodes split paragraphs.
fn push_paragraph_or_image_blocks(content: &mut Vec<Value>, inline: Vec<Value>) {
    let mut paragraph: Vec<Value> = Vec::new();

    macro_rules! flush {
        () => {
            if !paragraph.is_empty() {
                content.push(node(
                    "paragraph",
                    None,
                    Some(std::mem::take(&mut paragraph)),
                ));
            }
        };
    }

    for item in inline {
        // Vue `!node.marks?.length`: absent/null, empty array and zero-length
        // string marks are bare; strings/arrays with length are not; other
        // types have `.length === undefined` (truthy `!`) and stay bare.
        let marks_len = match item.get("marks") {
            Some(Value::Array(a)) => Some(a.len()),
            Some(Value::String(s)) => Some(s.len()),
            _ => None,
        };
        let bare_image = item.get("type").and_then(Value::as_str) == Some("image")
            && marks_len.map(|l| l == 0).unwrap_or(true);
        if bare_image {
            flush!();
            content.push(item);
        } else {
            paragraph.push(item);
        }
    }
    flush!();
}
