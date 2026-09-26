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
pub use crate::tx::Tx;

/// Undo grouping pause — matches ProseMirror's default `newGroupDelay`.
pub const UNDO_PAUSE: Duration = Duration::from_millis(500);

pub struct Editor {
    pub(crate) buf: Buffer,
    pub(crate) sel: Selection,
    doc: Option<Doc>,
    /// Content revision — bumps on every buffer change (edit/undo/redo).
    /// Renderers cache per-revision.
    rev: u64,
    /// Projection cache keyed by (revision, selection): the renderer calls
    /// this every frame; unchanged inputs return the stored projection.
    proj_cache: Option<(u64, Selection, crate::Projection)>,
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
            rev: 0,
            proj_cache: None,
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
        let sel = self.sel;
        let doc = self.doc();
        project(doc, &src, sel)
    }

    /// Cached projection for the render loop — reparses/reprojects only when
    /// the buffer revision or the selection changed since the last call.
    /// Callers may hold the `&Projection` only as long as the borrow allows;
    /// copy out what a frame needs.
    pub fn project_cached(&mut self) -> &Projection {
        let hit = self
            .proj_cache
            .as_ref()
            .is_some_and(|(rev, sel, _)| *rev == self.rev && *sel == self.sel);
        if !hit {
            let src = self.buf.text();
            let sel = self.sel;
            let proj = {
                let doc = self.doc();
                project(doc, &src, sel)
            };
            self.proj_cache = Some((self.rev, sel, proj));
        }
        &self.proj_cache.as_ref().unwrap().2
    }

    /// Buffer revision — increments on every content mutation. Renderers key
    /// shaped-line/highlighter caches on this.
    pub fn revision(&self) -> u64 {
        self.rev
    }

    /// Cached projection + the cached parse in one borrow — the renderer's
    /// per-frame entry point.
    pub fn project_and_doc(&mut self) -> (&Projection, &Doc) {
        let hit = self
            .proj_cache
            .as_ref()
            .is_some_and(|(rev, sel, _)| *rev == self.rev && *sel == self.sel);
        if !hit {
            if self.doc.is_none() {
                self.doc = Some(parse(&self.buf.text()));
            }
            let src = self.buf.text();
            let proj = project(self.doc.as_ref().unwrap(), &src, self.sel);
            self.proj_cache = Some((self.rev, self.sel, proj));
        }
        (
            &self.proj_cache.as_ref().unwrap().2,
            self.doc.as_ref().unwrap(),
        )
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
        self.rev += 1;
        self.proj_cache = None;
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
        // A command that produced no patches and no caret move is a true
        // no-op — don't give undo an empty entry to "restore".
        if !patches.is_empty() || sel_after != sel_before {
            self.history
                .record(patches, sel_before, sel_after, kind, now);
        }
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
        self.rev += 1;
        self.proj_cache = None;
        self.marked = None;
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
        self.rev += 1;
        self.proj_cache = None;
        self.marked = None;
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

    /// Drop undo/redo entirely — used when the whole document is swapped
    /// out (note switch, live refresh), so a later `undo` can't resurrect
    /// the previous document's text into the new one.
    pub fn reset_history(&mut self) {
        self.history = History::new(UNDO_PAUSE);
        self.marked = None;
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
