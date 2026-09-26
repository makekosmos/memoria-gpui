//! Command surface — TipTap StarterKit + TaskList parity.

pub mod block;
pub mod inline;
pub mod insert;
pub mod util;

use crate::cursor::Selection;
use crate::editor::{Editor, Tx};
use crate::history::EditKind;

pub use block::{
    set_code_block_lang, toggle_code_block, toggle_heading, toggle_list, toggle_quote,
    toggle_task_checked,
};
pub use inline::{toggle_inline, Inline};
pub use insert::{
    display_image_src, insert_image, insert_link, insert_local_image, local_image_url,
};
pub use util::ListKind;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Bold,
    Italic,
    Strike,
    InlineCode,
    Heading(u8),
    List(ListKind),
    TaskToggle,
    Quote,
    CodeBlock(String),
    /// Language picker: rewrite the fence info string of the block at caret.
    SetCodeLang {
        lang: String,
    },
    Rule,
    Link {
        dest: String,
        title: Option<String>,
    },
    Image {
        src: String,
        alt: String,
        title: Option<String>,
    },
    LocalImage {
        path: String,
        alt: String,
    },
}

/// Build the transaction for `cmd` against `src`/`sel`.
pub fn tx_for(doc: &crate::md::ast::Doc, src: &str, sel: Selection, cmd: &Command) -> Tx {
    match cmd {
        Command::Bold => toggle_inline(src, sel, Inline::Bold),
        Command::Italic => toggle_inline(src, sel, Inline::Italic),
        Command::Strike => toggle_inline(src, sel, Inline::Strike),
        Command::InlineCode => toggle_inline(src, sel, Inline::Code),
        Command::Heading(l) => toggle_heading(src, sel, *l),
        Command::List(k) => toggle_list(src, sel, *k),
        Command::TaskToggle => toggle_task_checked(src, sel),
        Command::Quote => toggle_quote(src, sel),
        Command::CodeBlock(lang) => toggle_code_block(doc, src, sel, lang),
        Command::SetCodeLang { lang } => block::set_code_block_lang(doc, src, sel.start(), lang),
        Command::Rule => block::insert_rule(src, sel),
        Command::Link { dest, title } => insert_link(src, sel, dest, title.as_deref()),
        Command::Image { src: s, alt, title } => insert_image(src, sel, s, alt, title.as_deref()),
        Command::LocalImage { path, alt } => insert_local_image(src, sel, path, alt),
    }
}

impl Editor {
    /// Run a command (never merges into an undo group).
    pub fn command(&mut self, cmd: &Command) {
        let (src, sel) = (self.text(), self.selection());
        let doc = crate::md::parse::parse(&src);
        let tx = tx_for(&doc, &src, sel, cmd);
        self.apply(tx, EditKind::Command);
    }
}
