//! `Editor` — buffer + selection + parse cache + history + IME state. All
//! positions are UTF-8 byte offsets into the markdown source unless a method
//! says `utf16`.

use std::time::{Duration, Instant};

use crate::buffer::Buffer;
use crate::cursor::Selection;
use crate::history::{EditKind, History, Patch};
use crate::md::ast::{Doc, RangeB};
use crate::md::parse;
use crate::project::project;
use crate::project::Projection;

/// Undo grouping pause — matches ProseMirror's default `newGroupDelay`.
pub const UNDO_PAUSE: Duration = Duration::from_millis(500);

/// A set of non-overlapping source edits + the selection after applying.
/// Ops may be given in any order; they're applied from right to left.
#[derive(Debug, Default)]
pub struct Tx {
    /// (start, end, replacement)
    pub ops: Vec<(usize, usize, String)>,
    pub sel: Option<Selection>,
}

impl Tx {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn replace(&mut self, start: usize, end: usize, text: impl Into<String>) -> &mut Self {
        self.ops.push((start, end, text.into()));
        self
    }

    pub fn insert(&mut self, pos: usize, text: impl Into<String>) -> &mut Self {
        self.replace(pos, pos, text)
    }

    pub fn selection(&mut self, sel: Selection) -> &mut Self {
        self.sel = Some(sel);
        self
    }
}

pub struct Editor {
    pub(crate) buf: Buffer,
    pub(crate) sel: Selection,
    doc: Option<Doc>,
    history: History,
    /// `Some` = manual clock (tests); `None` = wall clock.
    manual: Option<Instant>,
    /// IME marked-text (preedit) range, in source bytes.
    pub(crate) marked: Option<RangeB>,
}

impl Editor {
    pub fn new(src: &str) -> Self {
        Self {
            buf: Buffer::from_text(src),
            sel: Selection::caret(src.len()),
            doc: None,
            history: History::new(UNDO_PAUSE),
            manual: None,
            marked: None,
        }
    }

    // ---- read access -------------------------------------------------------

    pub fn len(&self) -> usize {
        self.buf.len_bytes()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn text(&self) -> String {
        self.buf.text()
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buf
    }

    pub fn selection(&self) -> Selection {
        self.sel
    }

    pub fn set_selection(&mut self, sel: Selection) {
        let len = self.buf.len_bytes();
        self.sel = Selection::new(
            self.buf.snap_boundary(sel.anchor.min(len)),
            self.buf.snap_boundary(sel.head.min(len)),
        );
    }

    pub fn marked_range(&self) -> Option<RangeB> {
        self.marked.clone()
    }

    pub(crate) fn set_marked(&mut self, r: Option<RangeB>) {
        self.marked = r;
    }

    /// Parsed tree (cached; invalidated by edits).
    pub fn doc(&mut self) -> &Doc {
        if self.doc.is_none() {
            self.doc = Some(parse(&self.buf.text()));
        }
        self.doc.as_ref().unwrap()
    }

    /// Live-preview projection under the current selection.
    pub fn project(&mut self) -> Projection {
        let src = self.buf.text();
        let doc = parse(&src);
        project(&doc, &src, self.sel)
    }

    /// Same, for an arbitrary selection — renderer/caret helpers.
    pub fn project_at(&self, sel: Selection) -> Projection {
        let src = self.buf.text();
        let doc = parse(&src);
        project(&doc, &src, sel)
    }

    /// Serialize = the source markdown, byte-identical (source of truth).
    pub fn serialize(&self) -> String {
        self.buf.text()
    }

    // ---- editing -----------------------------------------------------------

    /// Apply a transaction; records an undo entry. Returns new selection.
    pub fn apply(&mut self, mut tx: Tx, kind: EditKind) -> Selection {
        let sel_before = self.sel;
        let now = self.now();
        // Right-to-left so earlier offsets stay valid.
        tx.ops.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
        let mut patches = Vec::with_capacity(tx.ops.len());
        for (start, end, text) in tx.ops.drain(..) {
            let end = self.buf.snap_boundary(end.min(self.buf.len_bytes()));
            let start = self.buf.snap_boundary(start.min(end));
            let del = self.buf.replace(start, end, &text);
            patches.push(Patch {
                start,
                del,
                ins: text,
            });
        }
        self.doc = None;
        if kind != EditKind::Ime {
            self.marked = None;
        }
        let len = self.buf.len_bytes();
        let s = tx.sel.unwrap_or(sel_before);
        self.sel = Selection::new(
            self.buf.snap_boundary(s.anchor.min(len)),
            self.buf.snap_boundary(s.head.min(len)),
        );
        let sel_after = self.sel;
        self.history
            .record(patches, sel_before, sel_after, kind, now);
        self.sel
    }

    /// Type text at the caret (replacing a non-empty selection).
    pub fn insert_text(&mut self, text: &str) {
        let r = self.sel.range();
        let mut tx = Tx::new();
        tx.replace(r.start, r.end, text);
        let pos = r.start + text.len();
        tx.selection(Selection::caret(pos));
        self.apply(tx, EditKind::Insert);
    }

    pub fn undo(&mut self) -> bool {
        let Some(e) = self.history.pop_undo() else {
            return false;
        };
        for p in e.patches.iter().rev() {
            self.buf.replace(p.start, p.start + p.ins.len(), &p.del);
        }
        self.doc = None;
        self.sel = e.sel_before.clamp(self.buf.len_bytes());
        self.history.push_redo(e);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(e) = self.history.pop_redo() else {
            return false;
        };
        for p in e.patches.iter() {
            self.buf.replace(p.start, p.start + p.del.len(), &p.ins);
        }
        self.doc = None;
        self.sel = e.sel_after.clamp(self.buf.len_bytes());
        self.history.push_undo_direct(e);
        true
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Force the next edit to start a new undo group.
    pub fn break_undo_group(&mut self) {
        self.history.break_group();
    }

    // ---- clock -------------------------------------------------------------

    fn now(&self) -> Instant {
        self.manual.unwrap_or_else(Instant::now)
    }

    /// Switch to manual clock starting now (tests / deterministic grouping).
    pub fn manual_clock(&mut self) {
        self.manual = Some(Instant::now());
    }

    /// Advance the manual clock; a no-op on the wall clock.
    pub fn advance_time(&mut self, d: Duration) {
        if let Some(m) = &mut self.manual {
            *m += d;
        }
    }
}
