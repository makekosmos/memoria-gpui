//! Projection → render rows. One `Row` = one visible line of `Projection::text`
//! (the projection already joins sibling blocks with `\n`). Each row carries
//! its spans clipped to the row range, its top-level block index (for code
//! panels / pickers), and a `RowKind` (text / rule / standalone image).

use memoria_editor_core::md::ast::{BlockKind, Doc, Node};
use memoria_editor_core::project::{BlockTag, Marks, Payload, Projection, Span};
use std::ops::Range;

#[derive(Clone, Debug)]
pub enum RowKind {
    Text,
    /// `---`/`***`/`___` — paint a hairline, not text.
    Rule,
    /// Paragraph consisting only of an image → paint the image widget.
    StandaloneImage {
        src: String,
    },
}

#[derive(Clone, Debug)]
pub struct Row {
    /// Byte range of this line inside `Projection::text` (excludes `\n`).
    pub vis: Range<usize>,
    /// Index into `Doc::children` of the top-level block owning this row.
    pub block_ix: usize,
    /// Block styling tag (from the row's first real span).
    pub tag: BlockTag,
    /// Spans clipped to the row's vis range (row-local text runs come later).
    pub spans: Vec<Span>,
    pub kind: RowKind,
    /// First row of its top-level block (drives code-panel top edge etc.).
    pub first_in_block: bool,
    /// Last row of its top-level block.
    pub last_in_block: bool,
    /// Byte offset of this row's code text inside the block's concatenated
    /// code string (`code_block_text`); meaningful only on code rows.
    pub code_origin: usize,
}

/// Split the projection into rows. `doc` supplies top-level block ranges.
pub fn build_rows(proj: &Projection, doc: &Doc) -> Vec<Row> {
    let text = &proj.text;
    let mut rows = Vec::new();
    let mut start = 0usize;
    let mut push = |end: usize, rows: &mut Vec<Row>| {
        // `end` is the index *after* the `\n` — the row text is `start..end-1`.
        let vis_end = if end > start && text.as_bytes().get(end - 1) == Some(&b'\n') {
            end - 1
        } else {
            end
        };
        rows.push(make_row(start..vis_end, proj, doc));
        start = end;
    };
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            push(i + 1, &mut rows);
        }
    }
    push(text.len(), &mut rows);
    // Drop a fully empty trailing row produced by a final newline.
    if rows.len() > 1 && rows.last().is_some_and(|r| r.vis.is_empty()) {
        rows.pop();
    }
    mark_block_edges(&mut rows);
    rows
}

fn make_row(vis: Range<usize>, proj: &Projection, doc: &Doc) -> Row {
    let spans: Vec<Span> = proj
        .spans
        .iter()
        .filter(|s| s.vis.end > vis.start && s.vis.start < vis.end.max(vis.start + 1))
        .map(|s| {
            let mut s = s.clone();
            s.vis = s.vis.start.max(vis.start)..s.vis.end.min(vis.end);
            s
        })
        .collect();
    let src = spans
        .iter()
        .map(|s| s.src.start)
        .min()
        .unwrap_or_else(|| proj.to_source(vis.start.min(proj.text.len())));
    let block_ix = doc
        .children
        .partition_point(|n| n.range().end <= src)
        .min(doc.children.len().saturating_sub(1));
    let tag = spans
        .iter()
        .find(|s| !s.vis.is_empty() || s.payload == Payload::None)
        .map(|s| s.block.clone())
        .unwrap_or_else(|| tag_for_node(doc.children.get(block_ix)));
    let kind = row_kind(&spans);
    Row {
        vis,
        block_ix,
        tag,
        spans,
        kind,
        first_in_block: false,
        last_in_block: false,
        code_origin: 0,
    }
}

/// Tag for span-less rows (blank lines) from the owning block's kind.
fn tag_for_node(node: Option<&Node>) -> BlockTag {
    match node {
        Some(Node::Block { kind, .. }) => match kind {
            BlockKind::Heading(l) => BlockTag::Heading(*l),
            BlockKind::CodeBlock { lang, .. } => BlockTag::CodeBlock { lang: lang.clone() },
            BlockKind::BlockQuote => BlockTag::Quote { depth: 1 },
            BlockKind::Item { .. } | BlockKind::List { .. } => BlockTag::Item {
                depth: 0,
                ordered: false,
                task: false,
            },
            _ => BlockTag::Paragraph,
        },
        _ => BlockTag::Paragraph,
    }
}

fn mark_block_edges(rows: &mut [Row]) {
    let n = rows.len();
    let mut cursor = 0usize; // running code-text offset inside the current block
    let mut last_block = usize::MAX;
    for i in 0..n {
        let b = rows[i].block_ix;
        rows[i].first_in_block = i == 0 || rows[i - 1].block_ix != b;
        rows[i].last_in_block = i + 1 == n || rows[i + 1].block_ix != b;
        if b != last_block {
            cursor = 0;
            last_block = b;
        }
        if is_code_row(&rows[i]) {
            rows[i].code_origin = cursor;
            cursor += rows[i]
                .spans
                .iter()
                .filter(|s| s.payload != Payload::Marker)
                .map(|s| s.vis.len())
                .sum::<usize>()
                + 1; // the '\n' `code_block_text` appends per row
        }
    }
}

/// Standalone image = row whose only content spans are image-marked (or the
/// revealed image marker). `---` rows → Rule.
fn row_kind(spans: &[Span]) -> RowKind {
    if spans.iter().any(|s| s.payload == Payload::Rule) {
        return RowKind::Rule;
    }
    let content: Vec<&Span> = spans
        .iter()
        .filter(|s| !s.vis.is_empty() && s.payload != Payload::Marker)
        .collect();
    if let Some(img) = spans.iter().find_map(|s| match &s.payload {
        Payload::Image { src } => Some(src.to_string()),
        _ => None,
    }) {
        // Every visible char of the row belongs to the image (alt or the
        // revealed `![](…)` marker) → the row renders as an image widget.
        let all_image = content
            .iter()
            .all(|s| s.marks.has(Marks::IMAGE) || s.payload == Payload::Marker);
        if all_image {
            return RowKind::StandaloneImage { src: img };
        }
    }
    RowKind::Text
}

/// True while `row` is inside a fenced code block (`BlockTag::CodeBlock`).
pub fn is_code_row(row: &Row) -> bool {
    matches!(row.tag, BlockTag::CodeBlock { .. })
}

/// Code content of a block: concatenated non-marker text of its rows
/// (`\n` after each). Row i's slice starts at `row.code_origin`.
pub fn code_block_text(rows: &[Row], block_ix: usize, proj_text: &str) -> String {
    let mut text = String::new();
    for row in rows.iter().filter(|r| r.block_ix == block_ix) {
        for s in &row.spans {
            if s.payload != Payload::Marker {
                text.push_str(&proj_text[s.vis.clone()]);
            }
        }
        text.push('\n');
    }
    text
}

/// The fence info string for the block (for the picker + highlighter).
pub fn code_lang(tag: &BlockTag) -> Option<&str> {
    match tag {
        BlockTag::CodeBlock { lang } => lang.as_deref(),
        _ => None,
    }
}

/// First fenced code block containing source pos (for the picker hotkey).
pub fn code_block_at(doc: &Doc, pos: usize) -> Option<(usize, &Node)> {
    doc.children.iter().enumerate().find_map(|(i, n)| {
        if n.range().start <= pos
            && pos <= n.range().end
            && matches!(
                n,
                Node::Block {
                    kind: BlockKind::CodeBlock { fenced: true, .. },
                    ..
                }
            )
        {
            Some((i, n))
        } else {
            None
        }
    })
}
