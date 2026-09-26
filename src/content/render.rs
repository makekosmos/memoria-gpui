//! `tiptapDocToMarkdown` — PM doc → markdown rendering, 1:1 with Vue.

use serde_json::Value;

use super::{
    attrs_of, content_of, normalize_marks, prefix_multiline, render_code_fence, render_marked_text,
};

/// `tiptapDocToMarkdown`.
pub fn tiptap_doc_to_markdown(doc: &Value) -> String {
    render_tiptap_nodes(content_of(doc)).trim().to_string()
}

fn is_list_node(node: &Value) -> bool {
    matches!(
        node.get("type").and_then(Value::as_str),
        Some("bulletList") | Some("orderedList") | Some("taskList")
    )
}

/// `renderTiptapNodes` — consecutive list nodes join with one newline.
fn render_tiptap_nodes(nodes: &[Value]) -> String {
    let rendered: Vec<(&Value, String)> = nodes
        .iter()
        .map(|node| (node, render_tiptap_node(node)))
        .filter(|(_, text)| !text.is_empty())
        .collect();
    let mut out = String::new();
    for (index, (node, text)) in rendered.iter().enumerate() {
        if index == 0 {
            out.push_str(text);
            continue;
        }
        let prev_is_list = is_list_node(rendered[index - 1].0);
        let separator = if prev_is_list && is_list_node(node) {
            "\n"
        } else {
            "\n\n"
        };
        out.push_str(separator);
        out.push_str(text);
    }
    out
}

fn render_inline(nodes: &[Value]) -> String {
    nodes.iter().map(render_inline_node).collect()
}

fn render_tiptap_node(node: &Value) -> String {
    match node.get("type").and_then(Value::as_str) {
        Some("paragraph") => render_inline(content_of(node)),
        Some("heading") => {
            let level = attrs_of(node)
                .and_then(|a| a.get("level"))
                .and_then(Value::as_f64)
                .map(|l| (l as i64).clamp(1, 6) as usize)
                .unwrap_or(1);
            format!("{} {}", "#".repeat(level), render_inline(content_of(node)))
                .trim()
                .to_string()
        }
        Some("blockquote") => render_tiptap_nodes(content_of(node))
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
        Some("bulletList") => render_list(node, "-"),
        Some("orderedList") => render_list(node, "ordered"),
        Some("taskList") => render_list(node, "task"),
        Some("codeBlock") => render_code_fence(&render_inline(content_of(node)), node.get("attrs")),
        Some("horizontalRule") => "---".into(),
        Some("image") => render_image_node(node),
        Some("hardBreak") => "\\\n".into(),
        _ => render_inline(content_of(node)),
    }
}

fn render_list(node: &Value, marker: &str) -> String {
    let ordered_start = if marker == "ordered" {
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
            let prefix = match marker {
                "ordered" => format!("{}. ", ordered_start + index as i64),
                "task" => {
                    let checked = attrs_of(item)
                        .and_then(|a| a.get("checked"))
                        .map(super::truthy)
                        .unwrap_or(false);
                    if checked {
                        "- [x] ".to_string()
                    } else {
                        "- [ ] ".to_string()
                    }
                }
                _ => format!("{marker} "),
            };
            prefix_multiline(&render_inline_text_block(item), &prefix)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `renderInlineTextBlock` — children rendered as blocks; a list child joins
/// with "\n", others "\n\n"; all-empty falls back to inline text.
fn render_inline_text_block(node: &Value) -> String {
    let rendered: Vec<(&Value, String)> = content_of(node)
        .iter()
        .map(|child| (child, render_tiptap_node(child)))
        .filter(|(_, text)| !text.is_empty())
        .collect();
    if rendered.is_empty() {
        return render_inline(content_of(node));
    }
    let mut out = String::new();
    for (index, (child, text)) in rendered.iter().enumerate() {
        if index > 0 {
            out.push_str(if is_list_node(child) { "\n" } else { "\n\n" });
        }
        out.push_str(text);
    }
    out
}

fn render_inline_node(node: &Value) -> String {
    let node_type = node.get("type").and_then(Value::as_str);
    if node_type == Some("text") {
        if let Some(text) = node.get("text").and_then(Value::as_str) {
            return render_marked_text(text, &normalize_marks(node.get("marks")));
        }
    }
    match node_type {
        Some("image") => render_image_node(node),
        Some("hardBreak") => "\\\n".into(),
        _ => render_inline(content_of(node)),
    }
}

/// `renderImageNode` — `![alt](src "title")`, empty when `src` missing.
fn render_image_node(node: &Value) -> String {
    let attrs = attrs_of(node);
    let src = attrs
        .and_then(|a| a.get("src"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let alt = attrs
        .and_then(|a| a.get("alt"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let title = attrs
        .and_then(|a| a.get("title"))
        .and_then(Value::as_str)
        .map(|t| format!(" \"{t}\""))
        .unwrap_or_default();
    if src.is_empty() {
        String::new()
    } else {
        format!("![{alt}]({src}{title})")
    }
}
