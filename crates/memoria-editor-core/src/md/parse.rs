//! pulldown-cmark → `Doc` tree. Event byte ranges come from
//! `into_offset_iter`; we keep them verbatim so every node maps back to source.

use pulldown_cmark::{Alignment, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use super::ast::{Align, BlockKind, Doc, InlineKind, Node, RangeB};

pub fn parser_options() -> Options {
    let mut o = Options::empty();
    o.insert(Options::ENABLE_TABLES);
    o.insert(Options::ENABLE_STRIKETHROUGH);
    o.insert(Options::ENABLE_TASKLISTS);
    // GFM in pulldown-cmark 0.13 = [!NOTE]-style blockquote alerts only;
    // bare-URL autolinks are handled by our own post-pass (see project).
    o.insert(Options::ENABLE_GFM);
    o
}

pub fn parse(src: &str) -> Doc {
    let mut b = Builder::new(src);
    for (ev, range) in Parser::new_ext(src, parser_options()).into_offset_iter() {
        b.event(ev, range);
    }
    b.finish(src.len())
}

struct Frame {
    kind: FrameKind,
    range: RangeB,
    children: Vec<Node>,
}

enum FrameKind {
    Block(BlockKind),
    Inline(InlineKind),
    /// Transparent container: children are lifted into the parent.
    Flat,
}

struct Builder<'a> {
    src: &'a str,
    stack: Vec<Frame>,
    top: Vec<Node>,
}

impl<'a> Builder<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            stack: Vec::new(),
            top: Vec::new(),
        }
    }

    fn push_node(&mut self, n: Node) {
        match self.stack.last_mut() {
            Some(f) => f.children.push(n),
            None => self.top.push(n),
        }
    }

    fn event(&mut self, ev: Event<'a>, range: RangeB) {
        match ev {
            Event::Start(tag) => self.start_tag(tag, range),
            Event::End(tag) => self.end_tag(tag, range),
            Event::Text(text) => self.push_node(Node::Text {
                range,
                text: text.into_string().into_boxed_str(),
            }),
            Event::Code(text) => self.push_node(Node::Code {
                range,
                text: text.into_string().into_boxed_str(),
            }),
            Event::InlineMath(t) | Event::DisplayMath(t) => self.push_node(Node::Code {
                range,
                text: t.into_string().into_boxed_str(),
            }),
            Event::Html(_) | Event::InlineHtml(_) => self.push_node(Node::Html { range }),
            Event::SoftBreak => self.push_node(Node::Break { range, hard: false }),
            Event::HardBreak => self.push_node(Node::Break { range, hard: true }),
            Event::Rule => self.push_node(Node::Rule { range }),
            Event::TaskListMarker(checked) => self.push_node(Node::TaskMark { range, checked }),
            Event::FootnoteReference(_) => self.push_node(Node::FootnoteRef { range }),
        }
    }

    fn start_tag(&mut self, tag: Tag<'a>, range: RangeB) {
        let kind = match tag {
            Tag::Paragraph => FrameKind::Block(BlockKind::Paragraph),
            Tag::Heading { level, .. } => FrameKind::Block(BlockKind::Heading(h_level(level))),
            Tag::BlockQuote(_) => FrameKind::Block(BlockKind::BlockQuote),
            Tag::CodeBlock(k) => FrameKind::Block(BlockKind::CodeBlock {
                lang: fenced_lang(&k),
                fenced: matches!(k, pulldown_cmark::CodeBlockKind::Fenced(_)),
            }),
            Tag::List(start) => FrameKind::Block(BlockKind::List {
                ordered: start.is_some(),
                start: start.unwrap_or(1),
            }),
            Tag::Item => FrameKind::Block(BlockKind::Item { task: None }),
            Tag::Table(aligns) => FrameKind::Block(BlockKind::Table {
                aligns: aligns.iter().map(|a| align(*a)).collect(),
            }),
            Tag::TableHead => FrameKind::Block(BlockKind::TableHead),
            Tag::TableRow => FrameKind::Block(BlockKind::TableRow),
            Tag::TableCell => FrameKind::Block(BlockKind::TableCell),
            Tag::HtmlBlock => FrameKind::Block(BlockKind::HtmlBlock),
            Tag::Emphasis => FrameKind::Inline(InlineKind::Emphasis),
            Tag::Strong => FrameKind::Inline(InlineKind::Strong),
            Tag::Strikethrough => FrameKind::Inline(InlineKind::Strikethrough),
            Tag::Link {
                dest_url, title, ..
            } => FrameKind::Inline(InlineKind::Link {
                dest: dest_url.into_string(),
                title: title.into_string(),
            }),
            Tag::Image {
                dest_url, title, ..
            } => FrameKind::Inline(InlineKind::Image {
                src: dest_url.into_string(),
                title: title.into_string(),
            }),
            // FootnoteDefinition, MetadataBlock, DefinitionList*, Superscript,
            // Subscript and anything new: keep children, drop wrapper meaning.
            _ => FrameKind::Flat,
        };
        self.stack.push(Frame {
            kind,
            range,
            children: Vec::new(),
        });
    }

    fn end_tag(&mut self, _tag: TagEnd, _end_range: RangeB) {
        let Some(frame) = self.stack.pop() else {
            return;
        };
        let range = frame.range.clone();
        let node = match &frame.kind {
            FrameKind::Block(kind) => Node::Block {
                kind: postproc_block(kind.clone(), &frame, self.src),
                range,
                children: frame.children,
            },
            FrameKind::Inline(kind) => Node::Inline {
                kind: kind.clone(),
                range,
                children: frame.children,
            },
            FrameKind::Flat => {
                for c in frame.children {
                    self.push_node(c);
                }
                return;
            }
        };
        self.push_node(node);
    }

    fn finish(mut self, len: usize) -> Doc {
        while let Some(frame) = self.stack.pop() {
            // Unterminated construct: keep children, drop the wrapper.
            for c in frame.children {
                self.push_node(c);
            }
        }
        let range = 0..len;
        Doc {
            range,
            children: self.top,
        }
    }
}

fn h_level(l: HeadingLevel) -> u8 {
    match l {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn fenced_lang(k: &pulldown_cmark::CodeBlockKind) -> Option<String> {
    match k {
        pulldown_cmark::CodeBlockKind::Fenced(l) => {
            let l = l.trim();
            (!l.is_empty()).then(|| l.to_string())
        }
        _ => None,
    }
}

fn align(a: Alignment) -> Align {
    match a {
        Alignment::None => Align::None,
        Alignment::Left => Align::Left,
        Alignment::Center => Align::Center,
        Alignment::Right => Align::Right,
    }
}

/// Detect task items: pulldown emits a `TaskListMarker` leaf inside the item.
fn postproc_block(kind: BlockKind, frame: &Frame, _src: &str) -> BlockKind {
    if let BlockKind::Item { task: _ } = kind {
        let checked = frame.children.iter().find_map(|c| match c {
            Node::TaskMark { checked, .. } => Some(*checked),
            _ => None,
        });
        return BlockKind::Item { task: checked };
    }
    kind
}
