//! `BubbleTiptapRenderer` render model — walks a tiptap `doc` tree into flat
//! display blocks the GPUI view draws. Unknown node types degrade to their
//! extracted plain text (the text is still shown); marks map to inline flags.
//! This model is display-only — the original `contentJson` is what persists,
//! so unknown nodes survive a save untouched.

use serde_json::Value;

use super::text::{bubble_plain_text, plain_text_to_tiptap_doc};

/// One styled text run inside a block.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InlineRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    /// `link` mark `href`.
    pub link: Option<String>,
}

/// A top-level (or nested) display block.
#[derive(Clone, Debug, PartialEq)]
pub enum RenderBlock {
    Paragraph(Vec<InlineRun>),
    Heading {
        level: u8,
        runs: Vec<InlineRun>,
    },
    /// Nested blocks inside the quote.
    Blockquote(Vec<RenderBlock>),
    CodeBlock {
        language: Option<String>,
        text: String,
    },
    /// `bulletList` — each item is its own block list (paragraph + nesting).
    BulletList(Vec<Vec<RenderBlock>>),
    OrderedList {
        start: i64,
        items: Vec<Vec<RenderBlock>>,
    },
    /// `taskList` — `(checked, blocks)` per `taskItem`.
    TaskList(Vec<(bool, Vec<RenderBlock>)>),
    HorizontalRule,
    /// `image` — rendered as an alt/src placeholder line.
    Image {
        src: Option<String>,
        alt: Option<String>,
    },
    /// Unknown node: shows its extracted text so nothing renders blank.
    Unknown {
        type_name: String,
        text: String,
    },
}

/// `contentJson ?? plainTextToTiptapDoc(fallbackText)` → display blocks.
pub fn render_blocks(content_json: Option<&Value>, fallback_text: &str) -> Vec<RenderBlock> {
    let owned;
    let doc = match content_json {
        Some(value) if value.get("type").and_then(Value::as_str) == Some("doc") => value,
        _ => {
            owned = plain_text_to_tiptap_doc(fallback_text);
            &owned
        }
    };
    doc.get("content")
        .and_then(Value::as_array)
        .map(|blocks| blocks.iter().map(render_node).collect())
        .unwrap_or_default()
}

fn render_node(node: &Value) -> RenderBlock {
    let type_name = node.get("type").and_then(Value::as_str).unwrap_or("");
    let content = node
        .get("content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    match type_name {
        "paragraph" => RenderBlock::Paragraph(inline_runs(&content)),
        "heading" => {
            let level = node
                .get("attrs")
                .and_then(|a| a.get("level"))
                .and_then(Value::as_i64)
                .unwrap_or(1)
                .clamp(1, 6) as u8;
            RenderBlock::Heading {
                level,
                runs: inline_runs(&content),
            }
        }
        "blockquote" => RenderBlock::Blockquote(content.iter().map(render_node).collect()),
        "codeBlock" => RenderBlock::CodeBlock {
            language: node
                .get("attrs")
                .and_then(|a| a.get("language"))
                .and_then(Value::as_str)
                .map(str::to_string),
            text: content
                .iter()
                .filter(|n| n.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|n| n.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(""),
        },
        "bulletList" => RenderBlock::BulletList(list_items(&content)),
        "orderedList" => RenderBlock::OrderedList {
            start: node
                .get("attrs")
                .and_then(|a| a.get("start"))
                .and_then(Value::as_i64)
                .unwrap_or(1),
            items: list_items(&content),
        },
        "taskList" => RenderBlock::TaskList(
            content
                .iter()
                .map(|item| {
                    let checked = item
                        .get("attrs")
                        .and_then(|a| a.get("checked"))
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let blocks = item
                        .get("content")
                        .and_then(Value::as_array)
                        .map(|c| c.iter().map(render_node).collect())
                        .unwrap_or_default();
                    (checked, blocks)
                })
                .collect(),
        ),
        "horizontalRule" => RenderBlock::HorizontalRule,
        "image" => RenderBlock::Image {
            src: node
                .get("attrs")
                .and_then(|a| a.get("src"))
                .and_then(Value::as_str)
                .map(str::to_string),
            alt: node
                .get("attrs")
                .and_then(|a| a.get("alt"))
                .and_then(Value::as_str)
                .map(str::to_string),
        },
        _ => RenderBlock::Unknown {
            type_name: type_name.to_string(),
            text: bubble_plain_text(node),
        },
    }
}

fn list_items(content: &[Value]) -> Vec<Vec<RenderBlock>> {
    content
        .iter()
        .map(|item| {
            item.get("content")
                .and_then(Value::as_array)
                .map(|c| c.iter().map(render_node).collect())
                .unwrap_or_default()
        })
        .collect()
}

/// Inline children → runs; `hardBreak` becomes `\n` inside the current run.
fn inline_runs(content: &[Value]) -> Vec<InlineRun> {
    let mut runs: Vec<InlineRun> = Vec::new();
    for node in content {
        match node.get("type").and_then(Value::as_str) {
            Some("text") => {
                let mut run = InlineRun {
                    text: node
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    ..Default::default()
                };
                if let Some(marks) = node.get("marks").and_then(Value::as_array) {
                    for mark in marks {
                        match mark.get("type").and_then(Value::as_str) {
                            Some("bold") => run.bold = true,
                            Some("italic") => run.italic = true,
                            Some("strike") => run.strike = true,
                            Some("code") => run.code = true,
                            Some("link") => {
                                run.link = mark
                                    .get("attrs")
                                    .and_then(|a| a.get("href"))
                                    .and_then(Value::as_str)
                                    .map(str::to_string);
                            }
                            _ => {}
                        }
                    }
                }
                push_run(&mut runs, run);
            }
            Some("hardBreak") => {
                // Merge into the previous run's text when styles are equal.
                let run = InlineRun {
                    text: "\n".into(),
                    ..Default::default()
                };
                push_run(&mut runs, run);
            }
            _ => {
                // Unknown inline node — show its text rather than dropping it.
                let text = bubble_plain_text(node);
                if !text.is_empty() {
                    push_run(
                        &mut runs,
                        InlineRun {
                            text,
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }
    runs
}

fn push_run(runs: &mut Vec<InlineRun>, run: InlineRun) {
    if let Some(last) = runs.last_mut() {
        if last.bold == run.bold
            && last.italic == run.italic
            && last.strike == run.strike
            && last.code == run.code
            && last.link == run.link
        {
            last.text.push_str(&run.text);
            return;
        }
    }
    runs.push(run);
}
