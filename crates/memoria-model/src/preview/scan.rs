//! Markdown → plain-text scanning for card previews — see `preview.rs`.

use super::syntax::*;

/// Markdown → plain text. Blocks collapse to single newlines; inline markers
/// (`**`, `` ` ``, `[[…]]`, `![…](…)`, `#`, `>`) are dropped. Not a full
/// markdown parser — a card preview only needs legible text.
pub(crate) fn plain_text(markdown: &str) -> String {
    let src: Vec<char> = markdown.chars().collect();
    let mut out = String::with_capacity(markdown.len());
    // Emphasis delimiters only drop once they pair up — an unmatched `*`,
    // `_` or `~` (`snake_case`, `2*3`, a trailing `a*`) is literal text.
    // Openers are pushed tentatively and recorded as (char, out_pos, len) so
    // a later closer can drain them from `out`.
    let mut openers: Vec<(char, usize, usize)> = Vec::new();
    let mut i = skip_list_marker(&src, 0);
    let mut line_start = true;
    let mut heading = false;
    let mut in_code_fence = false;
    while let Some(&ch) = src.get(i) {
        if in_code_fence {
            if ch == '`' && src.get(i + 1) == Some(&'`') && src.get(i + 2) == Some(&'`') {
                i += 3;
                in_code_fence = false;
            } else {
                out.push(ch);
                i += 1;
            }
            continue;
        }
        match ch {
            '\n' => {
                if !out.ends_with('\n') && !out.is_empty() {
                    out.push('\n');
                }
                line_start = true;
                heading = false;
                openers.clear();
                i = skip_list_marker(&src, i + 1);
            }
            '`' => {
                if src.get(i + 1) == Some(&'`') && src.get(i + 2) == Some(&'`') {
                    i += 3; // ``` fence → raw code mode
                    in_code_fence = true;
                } else if let Some((content, end)) = inline_code_span(&src, i) {
                    // Code span contents are literal — no markers inside.
                    for &c in &src[content] {
                        out.push(if c == '\n' { ' ' } else { c });
                    }
                    i = end;
                } else {
                    while src.get(i) == Some(&'`') {
                        i += 1;
                    }
                }
            }
            '\\' => {
                // `\*` escapes punctuation; a lone `\` stays literal.
                match src.get(i + 1) {
                    Some(&c) if c.is_ascii_punctuation() => {
                        out.push(c);
                        i += 2;
                    }
                    _ => {
                        out.push(ch);
                        i += 1;
                    }
                }
            }
            '#' if line_start => {
                while src.get(i) == Some(&'#') {
                    i += 1;
                }
                if src.get(i) == Some(&' ') {
                    i += 1;
                }
                heading = true;
            }
            '#' if heading && atx_close(&src, i) => {
                // Trailing `#` run on a heading line — ATX closing sequence.
                while matches!(out.chars().last(), Some(' ') | Some('\t')) {
                    out.pop();
                }
                while !matches!(src.get(i), None | Some('\n')) {
                    i += 1;
                }
            }
            '>' if line_start => {
                // Quote markers: `>`, `> >`, plus the separating space.
                loop {
                    while src.get(i) == Some(&'>') {
                        i += 1;
                    }
                    if src.get(i) != Some(&' ') {
                        break;
                    }
                    i += 1;
                    if src.get(i) != Some(&'>') {
                        break;
                    }
                }
                // Still at a line start inside the quote — `> # h` is a
                // heading, `> - li` a list item.
                i = skip_list_marker(&src, i);
                continue;
            }
            '*' | '_' | '~' => {
                let start = i;
                while src.get(i) == Some(&ch) {
                    i += 1;
                }
                let run = i - start;
                let prev = start.checked_sub(1).and_then(|p| src.get(p));
                let next = src.get(i);
                let (could_open, could_close) = match ch {
                    // `_` can't open/close intra-word (CommonMark).
                    '_' => (
                        next.is_some_and(|c| !c.is_whitespace())
                            && !prev.is_some_and(|c| c.is_alphanumeric()),
                        prev.is_some_and(|c| !c.is_whitespace())
                            && !next.is_some_and(|c| c.is_alphanumeric()),
                    ),
                    // `~~` strikes; a single `~` is literal.
                    '~' if run >= 2 => (
                        next.is_some_and(|c| !c.is_whitespace()),
                        prev.is_some_and(|c| !c.is_whitespace()),
                    ),
                    '~' => (false, false),
                    _ => (
                        next.is_some_and(|c| !c.is_whitespace()),
                        prev.is_some_and(|c| !c.is_whitespace()),
                    ),
                };
                let closer = could_close
                    .then(|| openers.iter().rposition(|(c, _, _)| *c == ch))
                    .flatten();
                if let Some(oi) = closer {
                    let (_, opos, olen) = openers.remove(oi);
                    out.drain(opos..opos + olen);
                    for o in openers.iter_mut() {
                        if o.1 > opos {
                            o.1 -= olen;
                        }
                    }
                } else {
                    let pos = out.len();
                    for _ in 0..run {
                        out.push(ch);
                    }
                    if could_open {
                        openers.push((ch, pos, run));
                    }
                }
            }
            '!' => {
                let image_end = if src.get(i + 1) == Some(&'[') {
                    if src.get(i + 2) == Some(&'[') {
                        wiki_link(&src, i + 1).map(|(_, end)| end)
                    } else {
                        inline_link(&src, i + 1).map(|(_, end)| end)
                    }
                } else {
                    None
                };
                match image_end {
                    // `![alt](src)` / `![[t]]` — image nodes carry no text.
                    Some(end) => i = end,
                    None => {
                        out.push(ch);
                        i += 1;
                    }
                }
            }
            '[' => {
                let linked = if src.get(i + 1) == Some(&'[') {
                    wiki_link(&src, i).map(|(inner, end)| {
                        // `[[target|label]]` — show the label, else target.
                        let label = src[inner.clone()]
                            .iter()
                            .position(|&c| c == '|')
                            .map_or(inner.clone(), |o| inner.start + o + 1..inner.end);
                        (label, end)
                    })
                } else {
                    inline_link(&src, i)
                };
                match linked {
                    Some((label, end)) => {
                        let text: String = src[label].iter().collect();
                        out.push_str(&plain_text(&text));
                        i = end;
                    }
                    None => {
                        out.push(ch);
                        i += 1;
                    }
                }
            }
            _ => {
                out.push(ch);
                i += 1;
            }
        }
        if ch != '\n' {
            line_start = false;
        }
    }
    // Collapse 3+ newlines (Vue `tiptapPlainText` does the same).
    let mut collapsed = String::with_capacity(out.len());
    let mut newlines = 0;
    for ch in out.chars() {
        if ch == '\n' {
            newlines += 1;
            if newlines <= 2 {
                collapsed.push(ch);
            }
        } else {
            newlines = 0;
            collapsed.push(ch);
        }
    }
    collapsed.trim().to_string()
}
