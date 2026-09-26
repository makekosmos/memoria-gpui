//! `memoria-editor-gpui` — the GPUI note editor built on `memoria-editor-core`
//! (KOS-150, M3). `MemoriaEditor` wraps the pure core: Live Preview rendering
//! of the projection, `EntityInputHandler` IME contract, tree-sitter code
//! highlighting, code-block language picker, local images, and the Vue
//! hotkey surface (physical keys, `Ctrl+K Z` zen chord, zoom).

pub mod highlight;
pub mod languages;

mod actions;
mod editor;
mod element;
mod geo;
mod images;
mod input;
mod layout;
mod lifecycle;
mod mouse;
mod paint;
mod picker;
mod rows;
mod runs;
mod style;
mod view;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_regressions;

pub use editor::{EditorEvent, MemoriaEditor, AUTOSAVE_DEBOUNCE, PLACEHOLDER};
pub use element::EditorElement;
pub use picker::LangPicker;
pub use style::EditorScale;
pub use view::{key_bindings, KEY_CONTEXT};
