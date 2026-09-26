//! Minimal HTML → Markdown for paste. Covers browser/Word/Obsidian clips:
//! headings, p, br, hr, strong/em/del/code, pre, blockquote, ul/ol/li
//! (+ `input[type=checkbox]`), a, img, table, entity decoding.
//! Unknown tags pass content through; script/style/head content dropped.

mod render;
mod tok;

pub use render::html_to_markdown;
