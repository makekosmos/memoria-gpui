//! `legacyProseMirrorToText` — best-effort text extraction for pre-TipTap
//! ProseMirror dumps (loose `type`/`attrs`/`marks`).

use serde_json::Value;

use super::{
    attrs_of, content_of, normalize_marks, prefix_multiline, render_code_fence, render_marked_text,
    truthy,
};

/// `legacyProseMirrorToText` — `""` on anything unreadable.
pub fn legacy_prose_mirror_to_text(value: &Value) -> String {
    render_legacy_node(value).trim().to_string()
}

fn render_legacy_node(node: &Value) -> String {
    if !node.is_object() {
        return String::new();
    }

    if let Some(text) = node.get("text").and_then(Value::as_str) {
        return render_marked_text(text, &normalize_marks(node.get("marks")));
    }

    let media = media_markdown(node);
    if !media.is_empty() {
        return media;
    }

    let children: Vec<String> = content_of(node)
        .iter()
        .map(render_legacy_node)
        .filter(|part| !part.is_empty())
        .collect();
    let node_type = node.get("type").and_then(Value::as_str).unwrap_or("");

    match node_type {
        "doc" => children.join("\n\n"),
        "paragraph" => children.concat(),
        "heading" => render_legacy_heading(node, &children),
        "blockquote" => children
            .join("\n\n")
            .split('\n')
            .map(|line| {
                if line.is_empty() {
                    ">".to_string()
                } else {
                    format!("> {line}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        "bulletList" => render_legacy_list(node, "bullet"),
        "orderedList" => render_legacy_list(node, "ordered"),
        "taskList" => render_legacy_list(node, "task"),
        "listItem" | "taskItem" => children.join("\n\n"),
        "codeBlock" => render_code_fence(&children.concat(), node.get("attrs")),
        "horizontalRule" => "---".into(),
        "hardBreak" | "hard_break" => "\n".into(),
        _ => {
            if children.is_empty() {
                String::new()
            } else {
                children.join("\n\n")
            }
        }
    }
}

fn render_legacy_heading(node: &Value, children: &[String]) -> String {
    let level = attrs_of(node)
        .and_then(|a| a.get("level"))
        .and_then(Value::as_f64)
        .map(|l| (l as i64).clamp(1, 6) as usize)
        .unwrap_or(1);
    format!("{} {}", "#".repeat(level), children.concat())
        .trim()
        .to_string()
}

/// `mediaMarkdown` — any node whose type contains a media marker and has a
/// usable `src|url|href|fileUrl` attr becomes `![](src)`.
fn media_markdown(node: &Value) -> String {
    let node_type = node
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_lowercase();
    let is_media = ["image", "media", "imageblock", "figure", "attachment"]
        .iter()
        .any(|marker| node_type.contains(marker));
    if !is_media {
        return String::new();
    }
    let Some(attrs) = attrs_of(node) else {
        return String::new();
    };
    for key in ["src", "url", "href", "fileUrl"] {
        if let Some(candidate) = attrs.get(key).and_then(Value::as_str) {
            if !candidate.trim().is_empty() {
                return format!("![]({candidate})");
            }
        }
    }
    String::new()
}

/// `renderLegacyList` — list body is rendered per item then prefixed.
fn render_legacy_list(node: &Value, kind: &str) -> String {
    let ordered_start = if kind == "ordered" {
        attrs_of(node)
            .and_then(|a| a.get("start"))
            .and_then(Value::as_f64)
            .map(|s| s as i64)
            .unwrap_or(1)
    } else {
        1
    };
    content_of(node)
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let prefix = match kind {
                "ordered" => format!("{}. ", ordered_start + index as i64),
                "task" => legacy_task_prefix(item),
                _ => "- ".to_string(),
            };
            prefix_multiline(&render_legacy_node(item), &prefix)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn legacy_task_prefix(item: &Value) -> String {
    let checked = item
        .get("attrs")
        .filter(|a| truthy(a))
        .and_then(Value::as_object)
        .and_then(|a| a.get("checked"))
        .map(|v| *v == Value::Bool(true))
        .unwrap_or(false);
    if checked {
        "- [x] ".into()
    } else {
        "- [ ] ".into()
    }
}
