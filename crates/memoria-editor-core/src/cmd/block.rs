//! Block commands: headings, bullet/ordered/task lists, checkbox toggle,
//! blockquote, fenced code block, horizontal rule.

use crate::cmd::util::{heading_marker, line_range, lines_of, list_marker, quote_marker, ListKind};
use crate::cursor::Selection;
use crate::editor::Tx;
use crate::md::ast::{BlockKind, Doc, Node};

/// `set_heading(level)` — TipTap `toggleHeading`: all touched lines get the
/// level; if every touched line already has it, strip to paragraph.
pub fn toggle_heading(src: &str, sel: Selection, level: u8) -> Tx {
    let level = level.clamp(1, 6);
    let sel = Selection::new(
        crate::cmd::util::snap_down(src, sel.start()),
        crate::cmd::util::snap_up(src, sel.end()),
    );
    let lines = lines_of(src, sel.start(), sel.end());
    let all_at_level = lines.iter().all(|l| {
        heading_marker(src, l.start, l.end)
            .map(|(lv, _)| lv == level)
            .unwrap_or(false)
    });
    let mut tx = Tx::new();
    for l in &lines {
        match heading_marker(src, l.start, l.end) {
            Some((_, m_end)) if !all_at_level => {
                tx.replace(l.start, m_end, format!("{} ", "#".repeat(level as usize)));
            }
            Some((_, m_end)) => {
                tx.replace(l.start, m_end, "");
            }
            None => {
                tx.insert(l.start, format!("{} ", "#".repeat(level as usize)));
            }
        }
    }
    // Caret lands at the end of the last touched line's content.
    let delta: isize = tx
        .ops
        .iter()
        .filter(|(s, _, _)| *s <= sel.end())
        .map(|(s, e, t)| t.len() as isize - (e - s) as isize)
        .sum();
    let caret = (sel.end() as isize + delta).max(0) as usize;
    tx.selection(Selection::caret(caret));
    tx
}

/// `toggle_list(kind)` — strip markers when all touched list lines already
/// have the kind; otherwise (re)mark every line.
pub fn toggle_list(src: &str, sel: Selection, kind: ListKind) -> Tx {
    let sel = Selection::new(
        crate::cmd::util::snap_down(src, sel.start()),
        crate::cmd::util::snap_up(src, sel.end()),
    );
    let lines = lines_of(src, sel.start(), sel.end());
    let mut tx = Tx::new();
    let all_same = lines.iter().all(|l| {
        let Some(m) = list_marker(src, l.start, l.end) else {
            return false;
        };
        match kind {
            ListKind::Bullet => m.task.is_none() && m.num.is_none(),
            ListKind::Ordered => m.num.is_some(),
            ListKind::Task => m.task.is_some(),
        }
    });
    let mut counters: Vec<(usize, u64)> = Vec::new(); // indent_width -> next num
    for l in &lines {
        let cur = list_marker(src, l.start, l.end);
        if all_same {
            if let Some(m) = cur {
                tx.replace(m.marker.start, m.content, "");
            }
            continue;
        }
        let indent_w = cur
            .as_ref()
            .map(|m| m.indent.end - m.indent.start)
            .unwrap_or(0);
        if !all_same && src[l.start..l.end].trim().is_empty() {
            continue;
        }
        let repl = match kind {
            ListKind::Bullet => "- ".to_string(),
            ListKind::Ordered => {
                let n = counters
                    .iter_mut()
                    .find(|(w, _)| *w == indent_w)
                    .map(|(_, n)| {
                        let v = *n;
                        *n += 1;
                        v
                    })
                    .unwrap_or_else(|| {
                        let start = cur.as_ref().and_then(|m| m.num).unwrap_or(1);
                        counters.push((indent_w, start + 1));
                        start
                    });
                format!("{n}. ")
            }
            ListKind::Task => match cur.as_ref().and_then(|m| m.task.as_ref()) {
                Some((true, _)) => "- [x] ".to_string(),
                _ => "- [ ] ".to_string(),
            },
        };
        match cur {
            Some(m) => tx.replace(m.marker.start, m.content, repl),
            None => tx.replace(l.start, l.start, repl),
        };
    }
    tx.selection(Selection::caret(sel.end()));
    tx
}

/// Checkbox toggle on the caret's line — `- [ ]` ↔ `- [x]`.
pub fn toggle_task_checked(src: &str, sel: Selection) -> Tx {
    let mut tx = Tx::new();
    let l = line_range(src, crate::cmd::util::snap_up(src, sel.end()));
    if let Some(m) = list_marker(src, l.start, l.end) {
        if let Some((checked, tr)) = m.task {
            let b = tr.start + 1;
            tx.replace(b, b + 1, if checked { " " } else { "x" });
            tx.selection(Selection::caret(sel.end()));
        }
    }
    tx
}

/// `toggle_blockquote` — strip one `>` per line when all touched lines are
/// quoted; otherwise prefix `> `.
pub fn toggle_quote(src: &str, sel: Selection) -> Tx {
    let sel = Selection::new(
        crate::cmd::util::snap_down(src, sel.start()),
        crate::cmd::util::snap_up(src, sel.end()),
    );
    let lines = lines_of(src, sel.start(), sel.end());
    let non_blank: Vec<_> = lines
        .iter()
        .filter(|l| !src[l.start..l.end].trim().is_empty())
        .collect();
    let all_quoted = !non_blank.is_empty()
        && non_blank
            .iter()
            .all(|l| quote_marker(src, l.start, l.end).is_some());
    let mut tx = Tx::new();
    for l in &lines {
        if all_quoted {
            if let Some(end) = quote_marker(src, l.start, l.end) {
                tx.replace(l.start, end, "");
            }
        } else if src[l.start..l.end].trim().is_empty() {
            tx.insert(l.start, ">");
        } else {
            tx.insert(l.start, "> ");
        }
    }
    tx.selection(Selection::caret(sel.end()));
    tx
}

/// `toggle_code_block` — unwrap a fenced block containing the caret, or wrap
/// the touched lines in ```` ```lang ```` fences.
pub fn toggle_code_block(doc: &Doc, src: &str, sel: Selection, lang: &str) -> Tx {
    let sel = Selection::new(
        crate::cmd::util::snap_down(src, sel.start()),
        crate::cmd::util::snap_up(src, sel.end()),
    );
    let mut tx = Tx::new();
    if let Some(r) = fenced_block_containing(doc, sel.start()) {
        // Drop the fence lines; keep the content.
        let lines = lines_of(src, r.start, r.end);
        if let (Some(first), Some(last)) = (lines.first(), lines.last()) {
            if first.end < last.start {
                // >=2 fence lines: delete closing line incl. its newline, then
                // the opening line.
                let mut ls = last.start;
                if ls > 0 && src.as_bytes()[ls - 1] == b'\n' {
                    ls -= 1; // take the newline before it
                }
                tx.replace(ls, last.end, "");
                tx.replace(first.start, first.end + newline_len(src, first.end), "");
            } else {
                tx.replace(r.start, r.end, "");
            }
        }
        tx.selection(Selection::caret(r.start.min(src.len())));
        return tx;
    }
    // Wrap the touched lines.
    let lines = lines_of(src, sel.start(), sel.end());
    let first = lines.first().cloned().unwrap();
    let last = lines.last().cloned().unwrap();
    let open = format!("```{lang}\n");
    let close = "\n```";
    let open_len = open.len();
    tx.insert(last.end, close);
    tx.insert(first.start, open);
    tx.selection(Selection::caret(first.start + open_len));
    tx
}

/// `insert_hr` — `---` on its own line, split apart from surrounding text so
/// it can't parse as a setext heading.
pub fn insert_rule(src: &str, sel: Selection) -> Tx {
    let (s, e) = (
        crate::cmd::util::snap_down(src, sel.start()),
        crate::cmd::util::snap_up(src, sel.end()),
    );
    let l0 = line_range(src, s);
    let l1 = line_range(src, e.saturating_sub(1).max(s));
    let mut tx = Tx::new();
    if l0 == l1 && src[l0.clone()].trim().is_empty() {
        // Empty line: drop the rule there, caret after it.
        tx.replace(l0.start, l0.end, "---");
        tx.selection(Selection::caret(l0.start + 3));
        return tx;
    }
    // Split the touched lines: `ab|cd` → `ab\n\n---\n\ncd`.
    let before = src[l0.start..s].trim_end().to_string();
    let after = src[e.min(l1.end)..l1.end].trim_start().to_string();
    let mut text = String::new();
    if !before.is_empty() {
        text.push_str(&before);
        text.push_str("\n\n");
    }
    text.push_str("---");
    let caret = text.len();
    if !after.is_empty() {
        text.push_str("\n\n");
        text.push_str(&after);
    }
    tx.replace(l0.start, l1.end, &text);
    tx.selection(Selection::caret(l0.start + caret));
    tx
}

fn newline_len(src: &str, pos: usize) -> usize {
    match src.as_bytes().get(pos) {
        Some(b'\r') => 2,
        Some(b'\n') => 1,
        _ => 0,
    }
}

/// The fenced code block containing `pos`, if any.
pub fn fenced_block_containing(doc: &Doc, pos: usize) -> Option<crate::md::ast::RangeB> {
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
