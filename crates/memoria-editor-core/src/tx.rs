//! Transaction type — a batch of source edits plus the target selection.

use crate::cursor::Selection;

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
