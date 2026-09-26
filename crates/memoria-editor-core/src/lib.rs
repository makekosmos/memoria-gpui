//! `memoria-editor-core` — Memoria's markdown editor core (M2).
//! Pure Rust, no GPUI: rope buffer, pulldown-cmark parse, Live Preview
//! projection, Tiptap-parity commands, undo/redo, paste, IME contract.

pub mod buffer;
pub mod charcount;
pub mod cmd;
pub mod cursor;
pub mod editor;
pub mod history;
pub mod html2md;
pub mod ime;
pub mod keys;
pub mod md;
pub mod paste;
pub mod project;

pub use buffer::Buffer;
pub use charcount::count_chars;
pub use cursor::Selection;
mod tx;
pub use editor::Editor;
pub use history::EditKind;
pub use project::{project, Marks, Projection, Span};
pub use tx::Tx;
