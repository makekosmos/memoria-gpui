//! Live-preview projection: walks the parse tree and emits the visible text,
//! styled spans, and the source map. Syntax markers are hidden unless the
//! selection touches the owning construct (Obsidian/SuperMD rule).

use super::types::{BlockTag, Chunk, Marks, Payload, Projection, Span};
use crate::cursor::Selection;
use crate::md::ast::{Doc, Node, RangeB};
use crate::md::markers::{self, Marker};

/// Project `doc` to its visible form under selection `sel`.
pub fn project(doc: &Doc, src: &str, sel: Selection) -> Projection {
    let mut w = Walker {
        src,
        sel,
        markers: markers::collect(&doc.children, src),
        text: String::with_capacity(src.len()),
        spans: Vec::new(),
        chunks: Vec::new(),
    };
    w.siblings(&doc.range, &doc.children, &Ctx::top());
    Projection {
        text: w.text,
        spans: w.spans,
        chunks: w.chunks,
    }
}

/// Style/block context inherited while descending the tree.
#[derive(Clone)]
struct Ctx {
    marks: Marks,
    block: BlockTag,
    payload: Payload,
    item_depth: u8,
    item_ordered: bool,
    item_range: RangeB,
    in_table_head: bool,
}

impl Ctx {
    fn top() -> Self {
        Self {
            marks: Marks::default(),
            block: BlockTag::Other,
            payload: Payload::None,
            item_depth: 0,
            item_ordered: false,
            item_range: usize::MAX..usize::MAX,
            in_table_head: false,
        }
    }
}

struct Walker<'a> {
    src: &'a str,
    sel: Selection,
    markers: Vec<Marker>,
    text: String,
    spans: Vec<Span>,
    chunks: Vec<Chunk>,
}

impl<'a> Walker<'a> {
    /// Emit children in order with edge + inter-child gaps. Between two block
    /// siblings a `\n` joiner is always emitted (blocks live on own lines).
    fn siblings(&mut self, range: &RangeB, children: &[Node], ctx: &Ctx) {
        let mut prev = range.start;
        let mut prev_block = false;
        let mut first = true;
        for child in children {
            let gap_r = prev..child.range().start;
            let before = self.text.len();
            self.gap(gap_r.clone(), ctx, first);
            if self.text.len() == before && !first && (prev_block || child.is_block()) {
                // Adjacent block siblings (their own `\n` sits inside the
                // previous block's range) — still need a line separator.
                self.emit_atomic(&gap_r, "\n", ctx);
            }
            self.node(child, ctx);
            prev = child.range().end;
            prev_block = child.is_block();
            first = false;
        }
        self.gap(prev..range.end, ctx, true);
    }

    fn container(&mut self, range: &RangeB, children: &[Node], ctx: &Ctx) {
        self.siblings(range, children, ctx);
    }

    /// Emit the region between two siblings / container edges: literal text,
    /// markers (shown when their owner construct is under the selection, else
    /// hidden), and newline-collapsing rules.
    ///
    /// `edge` = container edge gap (leading/trailing): newlines are dropped,
    /// because line separation is the parent's job.
    fn gap(&mut self, r: RangeB, ctx: &Ctx, edge: bool) {
        if r.is_empty() {
            return;
        }
        // Carve marker ranges out of the gap.
        let mi = self.markers.partition_point(|m| m.range.end <= r.start);
        let mut pieces: Vec<(RangeB, Option<usize>)> = Vec::new(); // (src, marker idx)
        let mut pos = r.start;
        for (i, m) in self.markers.iter().enumerate().skip(mi) {
            if m.range.start >= r.end {
                break;
            }
            let s = m.range.start.max(r.start);
            let e = m.range.end.min(r.end);
            if pos < s {
                pieces.push((pos..s, None));
            }
            if s < e {
                pieces.push((s..e, Some(i)));
            }
            pos = pos.max(e);
        }
        if pos < r.end {
            pieces.push((pos..r.end, None));
        }

        let has_nl = self.src[r.clone()].contains('\n');
        // Collapse: nothing but hidden markers and whitespace, at least one
        // newline, and no revealed marker -> one joiner (`\n` + tail indent).
        let collapsible = pieces.iter().all(|(pr, m)| match m {
            Some(i) => !self
                .sel
                .touches(self.markers[*i].owner.start, self.markers[*i].owner.end),
            None => self.src[pr.clone()]
                .bytes()
                .all(|b| matches!(b, b'\n' | b'\r' | b' ' | b'\t')),
        });
        if has_nl && collapsible && !edge {
            // `\n` + trailing indent, but only from literal (non-marker) ws —
            // a hidden `> ` quote prefix must not leak into the joiner.
            let mut joiner = String::from("\n");
            if let Some((pr, None)) = pieces.last() {
                let tail: String = self.src[pr.clone()]
                    .chars()
                    .rev()
                    .take_while(|c| *c == ' ' || *c == '\t')
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                joiner.push_str(&tail);
            }
            self.emit_atomic(&r, &joiner, ctx);
            return;
        }

        for (pr, marker) in pieces {
            match marker {
                Some(i)
                    if self
                        .sel
                        .touches(self.markers[i].owner.start, self.markers[i].owner.end) =>
                {
                    let c = Ctx {
                        marks: ctx.marks.with(Marks::MARKER),
                        payload: Payload::Marker,
                        ..ctx.clone()
                    };
                    self.emit_text(&pr, &self.src[pr.clone()], &c);
                }
                Some(i) => match self.markers[i].subst.clone() {
                    Some(sub) => self.emit_atomic(&pr, &sub, &plain(ctx)),
                    None => self.hidden(pr),
                },
                None if edge => {
                    // Edge gap: drop line terminators, keep other chars.
                    let keep: String = self.src[pr.clone()]
                        .chars()
                        .filter(|c| *c != '\n' && *c != '\r')
                        .collect();
                    if keep.is_empty() {
                        self.hidden(pr);
                    } else {
                        self.emit_atomic(&pr, &keep, &plain(ctx));
                    }
                }
                None => self.emit_text(&pr, &self.src[pr.clone()], ctx),
            }
        }
    }

    /// 1:1 (or resolved-text) emission of `text` covering `src` range.
    fn emit_text(&mut self, src: &RangeB, text: &str, ctx: &Ctx) {
        if text.is_empty() {
            self.hidden(src.clone());
            return;
        }
        let vis = self.text.len()..self.text.len() + text.len();
        self.text.push_str(text);
        self.chunks.push(Chunk {
            vis: vis.clone(),
            src: src.clone(),
            atomic: text.len() != src.len(),
        });
        self.spans.push(Span {
            vis,
            src: src.clone(),
            marks: ctx.marks,
            block: ctx.block.clone(),
            payload: ctx.payload.clone(),
        });
    }

    /// Visible `text` replacing the whole `src` range (breaks, glyphs, joins).
    fn emit_atomic(&mut self, src: &RangeB, text: &str, ctx: &Ctx) {
        let vis = self.text.len()..self.text.len() + text.len();
        self.text.push_str(text);
        self.chunks.push(Chunk {
            vis: vis.clone(),
            src: src.clone(),
            atomic: true,
        });
        self.spans.push(Span {
            vis,
            src: src.clone(),
            marks: ctx.marks,
            block: ctx.block.clone(),
            payload: ctx.payload.clone(),
        });
    }

    /// Source range that produces no visible bytes.
    fn hidden(&mut self, src: RangeB) {
        if src.is_empty() {
            return;
        }
        let at = self.text.len();
        self.chunks.push(Chunk {
            vis: at..at,
            src,
            atomic: true,
        });
    }
}

fn plain(ctx: &Ctx) -> Ctx {
    Ctx {
        payload: Payload::None,
        ..ctx.clone()
    }
}

fn trim_trailing_newline(src: &str, r: &mut RangeB) {
    let b = src.as_bytes();
    while r.end > r.start && matches!(b[r.end - 1], b'\n' | b'\r') {
        r.end -= 1;
    }
}

mod block;
mod leaf;
