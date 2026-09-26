//! Undo/redo with input grouping: consecutive edits of the same kind merge
//! while the pause between them stays under `pause`. Each entry restores the
//! selection as it was before / after the grouped edits.

use std::time::{Duration, Instant};

use crate::cursor::Selection;

/// Why an edit happened — drives grouping (mirrors ProseMirror history's
/// `newGroupDelay` + adjacency semantics).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    /// Plain typing / single-char inserts (mergeable).
    Insert,
    /// Backspace/Delete (mergeable).
    Delete,
    /// IME marked-text update (mergeable).
    Ime,
    Paste,
    /// Explicit command (toggle, insert, key-command) — never merged.
    Command,
}

impl EditKind {
    fn mergeable(self) -> bool {
        matches!(self, Self::Insert | Self::Delete | Self::Ime)
    }
}

/// Inverse of one `replace(start..start+del.len() -> ins)` applied earlier.
#[derive(Debug, Clone)]
pub struct Patch {
    pub start: usize,
    /// Text that was removed (restored on undo).
    pub del: String,
    /// Text that was inserted (removed on undo).
    pub ins: String,
}

#[derive(Debug)]
pub struct Entry {
    pub patches: Vec<Patch>,
    pub sel_before: Selection,
    pub sel_after: Selection,
    kind: EditKind,
    at: Instant,
}

#[derive(Debug)]
pub struct History {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    pause: Duration,
    force_new: bool,
}

impl History {
    pub fn new(pause: Duration) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            pause,
            force_new: false,
        }
    }

    /// Record a finished edit. Consecutive mergeable same-kind edits within
    /// `pause` fold into the top entry.
    pub fn record(
        &mut self,
        patches: Vec<Patch>,
        sel_before: Selection,
        sel_after: Selection,
        kind: EditKind,
        now: Instant,
    ) {
        self.redo.clear();
        if let Some(top) = self.undo.last_mut() {
            if !self.force_new
                && kind.mergeable()
                && top.kind == kind
                && now.duration_since(top.at) <= self.pause
            {
                top.patches.extend(patches);
                top.sel_after = sel_after;
                top.at = now;
                return;
            }
        }
        self.undo.push(Entry {
            patches,
            sel_before,
            sel_after,
            kind,
            at: now,
        });
        self.force_new = false;
    }

    /// Explicit boundary: next edit starts a new group.
    pub fn break_group(&mut self) {
        self.force_new = true;
    }

    pub fn pop_undo(&mut self) -> Option<Entry> {
        let e = self.undo.pop()?;
        Some(e)
    }

    pub fn push_redo(&mut self, e: Entry) {
        self.redo.push(e);
    }

    pub fn pop_redo(&mut self) -> Option<Entry> {
        self.redo.pop()
    }

    pub fn push_undo_direct(&mut self, e: Entry) {
        self.undo.push(e);
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }
}
