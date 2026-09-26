//! Per-code-block tree-sitter highlighting (Vue `shikiHighlight.ts` parity).
//!
//! One `SyntaxHighlighter` per `(lang, code-text-hash)` — a block's text is
//! re-parsed only when its bytes change. Styled ranges are cached next to the
//! highlighter and returned relative to the block content start so the row
//! builder can merge them with projection marks.
//!
//! Vue uses Shiki `github-dark-default`; here `HighlightTheme::default_dark()`
//! is the equivalent palette (Documented in DESIGN.md token map).
//!
//! Vue cap: `MAX_HIGHLIGHT_SIZE = 24_000` chars — over that the block renders
//! plain (same as Vue).

use gpui::HighlightStyle;
use gpui_component::highlighter::{HighlightTheme, SyntaxHighlighter};
use ropey::Rope;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

/// Vue `MAX_HIGHLIGHT_SIZE` — don't parse code blocks larger than this.
pub const MAX_HIGHLIGHT_LEN: usize = 24_000;

/// Highlighters are only consulted on the UI thread; keep the cache off the
/// entity so `MemoriaEditor` stays free of non-Send interior state.
pub struct HighlightCache {
    theme: Arc<HighlightTheme>,
    /// (lang, content hash) → highlighter + styled ranges (block-local bytes).
    blocks: HashMap<(SharedStringKey, u64), BlockHigh>,
}

type SharedStringKey = gpui::SharedString;

struct BlockHigh {
    styles: Vec<(Range<usize>, HighlightStyle)>,
}

impl Default for HighlightCache {
    fn default() -> Self {
        Self::new()
    }
}

impl HighlightCache {
    pub fn new() -> Self {
        crate::languages::init_languages();
        Self {
            theme: HighlightTheme::default_dark(),
            blocks: HashMap::new(),
        }
    }

    /// Styled ranges for `code` (block content only — no fences) in `lang`.
    /// Returns `None` for over-size or unregistered code (plain fallback).
    pub fn styles_for(
        &mut self,
        lang: &str,
        code: &str,
    ) -> Option<&Vec<(Range<usize>, HighlightStyle)>> {
        if code.len() > MAX_HIGHLIGHT_LEN {
            return None;
        }
        let reg_name = crate::languages::registry_name(lang)
            .unwrap_or("")
            .to_string();
        // Unknown fence lang: try the literal name (registry may still know
        // it, e.g. `c++`); else plain.
        let name = if reg_name.is_empty() {
            lang.trim().to_lowercase()
        } else {
            reg_name
        };
        let key = (gpui::SharedString::from(name.clone()), hash(code));
        if !self.blocks.contains_key(&key) {
            let mut hl = SyntaxHighlighter::new(&name);
            let rope = Rope::from_str(code);
            hl.update(None, &rope, Some(Duration::from_millis(20)));
            let styles = hl.styles(&(0..code.len()), self.theme.as_ref());
            self.blocks.insert(key.clone(), BlockHigh { styles });
        }
        // Bound the cache — 64 blocks is plenty for a note.
        if self.blocks.len() > 64 {
            self.blocks.clear();
            return None;
        }
        self.blocks.get(&key).map(|b| &b.styles)
    }
}

fn hash(s: &str) -> u64 {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}
