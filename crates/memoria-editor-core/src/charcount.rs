//! Character count 1:1 with Vue `src/lib/charCount.ts`
//! (`countCharsInProseMirrorDoc`), evaluated over our parsed markdown tree:
//!
//! - text inside `text` nodes counts in **code points**;
//! - each leaf block (paragraph / heading / codeBlock / horizontalRule) beyond
//!   the first contributes one separator char;
//! - `hardBreak` (and in-paragraph soft breaks — Vue's markdown reader maps
//!   them to `hardBreak`) counts as one char;
//! - top-level `image` nodes split their paragraph but contribute nothing —
//!   Vue's `pushParagraphOrImageBlocks` does the same to the tiptap doc;
//! - tables count like raw text lines, because Vue's line-based markdown
//!   reader (`markdownToTiptapDoc`) never produces table nodes.

use crate::md::ast::{BlockKind, Doc, InlineKind, Node};

pub fn count_chars(doc: &Doc, src: &str) -> usize {
    let mut w = Counter {
        src,
        total: 0,
        leaves: 0,
    };
    for n in &doc.children {
        w.node(n);
    }
    w.total
}

struct Counter<'a> {
    src: &'a str,
    total: usize,
    leaves: usize,
}

impl<'a> Counter<'a> {
    /// A leaf block boundary: `+1` for every leaf after the first.
    fn leaf(&mut self) {
        if self.leaves > 0 {
            self.total += 1;
        }
        self.leaves += 1;
    }

    fn text(&mut self, s: &str) {
        self.total += s.chars().count();
    }

    fn inline(&mut self, n: &'a Node) {
        match n {
            Node::Inline {
                kind: InlineKind::Image { .. },
                ..
            } => {}
            Node::Inline { children, .. } => self.inlines(children),
            _ => self.node(n),
        }
    }

    fn inlines(&mut self, ns: &'a [Node]) {
        for n in ns {
            self.inline(n);
        }
    }

    /// Paragraph children split at top-level images into leaf segments.
    fn paragraph(&mut self, children: &'a [Node]) {
        let mut i = 0;
        while i < children.len() {
            if matches!(
                children[i],
                Node::Inline {
                    kind: InlineKind::Image { .. },
                    ..
                }
            ) {
                i += 1;
                continue;
            }
            let mut j = i;
            while j < children.len()
                && !matches!(
                    children[j],
                    Node::Inline {
                        kind: InlineKind::Image { .. },
                        ..
                    }
                )
            {
                j += 1;
            }
            self.leaf();
            self.inlines(&children[i..j]);
            i = j;
        }
    }

    fn node(&mut self, n: &'a Node) {
        match n {
            Node::Block {
                kind,
                range,
                children,
            } => match kind {
                BlockKind::Paragraph => self.paragraph(children),
                BlockKind::Heading(_) => {
                    self.leaf();
                    self.inlines(children);
                }
                BlockKind::CodeBlock { .. } => {
                    self.leaf();
                    self.inlines(children);
                }
                BlockKind::Table { .. } => {
                    self.leaf();
                    // Raw text incl. interior newlines as hard breaks.
                    let mut r = range.clone();
                    let b = self.src.as_bytes();
                    while r.end > r.start && matches!(b[r.end - 1], b'\n' | b'\r') {
                        r.end -= 1;
                    }
                    self.text(&self.src[r]);
                }
                BlockKind::Item { .. } => {
                    // Tiptap wraps item text in a paragraph -> a leaf block;
                    // loose lists carry real Paragraph children instead.
                    if children.iter().any(|c| !matches!(c, Node::Block { .. })) {
                        self.leaf();
                    }
                    for c in children {
                        self.node(c);
                    }
                }
                _ => {
                    for c in children {
                        self.node(c);
                    }
                }
            },
            Node::Text { text, .. } => self.text(text),
            Node::Code { text, .. } => self.text(text),
            Node::Html { range } => self.text(&self.src[range.clone()]),
            Node::Break { .. } => self.total += 1,
            Node::Rule { .. } => self.leaf(),
            Node::FootnoteRef { range } => self.text(&self.src[range.clone()]),
            Node::TaskMark { .. } => {}
            Node::Inline { .. } => self.inline(n),
        }
    }
}
