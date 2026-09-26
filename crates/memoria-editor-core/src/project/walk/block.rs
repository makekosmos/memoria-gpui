//! Block-level dispatch and container walking for the projection.

use super::{trim_trailing_newline, Ctx, Walker};
use crate::md::ast::{BlockKind, InlineKind, Node, RangeB};
use crate::project::types::{BlockTag, Marks, Payload};

impl<'a> Walker<'a> {
    pub(super) fn node(&mut self, node: &Node, ctx: &Ctx) {
        match node {
            Node::Block {
                kind,
                range,
                children,
            } => self.block(kind, range, children, ctx),
            Node::Inline {
                kind,
                range,
                children,
            } => {
                let mut c = ctx.clone();
                match kind {
                    InlineKind::Emphasis => c.marks = c.marks.with(Marks::ITALIC),
                    InlineKind::Strong => c.marks = c.marks.with(Marks::BOLD),
                    InlineKind::Strikethrough => c.marks = c.marks.with(Marks::STRIKE),
                    InlineKind::Link { dest, title } => {
                        c.marks = c.marks.with(Marks::LINK);
                        c.payload = Payload::Link {
                            dest: dest.as_str().into(),
                            title: title.as_str().into(),
                        };
                    }
                    InlineKind::Image { src, .. } => {
                        c.marks = c.marks.with(Marks::IMAGE);
                        c.payload = Payload::Image {
                            src: src.as_str().into(),
                        };
                    }
                }
                self.container(range, children, &c);
            }
            Node::Text { range, text } => self.text_leaf(range, text, ctx),
            Node::Code { range, text } => {
                let c = Ctx {
                    marks: ctx.marks.with(Marks::CODE),
                    ..ctx.clone()
                };
                self.emit_text(range, text, &c);
            }
            Node::Html { range } => {
                let c = Ctx {
                    payload: Payload::Html,
                    ..ctx.clone()
                };
                self.emit_text(range, &self.src[range.clone()], &c)
            }
            Node::Break { range, .. } => self.emit_atomic(range, "\n", ctx),
            Node::TaskMark { range, checked } => self.task_mark(range, *checked, ctx),
            Node::Rule { range } => {
                let mut r = range.clone();
                trim_trailing_newline(self.src, &mut r);
                let c = Ctx {
                    payload: Payload::Rule,
                    ..ctx.clone()
                };
                self.emit_text(&r, &self.src[r.clone()], &c);
            }
            Node::FootnoteRef { range } => self.emit_text(range, &self.src[range.clone()], ctx),
        }
    }

    pub(super) fn block(&mut self, kind: &BlockKind, range: &RangeB, children: &[Node], ctx: &Ctx) {
        // Table under the cursor shows its raw source lines.
        if matches!(kind, BlockKind::Table { .. }) && self.sel.touches(range.start, range.end) {
            let mut r = range.clone();
            trim_trailing_newline(self.src, &mut r);
            let c = Ctx {
                block: BlockTag::TableCell { header: false },
                payload: Payload::TableRaw,
                ..ctx.clone()
            };
            self.emit_text(&r, &self.src[r.clone()], &c);
            return;
        }
        let mut c = ctx.clone();
        match kind {
            BlockKind::Paragraph => c.block = BlockTag::Paragraph,
            BlockKind::Heading(l) => c.block = BlockTag::Heading(*l),
            BlockKind::BlockQuote => {
                c.block = BlockTag::Quote {
                    depth: match &ctx.block {
                        BlockTag::Quote { depth } => depth + 1,
                        _ => 1,
                    },
                }
            }
            BlockKind::CodeBlock { lang, fenced } => {
                c.block = BlockTag::CodeBlock { lang: lang.clone() };
                c.marks = c.marks.with(Marks::CODE);
                if *fenced {
                    return self.fenced_code(range, children, &c);
                }
            }
            BlockKind::List { ordered, .. } => c.item_ordered = *ordered,
            BlockKind::Item { task } => {
                c.item_depth += 1;
                c.item_range = range.clone();
                c.block = BlockTag::Item {
                    depth: c.item_depth - 1,
                    ordered: c.item_ordered,
                    task: task.is_some(),
                };
            }
            BlockKind::TableHead => c.in_table_head = true,
            BlockKind::TableCell => {
                c.block = BlockTag::TableCell {
                    header: c.in_table_head,
                }
            }
            _ => {}
        }
        self.container(range, children, &c);
    }

    /// Fenced code: content lines visible (CODE marks), fences are markers.
    /// When the fence is hidden, the content's trailing newline before ```
    /// and the `\n` after the opening fence are structural — dropped. When the
    /// caret reveals the fence, they emit normally so the marker keeps its
    /// own line.
    pub(super) fn fenced_code(&mut self, range: &RangeB, children: &[Node], ctx: &Ctx) {
        let shown = self.sel.touches(range.start, range.end);
        let mut prev = range.start;
        let n = children.len();
        for (i, child) in children.iter().enumerate() {
            self.gap(prev..child.range().start, ctx, !shown && i == 0);
            let full = child.range();
            let mut r = full.clone();
            if i == n - 1 && !shown {
                trim_trailing_newline(self.src, &mut r);
            }
            match child {
                Node::Text { text, .. } => {
                    let cut = full.end - r.end;
                    let t = &text[..text.len() - cut.min(text.len())];
                    self.emit_text(&r, t, ctx);
                    if r.end < full.end {
                        self.hidden(r.end..full.end);
                    }
                }
                other => self.node(other, ctx),
            }
            prev = full.end;
        }
        self.gap(prev..range.end, ctx, !shown);
    }
}
