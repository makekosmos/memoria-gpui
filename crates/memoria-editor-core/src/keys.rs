//! Key behavior — Enter / Backspace / Tab / Shift+Tab matching TipTap
//! semantics in lists, quotes, headings and code blocks.

use crate::cmd::block::fenced_block_containing;
use crate::cmd::util::{heading_marker, line_range, list_marker, quote_marker};
use crate::cursor::Selection;
use crate::editor::{Editor, Tx};
use crate::history::EditKind;

/// Code-block Tab width — `enableTabIndentation: true, tabSize: 2` in the
/// Vue editor (`CODE_BLOCK_TAB_SIZE`).
const CODE_INDENT: &str = "  ";

impl Editor {
    /// Enter key. In lists/quotes/headings/code blocks behaves like TipTap;
    /// elsewhere inserts one `\n` (markdown source semantics: a paragraph
    /// boundary needs the blank line, which the projection collapses anyway).
    pub fn key_enter(&mut self) {
        let sel = self.selection();
        let src = self.text();
        let (s, e) = (sel.start(), sel.end());
        let mut tx = Tx::new();
        if s != e {
            tx.replace(s, e, "");
        }
        let (ls, le) = {
            let r = line_range(&src, s);
            (r.start, r.end)
        };

        // Code block: newline + current indent (+2 after `{[(:`).
        // Only when the caret is past the opening fence line.
        if let Some(br) = fenced_block_containing(&crate::md::parse::parse(&src), s) {
            let content_start = src[br.start..br.end.min(src.len())]
                .find('\n')
                .map(|o| br.start + o + 1)
                .unwrap_or(br.end);
            if s >= content_start && s < br.end {
                let line = &src[ls..s.min(le)];
                let indent: String = line
                    .chars()
                    .take_while(|c| *c == ' ' || *c == '\t')
                    .collect();
                let extra = if line.trim_end().ends_with(['{', '[', '(', ':']) {
                    CODE_INDENT
                } else {
                    ""
                };
                let ins = format!("\n{indent}{extra}");
                tx.insert(s, &ins);
                tx.selection(Selection::caret(s + ins.len()));
                self.apply(tx, EditKind::Insert);
                return;
            }
        }

        // List item: continue marker, or lift when the item is empty.
        if let Some(m) = list_marker(&src, ls, le) {
            if src[m.content..le].trim().is_empty() && s >= m.content {
                // `- |` → plain empty line.
                tx.replace(m.indent.start, m.content, "");
                tx.selection(Selection::caret(m.indent.start));
            } else {
                let indent = &src[m.indent.clone()];
                let next = if m.task.is_some() {
                    "- [ ] ".to_string()
                } else if let Some(n) = m.num {
                    format!("{}. ", n + 1)
                } else {
                    src[m.marker.clone()]
                        .chars()
                        .next()
                        .map(|c| format!("{c} "))
                        .unwrap_or_else(|| "- ".into())
                };
                let ins = format!("\n{indent}{next}");
                tx.insert(s, &ins);
                tx.selection(Selection::caret(s + ins.len()));
            }
            self.apply(tx, EditKind::Insert);
            return;
        }

        // Blockquote: `> ` continues (ProseMirror `splitBlock` semantics;
        // an empty `> ` line keeps quoting — Backspace lifts out).
        if let Some(qe) = quote_marker(&src, ls, le) {
            let marker = &src[ls..qe];
            let ins = format!("\n{marker}");
            tx.insert(s, &ins);
            tx.selection(Selection::caret(s + ins.len()));
            self.apply(tx, EditKind::Insert);
            return;
        }

        // Heading: mid-heading splits keeping the level; at end → paragraph.
        if let Some((level, _)) = heading_marker(&src, ls, le) {
            if s < le {
                let prefix = format!("{} ", "#".repeat(level as usize));
                let ins = format!("\n{prefix}");
                tx.insert(s, &ins);
                tx.selection(Selection::caret(s + ins.len()));
            } else {
                tx.insert(s, "\n");
                tx.selection(Selection::caret(s + 1));
            }
            self.apply(tx, EditKind::Insert);
            return;
        }

        tx.insert(s, "\n");
        tx.selection(Selection::caret(s + 1));
        self.apply(tx, EditKind::Insert);
    }

    /// Backspace. Marker-aware at line starts (TipTap lifts/demotes); else
    /// deletes the previous grapheme cluster.
    pub fn key_backspace(&mut self) {
        let sel = self.selection();
        let src = self.text();
        let (s, e) = (sel.start(), sel.end());
        let mut tx = Tx::new();
        if s != e {
            tx.replace(s, e, "");
            tx.selection(Selection::caret(s));
            self.apply(tx, EditKind::Delete);
            return;
        }
        if s == 0 {
            return;
        }
        let (ls, le) = {
            let r = line_range(&src, s);
            (r.start, r.end)
        };
        // Caret inside/just after a line marker → remove the innermost one
        // (TipTap lifts one level per Backspace; `> - x` → `> x`).
        for (ms, me) in marker_span(&src, ls, le) {
            if s > ms && s <= me {
                tx.replace(ms, me, "");
                tx.selection(Selection::caret(ms));
                self.apply(tx, EditKind::Delete);
                return;
            }
        }
        let prev = self.buf.prev_grapheme(s);
        tx.replace(prev, s, "");
        tx.selection(Selection::caret(prev));
        self.apply(tx, EditKind::Delete);
    }

    /// Tab: indent list items / code lines (2 spaces, TipTap `tabSize: 2`);
    /// otherwise inserts a literal tab.
    pub fn key_tab(&mut self) {
        let sel = self.selection();
        let src = self.text();
        let (s, e) = (sel.start(), sel.end());
        let (ls, le) = {
            let r = line_range(&src, s);
            (r.start, r.end)
        };
        let in_code = fenced_block_containing(&crate::md::parse::parse(&src), s).is_some();
        let in_list = list_marker(&src, ls, le).is_some();
        let mut tx = Tx::new();
        if in_list {
            tx.insert(ls, CODE_INDENT);
            tx.selection(Selection::caret(s + CODE_INDENT.len()));
        } else if in_code {
            tx.replace(s, e, CODE_INDENT);
            tx.selection(Selection::caret(s + CODE_INDENT.len()));
        } else {
            tx.replace(s, e, "\t");
            tx.selection(Selection::caret(s + 1));
        }
        self.apply(tx, EditKind::Insert);
    }

    /// Shift+Tab: dedent list items / code lines (up to 2 spaces).
    pub fn key_shift_tab(&mut self) {
        let sel = self.selection();
        let src = self.text();
        let s = sel.start();
        let (ls, le) = {
            let r = line_range(&src, s);
            (r.start, r.end)
        };
        let in_code = fenced_block_containing(&crate::md::parse::parse(&src), s).is_some();
        let in_list = list_marker(&src, ls, le).is_some();
        if !in_code && !in_list {
            return;
        }
        // Remove up to CODE_INDENT leading whitespace.
        let mut e = ls;
        let b = src.as_bytes();
        let mut n = 0;
        while e < le && n < CODE_INDENT.len() && b[e] == b' ' {
            e += 1;
            n += 1;
        }
        if e == ls {
            return;
        }
        let mut tx = Tx::new();
        tx.replace(ls, e, "");
        let caret = s.saturating_sub(e - ls).max(ls);
        tx.selection(Selection::caret(caret));
        self.apply(tx, EditKind::Delete);
    }

    /// Shift+Enter — hard break, serialized `\` + `\n` like Vue.
    pub fn key_shift_enter(&mut self) {
        let sel = self.selection();
        let (s, e) = (sel.start(), sel.end());
        let mut tx = Tx::new();
        tx.replace(s, e, "\\\n");
        tx.selection(Selection::caret(s + 2));
        self.apply(tx, EditKind::Insert);
    }
}

/// Marker spans on the line, outermost→innermost (`> ` then `- ` then `# `).
/// Backspace removes the innermost span containing the caret.
fn marker_span(src: &str, ls: usize, le: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut p = ls;
    loop {
        if let Some(qe) = quote_marker(src, p, le) {
            out.push((p, qe));
            p = qe;
            continue;
        }
        if let Some(m) = list_marker(src, p, le) {
            out.push((p, m.content));
            break;
        }
        if let Some((_, me)) = heading_marker(src, p, le) {
            out.push((p, me));
            break;
        }
        break;
    }
    out
}
