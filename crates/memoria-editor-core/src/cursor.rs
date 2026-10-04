//! Cursor and selection in UTF-8 byte offsets into the markdown source.

/// A selection in source byte offsets. `anchor` is where the selection started,
/// `head` is the moving end (the caret). Collapsed selection = caret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Selection {
    pub anchor: usize,
    pub head: usize,
}

impl Selection {
    pub fn caret(pos: usize) -> Self {
        Self {
            anchor: pos,
            head: pos,
        }
    }

    pub fn new(anchor: usize, head: usize) -> Self {
        Self { anchor, head }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    pub fn start(&self) -> usize {
        self.anchor.min(self.head)
    }

    pub fn end(&self) -> usize {
        self.anchor.max(self.head)
    }

    pub fn range(&self) -> std::ops::Range<usize> {
        self.start()..self.end()
    }

    /// Obsidian-style "touching reveals": a collapsed caret counts as inside a
    /// construct when it sits on either boundary; a non-empty selection counts
    /// when it merely touches the construct range.
    pub fn touches(&self, start: usize, end: usize) -> bool {
        self.start() <= end && start <= self.end()
    }

    /// Clamp into a valid byte range.
    pub fn clamp(&self, len: usize) -> Self {
        Self {
            anchor: self.anchor.min(len),
            head: self.head.min(len),
        }
    }
}
