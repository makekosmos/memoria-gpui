//! Projection output types: styled spans over projected ("visible") text plus
//! the bidirectional byte map between visible and source offsets.

pub use crate::md::ast::RangeB;

/// Inline style bits carried by a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Marks(pub u16);

impl Marks {
    pub const BOLD: Self = Self(1 << 0);
    pub const ITALIC: Self = Self(1 << 1);
    pub const STRIKE: Self = Self(1 << 2);
    pub const CODE: Self = Self(1 << 3);
    pub const LINK: Self = Self(1 << 4);
    /// Source-syntax marker revealed because the cursor touches its construct.
    pub const MARKER: Self = Self(1 << 5);
    /// Widget glyph (`☐`/`☑`) standing in for `[ ]`/`[x]`.
    pub const WIDGET: Self = Self(1 << 6);
    /// Span lives inside an image's alt slot.
    pub const IMAGE: Self = Self(1 << 7);

    pub fn has(self, m: Marks) -> bool {
        self.0 & m.0 != 0
    }

    pub fn with(self, m: Marks) -> Self {
        Self(self.0 | m.0)
    }
}

/// Nearest enclosing block — the line-level styling context.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockTag {
    Paragraph,
    Heading(u8),
    CodeBlock {
        lang: Option<String>,
    },
    Quote {
        depth: u8,
    },
    /// `depth` = list nesting level (0 = outermost list).
    Item {
        depth: u8,
        ordered: bool,
        task: bool,
    },
    TableCell {
        header: bool,
    },
    Other,
}

/// Semantic payload a renderer may need (link target, image src, ...).
#[derive(Debug, Clone, PartialEq)]
pub enum Payload {
    None,
    /// A revealed markdown marker (`#`, `**`, `> `, fence, `](…)`, `|`).
    Marker,
    Link {
        dest: Box<str>,
        title: Box<str>,
    },
    Image {
        src: Box<str>,
    },
    TaskBox {
        checked: bool,
    },
    Rule,
    Html,
    /// Raw table row source shown while the cursor is inside the table.
    TableRaw,
}

/// One styled run of visible text.
#[derive(Debug, Clone)]
pub struct Span {
    /// Byte range in `Projection::text`.
    pub vis: RangeB,
    /// Byte range in the markdown source.
    pub src: RangeB,
    pub marks: Marks,
    pub block: BlockTag,
    pub payload: Payload,
}

/// One map chunk. `vis` may be empty (hidden source). `atomic` chunks map
/// non-1:1 (glyph replacement, escapes, collapsed joins): interior offsets
/// snap to chunk boundaries.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub vis: RangeB,
    pub src: RangeB,
    pub atomic: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Projection {
    /// Projected text (what the user sees).
    pub text: String,
    pub spans: Vec<Span>,
    /// Sorted by `src`; `vis` contiguous covering `0..text.len()`.
    pub chunks: Vec<Chunk>,
}

impl Projection {
    /// Source byte offset → visible byte offset. Chunks tile the source, so a
    /// position inside a hidden chunk snaps to the removal point.
    pub fn to_visible(&self, src_pos: usize) -> usize {
        let i = self.chunks.partition_point(|c| c.src.end < src_pos);
        let Some(c) = self.chunks.get(i) else {
            return self.text.len();
        };
        if src_pos < c.src.start || c.vis.is_empty() {
            return c.vis.start;
        }
        if c.atomic {
            return if src_pos == c.src.end {
                c.vis.end
            } else {
                c.vis.start
            };
        }
        c.vis.start + (src_pos - c.src.start).min(c.src.end - c.src.start)
    }

    /// Visible byte offset → source byte offset.
    pub fn to_source(&self, vis_pos: usize) -> usize {
        let i = self.chunks.partition_point(|c| c.vis.end <= vis_pos);
        let Some(c) = self.chunks.get(i) else {
            return self.chunks.last().map(|c| c.src.end).unwrap_or(0);
        };
        if vis_pos < c.vis.start {
            return c.src.start;
        }
        if c.atomic {
            return if vis_pos >= c.vis.end {
                c.src.end
            } else {
                c.src.start
            };
        }
        c.src.start + (vis_pos - c.vis.start).min(c.src.end - c.src.start)
    }
}
