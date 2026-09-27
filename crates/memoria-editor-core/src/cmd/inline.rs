//! Inline mark toggles: bold / italic / strike / code.
//!
//! TipTap parity: toggling over a fully-marked range removes the marks;
//! toggling inside a marked run splits it (`**aXcd**` sel=`Xc` →
//! `**a**Xc**d**` — selecting `Xc` removes bold from it); otherwise wraps the
//! selection.

use crate::cursor::Selection;
use crate::editor::Tx;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inline {
    Bold,
    Italic,
    Strike,
    Code,
}

impl Inline {
    /// Delimiters we strip when un-marking (longest first within a kind).
    fn delims(self) -> &'static [&'static str] {
        match self {
            Inline::Bold => &["**", "__"],
            Inline::Italic => &["*", "_"],
            Inline::Strike => &["~~"],
            Inline::Code => &["```", "``", "`"],
        }
    }

    /// Preferred delimiter when wrapping.
    fn wrap_delim(self, content: &str) -> String {
        match self {
            Inline::Code => {
                let mut max_run = 0usize;
                let mut run = 0usize;
                for b in content.bytes() {
                    run = if b == b'`' { run + 1 } else { 0 };
                    max_run = max_run.max(run);
                }
                "`".repeat(max_run + 1)
            }
            Inline::Bold => "**".into(),
            Inline::Italic => "*".into(),
            Inline::Strike => "~~".into(),
        }
    }
}

/// Toggle `kind` over `sel` in `src`; the transaction's selection is in
/// post-edit text coordinates.
pub fn toggle_inline(src: &str, sel: Selection, kind: Inline) -> Tx {
    let (s, e) = (
        crate::cmd::util::snap_down(src, sel.start()),
        crate::cmd::util::snap_up(src, sel.end()),
    );
    let mut tx = Tx::new();

    // Case 1: matching delimiters hug the selection → strip them.
    // `**aXcd**` sel=`aXcd` → `aXcd`; sel=`Xc` inside stays a wrap (split).
    if let Some(d) = surrounding_delim(src, s, e, kind) {
        let n = d.len();
        tx.replace(e, e + n, "");
        tx.replace(s - n, s, "");
        tx.selection(Selection::new(s - n, e - n));
        return tx;
    }

    // Case 2: selection covers `D content D` including the delimiters.
    for d in kind.delims() {
        let seg = &src[s..e];
        let n = d.len();
        if seg.len() >= 2 * n && seg.starts_with(d) && seg.ends_with(d) && edge_runs_odd(seg, d) {
            tx.replace(e - n, e, "");
            tx.replace(s, s + n, "");
            tx.selection(Selection::new(s, e - 2 * n));
            return tx;
        }
    }

    // Case 3 (default): wrap — inserting a close+open pair inside a marked
    // run also produces the correct split (`**a` `**Xc**` `d**`).
    let d = kind.wrap_delim(&src[s.min(e)..e.min(src.len())]);
    tx.insert(e, d.clone());
    tx.insert(s, d.clone());
    if s == e {
        tx.selection(Selection::caret(s + d.len()));
    } else {
        tx.selection(Selection::new(s + d.len(), e + d.len()));
    }
    tx
}

/// The delimiter run immediately hugging `s..e` (`src[..s]` ends with it and
/// `src[e..]` starts with the same run), if any.
fn surrounding_delim(src: &str, s: usize, e: usize, kind: Inline) -> Option<String> {
    if s == 0 || e >= src.len() {
        return None;
    }
    if kind == Inline::Code {
        let l = run_of(src, s, b'`', true);
        let r = run_of(src, e, b'`', false);
        return (l > 0 && l == r).then(|| "`".repeat(l));
    }
    for d in kind.delims() {
        let n = d.len();
        if s < n || e + n > src.len() {
            continue;
        }
        if src.is_char_boundary(s - n)
            && src.is_char_boundary(e + n)
            && src[s - n..s] == **d
            && src[e..e + n] == **d
            && delim_fits(kind, src, s, e, n)
        {
            return Some(d.to_string());
        }
    }
    None
}

/// For single-char `*`/`_` delimiters, both edge runs inside the selection
/// must be odd-length to count as emphasis: `*x*` (1) and `***x***` (3)
/// carry an italic layer that unwrapping removes; `**x**` (2) is a strong
/// pair — stripping one `*` would silently turn bold into italic, so the
/// toggle must wrap instead (`***x***`). Multi-char delimiters always fit.
fn edge_runs_odd(seg: &str, d: &str) -> bool {
    if d.len() != 1 {
        return true;
    }
    let c = d.as_bytes()[0] as char;
    seg.chars().take_while(|&ch| ch == c).count() % 2 == 1
        && seg.chars().rev().take_while(|&ch| ch == c).count() % 2 == 1
}

/// For italic single-char delimiters, the matched `*`/`_` must not be part
/// of a longer `**`/`__` run (that's a strong delimiter, not emphasis).
fn delim_fits(kind: Inline, src: &str, s: usize, e: usize, n: usize) -> bool {
    if kind != Inline::Italic {
        return true;
    }
    let b = src.as_bytes();
    let dc = b[s - n]; // the delimiter char itself
    let left_ext = s > n && b[s - n - 1] == dc;
    let right_ext = e + n < b.len() && b[e + n] == dc;
    !left_ext && !right_ext
}

/// Run length of `byte` at `pos` looking backward (`before`) or forward.
fn run_of(src: &str, pos: usize, byte: u8, before: bool) -> usize {
    let b = src.as_bytes();
    let mut n = 0;
    if before {
        while n < pos && b[pos - 1 - n] == byte {
            n += 1;
        }
    } else {
        while pos + n < b.len() && b[pos + n] == byte {
            n += 1;
        }
    }
    n
}
