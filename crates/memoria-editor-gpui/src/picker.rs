//! Code-block language picker — the GPUI counterpart of `EdenCodeBlockTools`
//! («Поиск языка...»). Opens on `ctrl-shift-p`-style action or the app calling
//! `open_lang_picker`; while open it captures key input in capture phase
//! (letters extend the query, arrows move, Enter applies, Esc closes).

use gpui::Context;

use crate::editor::MemoriaEditor;
use crate::languages::filter_languages;

/// Picker state — open over the code block at `block_ix`.
pub struct LangPicker {
    /// Top-level block index the picker edits.
    pub block_ix: usize,
    /// Query text («Поиск языка...» placeholder when empty).
    pub query: String,
    /// Highlighted entry within `filtered()`.
    pub selected: usize,
}

impl LangPicker {
    pub fn new(block_ix: usize, current: &str) -> Self {
        Self {
            block_ix,
            query: current.to_string(),
            selected: 0,
        }
    }

    /// Languages matching `query` (case-insensitive substring over the name).
    pub fn filtered(&self) -> Vec<&'static crate::languages::Lang> {
        filter_languages(&self.query)
    }

    /// Currently highlighted language (None = clear the fence).
    pub fn highlighted(&self) -> Option<&'static str> {
        self.filtered().get(self.selected).map(|l| l.name)
    }
}

impl MemoriaEditor {
    /// Open the picker for the fenced code block at the caret.
    pub fn open_lang_picker(&mut self, cx: &mut Context<Self>) {
        self.ensure_rows();
        let doc = self.core.doc().clone();
        let head = self.core.selection().head;
        if let Some((ix, node)) = crate::rows::code_block_at(&doc, head) {
            let lang = match node {
                memoria_editor_core::md::ast::Node::Block {
                    kind: memoria_editor_core::md::ast::BlockKind::CodeBlock { lang, .. },
                    ..
                } => lang.clone().unwrap_or_default(),
                _ => String::new(),
            };
            self.picker = Some(LangPicker::new(ix, &lang));
            cx.notify();
        }
    }

    pub fn close_lang_picker(&mut self, cx: &mut Context<Self>) {
        if self.picker.take().is_some() {
            cx.notify();
        }
    }

    /// Picker key input (called from the capture-phase key listener).
    /// Returns true when the key was consumed.
    pub(crate) fn picker_key(
        &mut self,
        key: &str,
        key_char: Option<&str>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(p) = self.picker.as_mut() else {
            return false;
        };
        let count = p.filtered().len();
        match key {
            "escape" => {
                self.picker = None;
            }
            "enter" => {
                if let Some(lang) = p.highlighted().map(str::to_string) {
                    self.command(memoria_editor_core::cmd::Command::SetCodeLang { lang }, cx);
                }
                self.picker = None;
                return true;
            }
            "up" => {
                p.selected = p.selected.saturating_sub(1);
            }
            "down" => {
                p.selected = (p.selected + 1).min(count.saturating_sub(1));
            }
            "backspace" => {
                p.query.pop();
                p.selected = 0;
            }
            _ => {
                if let Some(ch) = key_char {
                    if !ch.is_empty() && ch.chars().all(|c| !c.is_control()) {
                        p.query.push_str(ch);
                        p.selected = 0;
                    }
                }
            }
        }
        cx.notify();
        true
    }
}
