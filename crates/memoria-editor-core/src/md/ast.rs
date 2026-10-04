//! Block/inline tree over the markdown source. Every node carries its source
//! byte range; the source text is the single source of truth (the tree only
//! annotates it).

use std::ops::Range;

pub type RangeB = Range<usize>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    Paragraph,
    Heading(u8),
    BlockQuote,
    CodeBlock {
        lang: Option<String>,
        fenced: bool,
    },
    List {
        ordered: bool,
        start: u64,
    },
    /// `task`: Some(checked) for task items.
    Item {
        task: Option<bool>,
    },
    Table {
        aligns: Vec<Align>,
    },
    TableHead,
    TableRow,
    TableCell,
    HtmlBlock,
    /// Footnote defs, definition lists, metadata blocks — kept opaque.
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineKind {
    Emphasis,
    Strong,
    Strikethrough,
    Link { dest: String, title: String },
    Image { src: String, title: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Block {
        kind: BlockKind,
        range: RangeB,
        children: Vec<Node>,
    },
    Inline {
        kind: InlineKind,
        range: RangeB,
        children: Vec<Node>,
    },
    /// Inline text leaf. `text` is the resolved payload (escapes/entities
    /// already decoded); when `text.len() != range.len()` the byte mapping
    /// is atomic.
    Text { range: RangeB, text: Box<str> },
    /// Inline code span; `range` covers the backticks.
    Code { range: RangeB, text: Box<str> },
    /// Raw inline/block HTML — kept literal.
    Html { range: RangeB },
    /// Line break inside a paragraph. `hard` = `\`- or trailing-space break.
    Break { range: RangeB, hard: bool },
    /// `[ ]` / `[x]` marker of a task item.
    TaskMark { range: RangeB, checked: bool },
    /// `---` / `***` / `___`.
    Rule { range: RangeB },
    /// `[^label]` footnote reference — rendered literally for now.
    FootnoteRef { range: RangeB },
}

impl Node {
    pub fn range(&self) -> RangeB {
        match self {
            Node::Block { range, .. }
            | Node::Inline { range, .. }
            | Node::Text { range, .. }
            | Node::Code { range, .. }
            | Node::Html { range }
            | Node::Break { range, .. }
            | Node::TaskMark { range, .. }
            | Node::Rule { range }
            | Node::FootnoteRef { range } => range.clone(),
        }
    }

    pub fn is_block(&self) -> bool {
        matches!(self, Node::Block { .. })
    }
}

/// Document root — a block container spanning the whole source.
#[derive(Debug, Clone)]
pub struct Doc {
    pub range: RangeB,
    pub children: Vec<Node>,
}

/// The fenced code block containing `pos`, if any.
pub fn fenced_block_containing(doc: &Doc, pos: usize) -> Option<RangeB> {
    fn walk(ns: &[Node], pos: usize) -> Option<crate::md::ast::RangeB> {
        for n in ns {
            if let Node::Block {
                kind,
                range,
                children,
            } = n
            {
                if let BlockKind::CodeBlock { fenced: true, .. } = kind {
                    if range.start <= pos && pos <= range.end {
                        return Some(range.clone());
                    }
                }
                if range.start <= pos && pos <= range.end {
                    if let Some(r) = walk(children, pos) {
                        return Some(r);
                    }
                }
            }
        }
        None
    }
    walk(&doc.children, pos)
}
