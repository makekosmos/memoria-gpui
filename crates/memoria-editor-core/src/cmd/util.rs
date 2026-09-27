//! Line-level source analysis shared by commands and key handlers.

use crate::md::ast::RangeB;

/// Line range (content, excl. terminator) containing byte `pos`.
pub fn line_range(src: &str, pos: usize) -> RangeB {
    let bytes = src.as_bytes();
    let pos = pos.min(bytes.len());
    let mut s = pos;
    while s > 0 && bytes[s - 1] != b'\n' {
        s -= 1;
    }
    let mut e = pos;
    while e < bytes.len() && bytes[e] != b'\n' {
        e += 1;
    }
    // strip `\r` from the content range
    if e > s && bytes[e - 1] == b'\r' {
        e -= 1;
    }
    s..e
}

/// All line content-ranges intersecting `[start, end)`.
pub fn lines_of(src: &str, start: usize, end: usize) -> Vec<RangeB> {
    let mut out = Vec::new();
    let mut s = line_range(src, start).start;
    while s <= end.max(start) && s < src.len() {
        let r = line_range(src, s);
        out.push(r.clone());
        if r.end >= src.len() {
            break;
        }
        // move past the terminator
        let mut n = r.end;
        if src.as_bytes().get(n) == Some(&b'\r') {
            n += 1;
        }
        if src.as_bytes().get(n) == Some(&b'\n') {
            n += 1;
        }
        s = n.max(r.end + 1);
    }
    if out.is_empty() {
        out.push(start..end);
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    Bullet,
    Ordered,
    Task,
}

/// Parsed list marker on a line: `^(\s*)(marker)(\s+)` + optional `[ ]`/`[x]`.
#[derive(Debug, Clone)]
pub struct ListMark {
    /// Leading whitespace range (indent).
    pub indent: RangeB,
    /// `- `/`+ `/`* `/`1. ` part.
    pub marker: RangeB,
    /// `[ ]`/`[x]` incl. its trailing space, when the item is a task.
    pub task: Option<(bool, RangeB)>,
    /// Byte offset where the item's text begins.
    pub content: usize,
    /// Ordered list number, when ordered.
    pub num: Option<u64>,
}

/// Parse a list marker at the start of `src[ls..le]`.
pub fn list_marker(src: &str, ls: usize, le: usize) -> Option<ListMark> {
    let bytes = src.as_bytes();
    let mut p = ls;
    while p < le && (bytes[p] == b' ' || bytes[p] == b'\t') {
        p += 1;
    }
    let indent = ls..p;
    let mstart = p;
    let mut num = None;
    if p < le && matches!(bytes[p], b'-' | b'+' | b'*') {
        p += 1;
    } else {
        // ordered: digits then . or )
        let d0 = p;
        while p < le && bytes[p].is_ascii_digit() {
            p += 1;
        }
        if p == d0 || p >= le || !matches!(bytes[p], b'.' | b')') {
            return None;
        }
        num = src[d0..p].parse::<u64>().ok();
        p += 1;
    }
    // at least one space after the marker
    if p >= le || bytes[p] != b' ' && bytes[p] != b'\t' {
        return None;
    }
    while p < le && (bytes[p] == b' ' || bytes[p] == b'\t') {
        p += 1;
    }
    let marker = mstart..p;
    // task checkbox
    let mut task = None;
    let mut content = p;
    if p + 3 <= le
        && bytes[p] == b'['
        && matches!(bytes[p + 1], b' ' | b'x' | b'X')
        && bytes[p + 2] == b']'
    {
        let checked = bytes[p + 1] != b' ';
        let mut e = p + 3;
        if e < le && (bytes[e] == b' ' || bytes[e] == b'\t') {
            e += 1;
        }
        task = Some((checked, p..e));
        content = e;
    }
    Some(ListMark {
        indent,
        marker,
        task,
        content,
        num,
    })
}

/// `#{1,6}` prefix at line start -> (level, marker end byte).
pub fn heading_marker(src: &str, ls: usize, le: usize) -> Option<(u8, usize)> {
    let bytes = src.as_bytes();
    let mut p = ls;
    let mut spaces = 0;
    while p < le && bytes[p] == b' ' && spaces < 4 {
        p += 1;
        spaces += 1;
    }
    if spaces == 4 {
        return None;
    }
    let h0 = p;
    while p < le && bytes[p] == b'#' {
        p += 1;
    }
    let level = (p - h0) as u8;
    if level == 0 || level > 6 {
        return None;
    }
    // require a space or EOL after the hashes
    if p < le && bytes[p] != b' ' && bytes[p] != b'\t' {
        return None;
    }
    if p < le {
        p += 1;
    }
    Some((level, p))
}

/// `>` + optional space at line start -> marker end byte.
pub fn quote_marker(src: &str, ls: usize, le: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    let mut p = ls;
    let mut spaces = 0;
    while p < le && bytes[p] == b' ' && spaces < 3 {
        p += 1;
        spaces += 1;
    }
    if p < le && bytes[p] == b'>' {
        p += 1;
        if p < le && bytes[p] == b' ' {
            p += 1;
        }
        return Some(p);
    }
    None
}

/// Fence line ```` ```lang ```` or `~~~lang` -> (fence char run end, lang).
pub fn fence_line(src: &str, ls: usize, le: usize) -> Option<(char, usize, String)> {
    let bytes = src.as_bytes();
    let mut p = ls;
    let mut spaces = 0;
    while p < le && bytes[p] == b' ' && spaces < 3 {
        p += 1;
        spaces += 1;
    }
    if p >= le || (bytes[p] != b'`' && bytes[p] != b'~') {
        return None;
    }
    let ch = bytes[p] as char;
    let f0 = p;
    while p < le && bytes[p] == ch as u8 {
        p += 1;
    }
    if p - f0 < 3 {
        return None;
    }
    let lang = src[p..le].trim().to_string();
    Some((ch, p, lang))
}

/// Snap `pos` inward to a char boundary (down for starts, up for ends).
pub fn snap_down(src: &str, pos: usize) -> usize {
    let mut p = pos.min(src.len());
    while p > 0 && !src.is_char_boundary(p) {
        p -= 1;
    }
    p
}

pub fn snap_up(src: &str, pos: usize) -> usize {
    let mut p = pos.min(src.len());
    while p < src.len() && !src.is_char_boundary(p) {
        p += 1;
    }
    p
}

/// Tiptap `exitCode` for Shift+Enter inside a fenced block: land on a fresh
/// line after the fence. Returns `(insert_pos, insert_text, caret)`. An
/// unclosed fence (runs to EOF) first gets a matching closing fence
/// appended so there is an "after" to land on.
pub(crate) fn code_exit_edit(src: &str, br: &crate::md::ast::RangeB) -> (usize, String, usize) {
    let block = &src[br.start.min(src.len())..br.end.min(src.len())];
    let tail = block.trim_end();
    let last = tail.rsplit('\n').next().unwrap_or(tail);
    // Closed = the last line is a fence that isn't the opener itself.
    let closed = tail.contains('\n') && {
        let t = last.trim_start_matches([' ', '\t']);
        let n = t.chars().take_while(|&c| c == '`' || c == '~').count();
        n >= 3 && t[n..].trim().is_empty()
    };
    let mut pos = br.end.min(src.len());
    if src.as_bytes().get(pos) == Some(&b'\n') {
        pos += 1;
    }
    if !closed {
        let opener: String = block
            .trim_start_matches([' ', '\t'])
            .chars()
            .take_while(|&c| c == '`' || c == '~')
            .collect();
        let fence = if opener.is_empty() { "```" } else { &opener };
        let ins = format!(
            "{}{fence}\n\n",
            if block.ends_with('\n') { "" } else { "\n" }
        );
        return (pos, ins.clone(), pos + ins.len());
    }
    if pos == src.len() {
        return (pos, "\n".into(), pos + 1);
    }
    // A blank line already follows the fence — land on it; otherwise leave
    // one so the new paragraph doesn't merge into the next line's text.
    let ins = if src.as_bytes().get(pos) == Some(&b'\n') {
        ""
    } else {
        "\n\n"
    };
    (pos, ins.into(), pos)
}
