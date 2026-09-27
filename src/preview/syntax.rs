//! Leaf matchers for the preview scanner — positions over `&[char]`.

use std::ops::Range;

/// First content index after leading whitespace plus an optional
/// `- `/`+ `/`N. ` list marker at a line start.
pub(super) fn skip_list_marker(src: &[char], mut i: usize) -> usize {
    while matches!(src.get(i), Some(' ') | Some('\t')) {
        i += 1;
    }
    match src.get(i) {
        Some('-') | Some('+') if src.get(i + 1) == Some(&' ') => i + 2,
        Some(d) if d.is_ascii_digit() => {
            let mut j = i;
            while matches!(src.get(j), Some(c) if c.is_ascii_digit()) {
                j += 1;
            }
            if j > i && src.get(j) == Some(&'.') && src.get(j + 1) == Some(&' ') {
                j + 2
            } else {
                i
            }
        }
        _ => i,
    }
}

/// `[label](dest)` starting at a `[` → `(label_span, end)`. `\n` or an
/// unclosed bracket/paren means it isn't a link.
pub(super) fn inline_link(src: &[char], start: usize) -> Option<(Range<usize>, usize)> {
    let mut j = start + 1;
    let mut depth = 1usize;
    while j < src.len() {
        match src[j] {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            '\n' => return None,
            _ => {}
        }
        j += 1;
    }
    if src.get(j) != Some(&']') || src.get(j + 1) != Some(&'(') {
        return None;
    }
    let label = start + 1..j;
    j += 2;
    while j < src.len() {
        match src[j] {
            ')' => return Some((label, j + 1)),
            '\n' => return None,
            _ => j += 1,
        }
    }
    None
}

/// `[[target|label]]` starting at the first `[` → `(inner_span, end)`.
pub(super) fn wiki_link(src: &[char], start: usize) -> Option<(Range<usize>, usize)> {
    let mut j = start + 2;
    while j < src.len() {
        match src[j] {
            ']' if src.get(j + 1) == Some(&']') => return Some((start + 2..j, j + 2)),
            '[' | '\n' => return None,
            _ => j += 1,
        }
    }
    None
}

/// `` `x` `` / `` ``x`` `` inline code span at `start` → `(content, end)`.
/// Runs of 3+ backticks are block fences, handled by the caller.
pub(super) fn inline_code_span(src: &[char], start: usize) -> Option<(Range<usize>, usize)> {
    let ticks = if src.get(start + 1) == Some(&'`') {
        2
    } else {
        1
    };
    let mut j = start + ticks;
    while j < src.len() {
        if src[j] == '`' {
            let mut run = 0usize;
            while src.get(j + run) == Some(&'`') {
                run += 1;
            }
            if run == ticks {
                return Some((start + ticks..j, j + run));
            }
            j += run;
        } else {
            j += 1;
        }
    }
    None
}

/// Only `#`/whitespace until EOL → an ATX closing sequence (`# Foo #`).
pub(super) fn atx_close(src: &[char], mut i: usize) -> bool {
    while matches!(src.get(i), Some('#') | Some(' ') | Some('\t')) {
        i += 1;
    }
    matches!(src.get(i), None | Some('\n'))
}
