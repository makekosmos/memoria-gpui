//! `NoteDoc` — per-note document shared by main + sticker windows. One entity owns the
//! entry snapshot, the canonical title and the `MemoriaEditor` buffer, so
//! editing in any window mutates a single document and a single save path
//! exists (the Vue equivalent is the sticker sub-app sharing Eden's store).
//!
//! Two window-bound states can't be shared across windows:
//! - `InputState` caches per-view layout on the entity — two windows
//!   rendering one input flip-flop `cx.notify()` on every paint and never
//!   stop redrawing, so each window binds its own input to `title_text`.
//! - `MemoriaEditor`'s focus/blur subscriptions are window-bound — each new
//!   host window registers its own set via `attach_window`.
//!
//! Saves are emitted as `DocEvent::Save` and forwarded by `Memoria` through
//! its backend: a queued effect, so demo-backend synchronous replies can't
//! re-borrow this entity mid-update.

use std::collections::HashMap;

use gpui::{prelude::*, Context, Entity, Subscription, Task, WeakEntity, Window, WindowId};
use gpui_component::input::{InputEvent, InputState};
use memoria_editor_gpui::{EditorEvent, MemoriaEditor, AUTOSAVE_DEBOUNCE};

use super::refresh;
use memoria_model::content;
use memoria_model::live_refresh::RemoteEntryDecision;
use memoria_model::model::Entry;

/// Doc→app events — `Memoria` subscribes at doc creation and forwards saves
/// through `Backend::send` (Engine worker or the in-memory demo store).
pub(crate) enum DocEvent {
    /// `Command::SaveEntry` payload, already merged with the canonical title.
    Save(Box<Entry>),
}

impl gpui::EventEmitter<DocEvent> for NoteDoc {}

/// One open note document — created once per entry id (`Memoria::docs`).
pub(crate) struct NoteDoc {
    /// `None` until `LoadEntry` lands — `Ok(None)`/error flips `failed`.
    pub(crate) entry: Option<Entry>,
    pub(crate) loading: bool,
    pub(crate) failed: bool,
    /// Unsaved title edits — remote refreshes must not clobber them.
    pub(crate) title_dirty: bool,
    /// Title baseline for `title_dirty` — updated on entry fill and save.
    saved_title: String,
    /// Title sent with the in-flight save — `Reply::Saved` restores dirty
    /// against it (typing during the save stays dirty).
    pending_save_title: Option<String>,
    /// Canonical title — each window's `InputState` binds over this string.
    title_text: String,
    pub(crate) status: Option<String>,
    /// The shared editor buffer — safe to render in several windows (its
    /// per-frame layout lives in `PrepaintState`, not on the entity).
    pub(crate) editor: Entity<MemoriaEditor>,
    /// Title bindings keyed by host window.
    titles: HashMap<WindowId, WeakEntity<InputState>>,
    /// Windows that already registered editor focus subs.
    attached: Vec<WindowId>,
    /// Debounced save for title-only edits (Vue `entry-draft-change` →
    /// `scheduleSave` — title drafts persist through the same 300ms path).
    title_save: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl NoteDoc {
    /// Stub document — the caller registers it under the entry id and kicks
    /// off `LoadEntry`. The editor binds to `window`; additional windows
    /// attach via `ensure_editor_attached`.
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            let mut e = MemoriaEditor::new("", window, cx);
            e.image_root = std::env::var("MEMORIA_IMAGES_DIR").ok().map(Into::into);
            e
        });
        let editor_sub = cx.subscribe(&editor, |this, _, ev: &EditorEvent, cx| match ev {
            EditorEvent::Edited => cx.notify(),
            EditorEvent::Autosave(markdown) => this.emit_save(markdown, cx),
            EditorEvent::ZenToggled
            | EditorEvent::ZoomChanged(_)
            | EditorEvent::SelectionChanged
            | EditorEvent::CtrlK
            | EditorEvent::Submit => {}
        });
        let wid = window.window_handle().window_id();
        Self {
            entry: None,
            loading: true,
            failed: false,
            title_dirty: false,
            saved_title: String::new(),
            pending_save_title: None,
            title_text: String::new(),
            status: None,
            editor,
            titles: HashMap::new(),
            attached: vec![wid],
            title_save: None,
            _subs: vec![editor_sub],
        }
    }

    /// `MemoriaEditor::new` binds focus subs to its creating window — call
    /// once per additional host window (idempotent).
    pub(crate) fn ensure_editor_attached(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let wid = window.window_handle().window_id();
        if self.attached.contains(&wid) {
            return;
        }
        self.attached.push(wid);
        self.editor.update(cx, |e, cx| e.attach_window(window, cx));
    }

    /// This window's title binding — created on first request, wired to fold
    /// `InputEvent::Change` back into `title_text`.
    pub(crate) fn title_for(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        let wid = window.window_handle().window_id();
        self.titles.retain(|_, w| w.upgrade().is_some());
        if let Some(input) = self.titles.get(&wid).and_then(|w| w.upgrade()) {
            return input;
        }

        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Название")
                .default_value(self.title_text.clone())
        });
        let sub = cx.subscribe(&input, |this, input, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                this.on_title_changed(&input, cx);
            }
        });
        self._subs.push(sub);
        self.titles.insert(wid, input.downgrade());
        input
    }

    /// `InputEvent::Change` from any bound title input — fold the edit into
    /// the canonical title and arm the draft-save debounce. The guard
    /// matters: `apply_pending` writes back too, and an unguarded notify
    /// would ping-pong.
    fn on_title_changed(&mut self, input: &Entity<InputState>, cx: &mut Context<Self>) {
        let value = input.read(cx).value().to_string();
        if value == self.title_text {
            return;
        }
        self.title_text = value;
        self.title_dirty = self.title_text != self.saved_title;
        self.title_save = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(AUTOSAVE_DEBOUNCE).await;
            this.update(cx, |doc, cx| {
                if doc.title_dirty {
                    doc.save_now(cx);
                }
            })
            .ok();
        }));
        cx.notify();
    }

    /// `Reply::Entry` — M1 live-refresh decision: remote edits fill only a
    /// clean buffer; self-echoes/dirty buffers refresh the snapshot only.
    pub(crate) fn apply_entry_reply(
        &mut self,
        result: Result<Option<Entry>, String>,
        cx: &mut Context<Self>,
    ) {
        self.loading = false;
        match result {
            Ok(Some(entry)) => {
                self.failed = false;
                let markdown = content::read_entry_markdown(
                    &serde_json::from_str(&entry.content_json).unwrap_or(serde_json::Value::Null),
                );
                let current_md = self.editor.read(cx).markdown();
                let decision = refresh::refresh_decision(
                    &entry,
                    self.entry.as_ref(),
                    &current_md,
                    self.is_dirty(cx),
                );
                if decision == RemoteEntryDecision::Apply {
                    self.apply_fill(&entry, &markdown, cx);
                }
                self.entry = Some(entry);
                self.status = None;
            }
            Ok(None) => {
                self.failed = true;
                self.status = Some("Заметка не найдена".into());
            }
            Err(error) => {
                self.failed = true;
                self.status = Some(error);
            }
        }
        cx.notify();
    }

    /// Forced fill — the caller (nav/pending_fill) already ran the refresh
    /// decision and decided `Apply`.
    pub(crate) fn apply_fill(&mut self, entry: &Entry, markdown: &str, cx: &mut Context<Self>) {
        self.title_text = entry.title.clone();
        self.saved_title = entry.title.clone();
        self.title_dirty = false;
        // `set_markdown` emits no `Edited` and clears the dirty flag — a
        // programmatic fill isn't a user edit.
        self.editor.update(cx, |e, cx| e.set_markdown(markdown, cx));
        self.entry = Some(entry.clone());
        self.loading = false;
        self.failed = false;
        self.status = None;
        cx.notify();
    }

    /// `Reply::Saved` — restore dirty flags from the in-flight
    /// revision/title (typing during the save stays dirty).
    pub(crate) fn on_saved(
        &mut self,
        result: Result<memoria_model::model::SaveEntryResult, String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(saved) if saved.ok => {
                self.editor.update(cx, |e, _| e.mark_saved());
                if let Some(title) = self.pending_save_title.take() {
                    self.saved_title = title;
                }
                self.title_dirty = self.title_text != self.saved_title;
                self.status = Some("Сохранено".into());
            }
            Ok(saved) => {
                self.status = Some(
                    saved
                        .message
                        .unwrap_or_else(|| "Не удалось сохранить".into()),
                );
            }
            Err(error) => self.status = Some(error),
        }
        cx.notify();
    }

    /// Push the canonical title into this window's binding. Called from
    /// `Render` because `set_value` needs a `Window`; the value check makes
    /// the per-frame call cheap and keeps cross-window sync echo-free
    /// (`set_value` also suppresses `InputEvent` emission).
    pub(crate) fn apply_pending(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let want = self.title_text.clone();
        let wid = window.window_handle().window_id();
        if let Some(input) = self.titles.get(&wid).and_then(|w| w.upgrade()) {
            input.update(cx, |s, cx| {
                if s.value().as_str() != want {
                    s.set_value(want, window, cx);
                }
            });
        }
    }

    /// Dirty = unsaved edits in either input (title or body).
    pub(crate) fn is_dirty(&self, cx: &gpui::App) -> bool {
        self.title_dirty || self.editor.read(cx).is_dirty()
    }

    /// Save now — title drafts debounce through the same path.
    pub(crate) fn save_now(&mut self, cx: &mut Context<Self>) {
        self.emit_save(&self.editor.read(cx).markdown(), cx);
    }

    /// Flush the debounced autosave (body + pending title draft) before the
    /// document loses focus/switches.
    pub(crate) fn flush(&mut self, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, cx| e.flush_autosave(cx));
        if self.title_dirty {
            self.title_save = None;
            self.save_now(cx);
        }
    }

    fn emit_save(&mut self, markdown: &str, cx: &mut Context<Self>) {
        let Some(entry) = self.entry.clone() else {
            return;
        };
        let mut next = entry;
        next.title = self.title_text.clone();
        next.content_json = content::write_entry_markdown(markdown).to_string();
        next.content_loaded = Some(true);
        self.editor.update(cx, |e, _| e.mark_pending_save());
        self.pending_save_title = Some(self.title_text.clone());
        self.status = Some("Сохранение…".into());
        // Queued effect — the app's `Backend::send` may reply synchronously
        // (demo store), and a nested reply must not re-borrow this entity.
        cx.emit(DocEvent::Save(Box::new(next)));
        cx.notify();
    }
}
