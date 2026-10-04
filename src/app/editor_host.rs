//! M3 editor host — per-entry `NoteDoc` documents shared between the main
//! window and sticker windows, plus the manual save path. The lazy
//! `MemoriaEditor` / title `InputState` live on the doc; `self.editor` /
//! `self.title_input` are per-frame mirrors of the *current* doc for the
//! existing render code.
use gpui::{prelude::*, Context, Entity, Window};
use memoria_editor_gpui::EditorEvent;
use memoria_model::content;
use memoria_model::store::Command;

use super::{DocEvent, Memoria, NoteDoc};

impl Memoria {
    /// Get-or-create the shared document for `entry_id`. Doc-initiated saves
    /// (editor autosave, title-draft debounce) arrive as `DocEvent::Save` —
    /// a queued effect, so demo-backend synchronous replies can't re-borrow
    /// the doc mid-update.
    pub(crate) fn ensure_doc(
        &mut self,
        entry_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<NoteDoc> {
        if let Some(doc) = self.docs.get(entry_id) {
            return doc.clone();
        }
        let doc = cx.new(|cx| NoteDoc::new(window, cx));
        self._subs
            .push(cx.subscribe(&doc, |this, _, ev: &DocEvent, cx| match ev {
                DocEvent::Save(entry) => this.send(Command::SaveEntry(entry.clone()), cx),
            }));
        // App-level editor events — zen chord + the dirty flag driving the
        // save button/status. `Autosave` is owned by the doc (its emit is
        // keyed to *this* entry — a shared buffer must never save through
        // `self.current`).
        let editor = doc.read(cx).editor.clone();
        self._subs.push(
            cx.subscribe(&editor, |this, _, ev: &EditorEvent, cx| match ev {
                EditorEvent::Edited => {
                    this.dirty = true;
                    cx.notify();
                }
                EditorEvent::ZenToggled => {
                    // Vue `useKeyboard`: zen only toggles while a note is
                    // open (`eden.currentEntry` guard).
                    if this.current.is_some() {
                        this.zen = !this.zen;
                    }
                    cx.notify();
                }
                EditorEvent::Autosave(_)
                | EditorEvent::ZoomChanged(_)
                | EditorEvent::SelectionChanged
                | EditorEvent::CtrlK
                | EditorEvent::Submit => {}
            }),
        );
        self.docs.insert(entry_id.to_string(), doc.clone());
        // Fresh docs always fetch — the sticker path can open a note the
        // list/nav flow never loaded (`content_loaded` previews skip the
        // nav-level request).
        self.send(
            Command::LoadEntry {
                id: entry_id.to_string(),
                content_only: false,
            },
            cx,
        );
        doc
    }

    /// The current entry's shared document (created on demand).
    fn current_doc(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<NoteDoc>> {
        let id = self.current.as_ref()?.id.clone();
        Some(self.ensure_doc(&id, window, cx))
    }

    /// Dirty flag of the current doc — `self.dirty` catches doc-less edits;
    /// once a doc exists it's the source of truth (title or buffer).
    pub(crate) fn current_doc_dirty(&self, cx: &gpui::App) -> bool {
        self.current
            .as_ref()
            .and_then(|e| self.docs.get(&e.id))
            .map(|d| d.read(cx).is_dirty(cx))
            .unwrap_or(self.dirty)
    }

    /// Sync `self.editor`/`self.title_input` with the current doc and push
    /// any pending title fill into this window's binding — called from
    /// `Render` because `set_value` needs a `Window`.
    pub(crate) fn sync_editor_entities(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(doc) = self.current_doc(window, cx) else {
            return;
        };
        let (editor, title) = doc.update(cx, |doc, cx| {
            // A sticker may have created the doc — register this window's
            // focus subs too (idempotent).
            doc.ensure_editor_attached(window, cx);
            (doc.editor.clone(), doc.title_for(window, cx))
        });
        self.editor = Some(editor);
        self.title_input = Some(title);
        doc.update(cx, |doc, cx| doc.apply_pending(window, cx));
    }

    pub(crate) fn save(&mut self, cx: &mut Context<Self>) {
        let Some(entry) = self.current.clone() else {
            return;
        };
        // Prefer the shared doc path — it stamps the in-flight title/markdown
        // so `Reply::Saved` restores dirty flags correctly in every window.
        if let Some(doc) = self.docs.get(&entry.id).cloned() {
            doc.update(cx, |d, cx| d.save_now(cx));
            return;
        }
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

    /// `saveEntry` body — the manual button path (autosaves are emitted by
    /// the doc as `DocEvent::Save`).
    pub(crate) fn send_save(&mut self, title: &str, markdown: &str, cx: &mut Context<Self>) {
        let Some(entry) = self.current.clone() else {
            return;
        };
        let mut next = entry;
        next.title = title.to_string();
        next.content_json = content::write_entry_markdown(markdown).to_string();
        next.content_loaded = Some(true);
        // Stamp the revision so a stale `Saved` can't un-dirty later edits.
        if let Some(editor) = &self.editor {
            editor.update(cx, |e, _| e.mark_pending_save());
        }
        self.send(Command::SaveEntry(Box::new(next)), cx);
    }
}
