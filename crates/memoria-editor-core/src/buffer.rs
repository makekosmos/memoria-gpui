//! Rope-backed text buffer addressed in UTF-8 byte offsets, with grapheme
//! cluster navigation via `unicode-segmentation`.

use ropey::Rope;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone)]
pub struct Buffer {
    rope: Rope,
}

impl Buffer {
    pub fn new() -> Self {
        Self { rope: Rope::new() }
    }

    pub fn from_text(s: &str) -> Self {
        Self {
            rope: Rope::from_str(s),
        }
    }

    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    pub fn is_empty(&self) -> bool {
        self.rope.len_bytes() == 0
    }

    /// Source slice as `String`. Fine at editor scale; used by parse/projection.
    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    pub fn slice(&self, start: usize, end: usize) -> String {
        if start >= end || start >= self.len_bytes() {
            return String::new();
        }
        let end = end.min(self.len_bytes());
        match self.rope.get_byte_slice(start..end) {
            Some(s) => s.to_string(),
            // Non-boundary request: round both ends inward to char boundaries.
            None => self
                .rope
                .get_slice(self.char_floor(start)..self.char_ceil(end))
                .map(|s| s.to_string())
                .unwrap_or_default(),
        }
    }

    fn char_floor(&self, byte: usize) -> usize {
        let c = self.rope.byte_to_char(byte);
        c + usize::from(self.rope.char_to_byte(c) < byte)
    }

    /// Char index for a slice *end*: `byte_to_char` already yields the first
    /// char at/after `byte`, i.e. a mid-char offset excludes the char.
    fn char_ceil(&self, byte: usize) -> usize {
        self.rope.byte_to_char(byte)
    }

    /// Replace `[start, end)` (bytes) with `text`; returns the removed text.
    /// Used by history to build inverse patches.
    pub fn replace(&mut self, start: usize, end: usize, text: &str) -> String {
        debug_assert!(start <= end && end <= self.len_bytes());
        debug_assert!(self.is_boundary(start) && self.is_boundary(end));
        let removed = self.slice(start, end);
        let cs = self.rope.byte_to_char(start);
        let ce = self.rope.byte_to_char(end);
        if cs != ce {
            self.rope.remove(cs..ce);
        }
        if !text.is_empty() {
            self.rope.insert(cs, text);
        }
        removed
    }

    pub fn insert(&mut self, pos: usize, text: &str) {
        debug_assert!(self.is_boundary(pos));
        self.rope
            .insert(self.rope.byte_to_char(pos.min(self.len_bytes())), text);
    }

    /// Greatest char boundary `<= byte`.
    pub fn snap_boundary(&self, byte: usize) -> usize {
        let byte = byte.min(self.len_bytes());
        self.rope.char_to_byte(self.rope.byte_to_char(byte))
    }

    /// `byte` sits on a char boundary (0 and len_bytes count as boundaries).
    pub fn is_boundary(&self, byte: usize) -> bool {
        if byte > self.len_bytes() {
            return false;
        }
        self.rope.char_to_byte(self.rope.byte_to_char(byte)) == byte
    }

    /// Line index (0-based) containing byte offset.
    pub fn byte_to_line(&self, byte: usize) -> usize {
        self.rope.byte_to_line(byte.min(self.len_bytes()))
    }

    /// Byte offset of a line's start (the `\n` belongs to the previous line).
    pub fn line_to_byte(&self, line: usize) -> usize {
        self.rope.line_to_byte(line.min(self.rope.len_lines()))
    }

    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// Byte range of the line containing `byte`, excluding the line terminator.
    pub fn line_range(&self, byte: usize) -> (usize, usize) {
        let line = self.byte_to_line(byte);
        let start = self.line_to_byte(line);
        let mut end = self.line_to_byte(line + 1);
        // strip trailing \n / \r\n
        let text = self.slice(start, end);
        let trimmed = text.trim_end_matches(['\n', '\r']);
        end = start + trimmed.len();
        (start, end)
    }

    /// Previous grapheme cluster boundary before `byte` (same line context).
    pub fn prev_grapheme(&self, byte: usize) -> usize {
        let byte = byte.min(self.len_bytes());
        if byte == 0 {
            return 0;
        }
        let (ls, le) = self.line_range(byte);
        if byte <= ls {
            // At a line start: step over the previous line's terminator —
            // exactly one (`\r\n` counts as a unit). Consuming a whole run
            // would make Backspace erase every blank line above at once.
            let tail = self.slice(byte.saturating_sub(4), byte);
            let back = match tail.as_bytes() {
                [.., b'\r', b'\n'] => 2,
                [.., b'\n'] | [.., b'\r'] => 1,
                // Unicode line separator (NEL/LS/PS/VT/FF) — step its width.
                _ => tail.chars().last().map_or(0, |c| c.len_utf8()),
            };
            return byte - back;
        }
        if byte > le {
            // inside/after line terminator: step over it as one unit
            return le;
        }
        let line = self.slice(ls, le);
        line.grapheme_indices(true)
            .map(|(i, _)| ls + i)
            .take_while(|&i| i < byte)
            .last()
            .unwrap_or(ls)
    }

    /// Next grapheme cluster boundary after `byte`.
    pub fn next_grapheme(&self, byte: usize) -> usize {
        let len = self.len_bytes();
        if byte >= len {
            return len;
        }
        let (ls, le) = self.line_range(byte);
        if byte >= le {
            // step over the line terminator as one unit
            let next = self.line_to_byte(self.byte_to_line(byte) + 1);
            return next.min(len);
        }
        let line = self.slice(ls, le);
        line.grapheme_indices(true)
            .map(|(i, _)| ls + i)
            .find(|&i| i > byte)
            .unwrap_or(le)
    }

    /// UTF-16 code units before `byte` — for the GPUI-shaped IME contract.
    pub fn byte_to_utf16(&self, byte: usize) -> usize {
        let byte = byte.min(self.len_bytes());
        let c = self.rope.byte_to_char(byte);
        self.rope.slice(..c).to_string().encode_utf16().count()
    }

    /// Byte offset at UTF-16 unit `units` (snaps to the char containing it).
    pub fn utf16_to_byte(&self, units: usize) -> usize {
        let mut acc = 0usize;
        let mut byte = 0usize;
        for c in self.rope.chars() {
            if acc + c.len_utf16() > units {
                break;
            }
            acc += c.len_utf16();
            byte += c.len_utf8();
        }
        byte.min(self.len_bytes())
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::str::FromStr for Buffer {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_text(s))
    }
}
