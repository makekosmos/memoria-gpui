//! Leaf emission helpers: text leaves (with bare-URL splitting), task
//! checkboxes, and plain/atomic/hidden emitters' callers.

use super::{Ctx, Walker};
use crate::md::ast::RangeB;
use crate::project::autolink::scan_bare_links;
use crate::project::types::{Marks, Payload};

impl<'a> Walker<'a> {
    pub(super) fn task_mark(&mut self, range: &RangeB, checked: bool, ctx: &Ctx) {
        let shown = ctx.item_range.start != usize::MAX
            && self.sel.touches(ctx.item_range.start, ctx.item_range.end);
        if shown {
            let c = Ctx {
                marks: ctx.marks.with(Marks::MARKER),
                payload: Payload::Marker,
                ..ctx.clone()
            };
            self.emit_text(range, &self.src[range.clone()], &c);
        } else {
            let c = Ctx {
                marks: ctx.marks.with(Marks::WIDGET),
                payload: Payload::TaskBox { checked },
                ..ctx.clone()
            };
            self.emit_atomic(range, if checked { "\u{2611}" } else { "\u{2610}" }, &c);
        }
    }

    pub(super) fn text_leaf(&mut self, range: &RangeB, text: &str, ctx: &Ctx) {
        if !ctx.marks.has(Marks::LINK)
            && !ctx.marks.has(Marks::CODE)
            && !ctx.marks.has(Marks::IMAGE)
            && text.len() == range.len()
        {
            let links = scan_bare_links(text);
            if !links.is_empty() {
                self.emit_text_with_links(range, text, links, ctx);
                return;
            }
        }
        self.emit_text(range, text, ctx);
    }

    fn emit_text_with_links(
        &mut self,
        range: &RangeB,
        text: &str,
        links: Vec<(usize, usize, String)>,
        ctx: &Ctx,
    ) {
        let mut pos = 0;
        for (off, len, dest) in links {
            if off > pos {
                self.emit_text(
                    &(range.start + pos..range.start + off),
                    &text[pos..off],
                    ctx,
                );
            }
            let c = Ctx {
                marks: ctx.marks.with(Marks::LINK),
                payload: Payload::Link {
                    dest: dest.into(),
                    title: "".into(),
                },
                ..ctx.clone()
            };
            self.emit_text(
                &(range.start + off..range.start + off + len),
                &text[off..off + len],
                &c,
            );
            pos = off + len;
        }
        if pos < text.len() {
            self.emit_text(
                &(range.start + pos..range.start + text.len()),
                &text[pos..],
                ctx,
            );
        }
    }
}
