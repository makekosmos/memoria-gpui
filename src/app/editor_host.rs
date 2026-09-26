//! M3 editor host — lazy `MemoriaEditor` / title `InputState`, save + autosave.
use gpui::{Context, Entity, Window};
use gpui_component::input::{InputEvent, InputState};
use memoria_editor_gpui::{EditorEvent, MemoriaEditor};
use memoria_gpui::content;
use memoria_gpui::store::Command;

use super::Memoria;

impl Memoria {
    pub(crate) fn title_state(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(state) = &self.title_input {
            return state.clone();
        }
        let state = cx.new(|cx| InputState::new(window, cx).placeholder("Название"));
        self._subs
            .push(cx.subscribe(&state, |this, _, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.dirty = true;
                    cx.notify();
                }
            }));
        self.title_input = Some(state.clone());
        state
    }

    /// Lazily create the live-preview editor entity. Autosave/zen/zoom
    /// events map onto the shell chrome (sidebar hide, Engine save).
    pub(crate) fn editor_state(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<MemoriaEditor> {
        if let Some(editor) = &self.editor {
            return editor.clone();
        }
        let editor = cx.new(|cx| {
            let mut e = MemoriaEditor::new("", window, cx);
            e.image_root = std::env::var("MEMORIA_IMAGES_DIR").ok().map(Into::into);
            e
        });
        self._subs.push(
            cx.subscribe(&editor, |this, _, ev: &EditorEvent, cx| match ev {
                EditorEvent::Edited => {
                    this.dirty = true;
                    cx.notify();
                }
                EditorEvent::Autosave(markdown) => this.autosave(markdown, cx),
                EditorEvent::ZenToggled => {
                    this.zen = !this.zen;
                    cx.notify();
                }
                EditorEvent::ZoomChanged(_)
                | EditorEvent::SelectionChanged
                | EditorEvent::CtrlK => {}
            }),
        );
        self.editor = Some(editor.clone());
        editor
    }

    pub(crate) fn save(&mut self, cx: &mut Context<Self>) {
        let Some(entry) = self.current.clone() else {
            return;
        };
        let title = self
            .title_input
            .as_ref()
            .map(|s| s.read(cx).value().to_string())
            .unwrap_or_else(|| entry.title.clone());
        let markdown = self
            .editor
            .as_ref()
            .map(|e| e.read(cx).markdown().to_string())
            .unwrap_or_default();
        self.send_save(&title, &markdown, cx);
    }

    /// `saveEntry` body — shared by the manual button and editor autosave.
    pub(crate) fn send_save(&mut self, title: &str, markdown: &str, cx: &mut Context<Self>) {
        let Some(entry) = self.current.clone() else {
            return;
        };
        let mut next = entry;
        next.title = title.to_string();
        next.content_json = content::write_entry_markdown(markdown).to_string();
        next.content_loaded = Some(true);
        self.send(Command::SaveEntry(Box::new(next)), cx);
    }

    /// `EditorEvent::Autosave` — the 300ms debounce lives in the editor
    /// entity (Vue `editor.viewUpdateListener`), the save itself is ours.
    pub(crate) fn autosave(&mut self, markdown: &str, cx: &mut Context<Self>) {
        let Some(entry) = self.current.clone() else {
            return;
        };
        let title = self
            .title_input
            .as_ref()
            .map(|s| s.read(cx).value().to_string())
            .unwrap_or_else(|| entry.title.clone());
        self.send_save(&title, markdown, cx);
    }
}
