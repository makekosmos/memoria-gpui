//! `BubbleTiptapRenderer` GPUI side — walks `RenderBlock`s (from
//! `diary::render_model`) into elements. Unknown nodes already degraded to
//! text in the model; they render as plain paragraphs.
use std::ops::Range;

use gpui::{div, prelude::*, px, AnyElement, HighlightStyle, StyledText};
use memoria_model::diary::{render_blocks, BubbleTimelineNode, InlineRun, RenderBlock};

use super::Memoria;
use crate::theme::*;

/// `InlineRun` list → `StyledText` (wrapping) + highlight ranges.
fn runs_to_text(runs: &[InlineRun]) -> StyledText {
    let mut text = String::new();
    let mut marks: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
    for run in runs {
        let start = text.len();
        text.push_str(&run.text);
        let end = text.len();
        if end == start {
            continue;
        }
        let mut style = HighlightStyle::default();
        if run.bold {
            style.font_weight = Some(gpui::FontWeight::BOLD);
        }
        if run.italic {
            style.font_style = Some(gpui::FontStyle::Italic);
        }
        if run.strike {
            style.strikethrough = Some(gpui::StrikethroughStyle::default());
        }
        if run.code {
            // `HighlightStyle` has no font family — background tint only.
            style.background_color = Some(rgba(FG(), 0.07));
        }
        if run.link.is_some() {
            style.color = Some(c(ACCENT()));
            style.underline = Some(gpui::UnderlineStyle::default());
        }
        if style != HighlightStyle::default() {
            marks.push((start..end, style));
        }
    }
    StyledText::new(text).with_highlights(marks)
}

fn paragraph(runs: &[InlineRun]) -> gpui::Div {
    div().mb(px(7.2)).min_w_0().child(runs_to_text(runs))
}

fn list_row(marker: String, blocks: Vec<RenderBlock>, depth: u32) -> gpui::Div {
    div()
        .flex()
        .min_w_0()
        .mb(px(2.))
        .child(
            div()
                .w(px(18.))
                .flex_none()
                .text_color(rgba(FG(), 0.55))
                .child(marker),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .children(render_list(blocks, depth + 1)),
        )
}

/// Nested block list → elements (used by list items / blockquotes).
fn render_list(blocks: Vec<RenderBlock>, depth: u32) -> Vec<AnyElement> {
    let mut out: Vec<AnyElement> = Vec::new();
    for (ix, block) in blocks.into_iter().enumerate() {
        let _ = ix;
        out.extend(render_block(block, depth));
    }
    out
}

fn render_block(block: RenderBlock, depth: u32) -> Vec<AnyElement> {
    match block {
        RenderBlock::Paragraph(runs) => vec![paragraph(&runs).into_any_element()],
        RenderBlock::Heading { level, runs } => {
            let size = match level {
                1 => 22.,
                2 => 18.,
                3 => 16.,
                _ => 15.,
            };
            vec![div()
                .mb(px(7.2))
                .text_size(px(size))
                .font_weight(gpui::FontWeight::BOLD)
                .child(runs_to_text(&runs))
                .into_any_element()]
        }
        RenderBlock::Blockquote(children) => vec![div()
            .mb(px(7.2))
            .pl(px(12.))
            .border_l_2()
            .border_color(c(BORDER()))
            .text_color(rgba(FG(), 0.72))
            .flex()
            .flex_col()
            .children(render_list(children, depth))
            .into_any_element()],
        RenderBlock::CodeBlock { text, .. } => vec![div()
            .mb(px(7.2))
            .rounded(px(8.))
            .bg(rgba(FG(), 0.06))
            .px(px(12.8))
            .py(px(11.2))
            .font_family("Berkeley Mono")
            .text_size(px(13.))
            .child(text)
            .into_any_element()],
        RenderBlock::BulletList(items) => items
            .into_iter()
            .map(|blocks| list_row("•".into(), blocks, depth).into_any_element())
            .collect(),
        RenderBlock::OrderedList { start, items } => items
            .into_iter()
            .enumerate()
            .map(|(i, blocks)| {
                list_row(format!("{}.", start + i as i64), blocks, depth).into_any_element()
            })
            .collect(),
        RenderBlock::TaskList(items) => items
            .into_iter()
            .map(|(checked, blocks)| {
                list_row(if checked { "☑" } else { "☐" }.into(), blocks, depth).into_any_element()
            })
            .collect(),
        RenderBlock::HorizontalRule => vec![div()
            .h(px(1.))
            .my(px(8.))
            .bg(c(BORDER()))
            .into_any_element()],
        RenderBlock::Image { src, alt } => vec![div()
            .mb(px(7.2))
            .text_color(rgba(FG(), 0.55))
            .child(format!(
                "[{}]",
                alt.filter(|a| !a.is_empty())
                    .or(src)
                    .unwrap_or_else(|| "изображение".into())
            ))
            .into_any_element()],
        RenderBlock::Unknown { text, .. } => vec![if text.is_empty() {
            div().h(px(0.)).into_any_element()
        } else {
            div()
                .mb(px(7.2))
                .text_color(rgba(FG(), 0.72))
                .child(text)
                .into_any_element()
        }],
    }
}

impl Memoria {
    /// `BubbleTiptapRenderer` — `contentJson ?? plainTextToTiptapDoc(text)`.
    pub(crate) fn render_bubble_blocks(&self, node: &BubbleTimelineNode) -> Vec<AnyElement> {
        let blocks = render_blocks(node.content_json.as_ref(), &node.text);
        let mut out = Vec::with_capacity(blocks.len());
        for block in blocks {
            out.extend(render_block(block, 0));
        }
        if out.is_empty() {
            out.push(
                div()
                    .text_color(rgba(FG(), 0.55))
                    .child("…")
                    .into_any_element(),
            );
        }
        out
    }
}
