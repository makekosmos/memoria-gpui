//! Marker extraction: the byte ranges a Live Preview hides when the cursor is
//! outside the construct (heading `#`, list `- `, `**`, fences, link syntax,
//! blockquote `>`, table pipes, ...).
//!
//! Ranges derive from node ranges vs. their children ranges plus a bounded
//! source scan — never wider than the construct's own range.

use super::ast::{BlockKind, InlineKind, Node, RangeB};

/// A marker region owned by the construct covering `owner`.
#[derive(Debug, Clone)]
pub struct Marker {
    /// Hidden bytes in source.
    pub range: RangeB,
    /// Construct that owns the marker; shown when selection touches it.
    pub owner: RangeB,
    /// Replacement text when hidden (e.g. `• ` for a bullet) — `None` = drop.
    pub subst: Option<String>,
}

pub fn collect(doc_children: &[Node], src: &str) -> Vec<Marker> {
    let mut out = Vec::new();
    for n in doc_children {
        walk(n, src, &mut out);
    }
    out.sort_by_key(|m| (m.range.start, m.range.end));
    out
}

fn walk(node: &Node, src: &str, out: &mut Vec<Marker>) {
    match node {
        Node::Block {
            kind,
            range,
            children,
        } => {
            block_markers(kind, range, children, src, out);
            for c in children {
                walk(c, src, out);
            }
        }
        Node::Inline {
            kind,
            range,
            children,
        } => {
            inline_markers(kind, range, children, src, out);
            for c in children {
                walk(c, src, out);
            }
        }
        _ => {}
    }
}

fn emit(out: &mut Vec<Marker>, owner: &RangeB, range: RangeB) {
    if range.start < range.end {
        out.push(Marker {
            range,
            owner: owner.clone(),
            subst: None,
        });
    }
}

fn emit_subst(out: &mut Vec<Marker>, owner: &RangeB, range: RangeB, subst: String) {
    if range.start < range.end {
        out.push(Marker {
            range,
            owner: owner.clone(),
            subst: Some(subst),
        });
    }
}

fn block_markers(
    kind: &BlockKind,
    range: &RangeB,
    children: &[Node],
    src: &str,
    out: &mut Vec<Marker>,
) {
    match kind {
        BlockKind::Heading(_) => heading_markers(range, children, src, out),
        BlockKind::Item { .. } => {
            // `- ` / `1. ` / `+ ` up to first child (a TaskMark or content).
            // Ordered markers (`1.`) carry info — they stay literal text.
            if let Some(first) = children.first() {
                let mr = range.start..first.range().start;
                let mtxt = &src[mr.clone()];
                let indent: String = mtxt
                    .chars()
                    .take_while(|c| *c == ' ' || *c == '\t')
                    .collect();
                let body = mtxt[indent.len()..].trim_end();
                if !body.as_bytes().first().is_some_and(|b| b.is_ascii_digit()) {
                    let subst = if matches!(first, Node::TaskMark { .. }) {
                        indent
                    } else {
                        format!("{indent}\u{2022} ")
                    };
                    emit_subst(out, range, mr, subst);
                }
            }
        }
        BlockKind::BlockQuote => quote_markers(range, src, out),
        BlockKind::CodeBlock { fenced: true, .. } => {
            // Fenced: opening ```lang line and closing ``` line.
            if let (Some(first), Some(last)) = (children.first(), children.last()) {
                emit(out, range, range.start..first.range().start);
                emit(out, range, last.range().end..range.end);
            } else {
                emit(out, range, range.clone());
            }
        }
        BlockKind::Table { .. } => table_markers(range, src, out),
        _ => {}
    }
}

/// `#` run + following space at line start; optional ATX closing run.
fn heading_markers(range: &RangeB, children: &[Node], src: &str, out: &mut Vec<Marker>) {
    let bytes = src.as_bytes();
    let mut content_end = range.end;
    while content_end > range.start && matches!(bytes[content_end - 1], b'\n' | b'\r') {
        content_end -= 1;
    }
    match children.first() {
        None => emit(out, range, range.start..content_end),
        Some(first) => {
            emit(out, range, range.start..first.range().start);
            let Some(last) = children.last() else { return };
            // Trailing ` ###` closing sequence.
            let mut s = content_end;
            while s > last.range().end && bytes[s - 1] == b'#' {
                s -= 1;
            }
            if s < content_end {
                while s > last.range().end && bytes[s - 1] == b' ' {
                    s -= 1;
                }
                emit(out, range, s..content_end);
            }
        }
    }
}

/// Per-line `>` + optional following space, inside the quote range.
fn quote_markers(range: &RangeB, src: &str, out: &mut Vec<Marker>) {
    let bytes = src.as_bytes();
    let mut pos = range.start;
    while pos < range.end {
        let mut line_end = pos;
        while line_end < bytes.len() && bytes[line_end] != b'\n' {
            line_end += 1;
        }
        let line_end = line_end.min(range.end);
        // Up to 3 leading spaces, then '>', then one optional space.
        let mut p = pos;
        let mut spaces = 0;
        while p < line_end && bytes[p] == b' ' && spaces < 3 {
            p += 1;
            spaces += 1;
        }
        if p < line_end && bytes[p] == b'>' {
            p += 1;
            if p < line_end && bytes[p] == b' ' {
                p += 1;
            }
            emit(out, range, pos..p);
        }
        pos = if line_end < bytes.len() && line_end < range.end {
            line_end + 1 // skip '\n'
        } else {
            range.end
        };
    }
}

/// Table: `|` pipes outside cell content and the `|---|---|` delimiter row.
fn table_markers(range: &RangeB, src: &str, out: &mut Vec<Marker>) {
    // Delimiter row: any line whose trimmed content is only |, -, :, spaces.
    let bytes = src.as_bytes();
    let mut pos = range.start;
    while pos < range.end {
        let mut line_end = pos;
        while line_end < bytes.len() && bytes[line_end] != b'\n' {
            line_end += 1;
        }
        let end = line_end.min(range.end);
        let line = &src[pos..end];
        let t = line.trim();
        let is_delim = !t.is_empty()
            && t.bytes().all(|b| matches!(b, b'|' | b'-' | b':' | b' '))
            && t.contains('-');
        if is_delim {
            emit(out, range, pos..end);
        } else {
            // Pipes on this line → thin separators. `\|` is content.
            let mut i = pos;
            while i < end {
                if bytes[i] == b'|' && (i == 0 || bytes[i - 1] != b'\\') {
                    emit_subst(out, range, i..i + 1, "\u{2502}".to_string());
                }
                i += 1;
            }
        }
        pos = if end < range.end && end < bytes.len() {
            end + 1
        } else {
            range.end
        };
    }
}

fn inline_markers(
    kind: &InlineKind,
    range: &RangeB,
    children: &[Node],
    src: &str,
    out: &mut Vec<Marker>,
) {
    let first = children.first().map(|c| c.range());
    let last = children.last().map(|c| c.range());
    match kind {
        InlineKind::Emphasis | InlineKind::Strong | InlineKind::Strikethrough => {
            let delim = src.as_bytes()[range.start];
            emit_delim_run(
                out,
                range,
                src,
                range.start,
                first.map(|r| r.start),
                delim,
                true,
            );
            emit_delim_run(
                out,
                range,
                src,
                range.end,
                last.map(|r| r.end),
                delim,
                false,
            );
        }
        InlineKind::Link { .. } | InlineKind::Image { .. } => {
            // `[`/`![` opener and `](dest)`/`](dest "t")`/`][ref]` closer.
            let open_end = match (kind, first) {
                (InlineKind::Image { .. }, Some(f)) => f.start,
                (InlineKind::Image { .. }, None) => (range.start + 2).min(range.end),
                (_, Some(f)) => f.start,
                (_, None) => (range.start + 1).min(range.end),
            };
            emit(out, range, range.start..open_end);
            let close_start = last.map(|r| r.end).unwrap_or(open_end);
            emit(out, range, close_start..range.end);
        }
    }
}

/// Leading/trailing run of `delim`, clamped to the gap between the construct
/// edge and its first/last child (so `***x***` splits correctly across the
/// nested emphasis levels).
fn emit_delim_run(
    out: &mut Vec<Marker>,
    owner: &RangeB,
    src: &str,
    edge: usize,
    child_bound: Option<usize>,
    delim: u8,
    at_start: bool,
) {
    let bytes = src.as_bytes();
    if at_start {
        let bound = child_bound.unwrap_or(owner.end);
        let mut e = edge;
        while e < bound && bytes[e] == delim {
            e += 1;
        }
        emit(out, owner, edge..e);
    } else {
        let bound = child_bound.unwrap_or(owner.start);
        let mut s = edge;
        while s > bound && bytes[s - 1] == delim {
            s -= 1;
        }
        emit(out, owner, s..edge);
    }
}
