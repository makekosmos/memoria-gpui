//! M1 debug shell: notes list + a plain-markdown editor surface so the whole
//! Engine path (list → load → edit → save) is exercised end to end. The real
//! editor lands in M2 — this page exists to prove the data layer.
use std::sync::mpsc::TryRecvError;

use gpui::{div, prelude::*, px, Context, Entity, MouseButton, SharedString, Subscription, Window};
use gpui_component::input::{Input, InputEvent, InputState, Textarea, TextareaState};

use memoria_gpui::content;
use memoria_gpui::entry_titles::get_entry_display_title;
use memoria_gpui::model::Entry;
use memoria_gpui::store::{Command, EngineEvent, Reply, Worker};

use crate::theme::*;

mod render;

const ENGINE_OFFLINE: &str =
    "Engine не запущен. Запустите Kosmos — список обновится автоматически.";

pub struct Memoria {
    worker: Option<Worker>,
    busy: bool,
    online: bool,
    banner: Option<String>,
    list: Vec<Entry>,
    selected_id: Option<String>,
    current: Option<Entry>,
    /// Unsaved edits in the inputs — remote refreshes must not clobber them.
    dirty: bool,
    status: Option<String>,
    title_input: Option<Entity<InputState>>,
    body_input: Option<Entity<TextareaState>>,
    /// (title, markdown) queued for the inputs — `set_value` needs a `Window`,
    /// so replies stash values here and `render` applies them.
    pending_fill: Option<(String, String)>,
    _subs: Vec<Subscription>,
    _poll: Option<gpui::Task<()>>,
}

impl Memoria {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let worker = Worker::start();
        let _ = worker.commands.send(Command::LoadNoteTypes);
        let _ = worker.commands.send(Command::LoadList(Vec::new()));
        let mut this = Self {
            worker: Some(worker),
            busy: true,
            online: true,
            banner: None,
            list: Vec::new(),
            selected_id: None,
            current: None,
            dirty: false,
            status: None,
            title_input: None,
            body_input: None,
            pending_fill: None,
            _subs: Vec::new(),
            _poll: None,
        };
        this.start_poll(cx);
        this
    }

    fn start_poll(&mut self, cx: &mut Context<Self>) {
        self._poll = Some(cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;
            let alive = this
                .update(cx, |this, cx| this.drain_replies(cx))
                .unwrap_or(false);
            if !alive {
                break;
            }
        }));
    }

    fn drain_replies(&mut self, cx: &mut Context<Self>) -> bool {
        loop {
            let reply = match self.worker.as_ref().map(|w| w.replies.try_recv()) {
                Some(Ok(reply)) => reply,
                Some(Err(TryRecvError::Disconnected)) => {
                    self.worker = None;
                    self.busy = false;
                    self.banner = Some(ENGINE_OFFLINE.into());
                    cx.notify();
                    return true;
                }
                _ => return true,
            };
            self.on_reply(reply, cx);
        }
    }

    fn on_reply(&mut self, reply: Reply, cx: &mut Context<Self>) {
        self.busy = false;
        match reply {
            Reply::List(Ok(list)) => {
                self.list = list;
                if self.online {
                    self.banner = None;
                }
            }
            Reply::List(Err(error)) | Reply::NoteTypes(Err(error)) => {
                self.online = false;
                self.banner = Some(error);
            }
            Reply::Entry { result, .. } => match result {
                Ok(Some(entry)) => self.set_current(entry, cx),
                Ok(None) => self.status = Some("Заметка не найдена".into()),
                Err(error) => self.status = Some(error),
            },
            Reply::Saved(result) => match result {
                Ok(saved) if saved.ok => {
                    self.dirty = false;
                    self.status = Some("Сохранено".into());
                    self.send(Command::LoadList(Vec::new()));
                }
                Ok(saved) => {
                    self.status = Some(
                        saved
                            .message
                            .unwrap_or_else(|| "Не удалось сохранить".into()),
                    );
                }
                Err(error) => self.status = Some(error),
            },
            Reply::Deleted { result, .. } => match result {
                Ok(done) if done.ok => {
                    self.current = None;
                    self.selected_id = None;
                    self.send(Command::LoadList(Vec::new()));
                }
                Ok(done) => {
                    self.status = done.message;
                }
                Err(error) => self.status = Some(error),
            },
            Reply::NoteTypes(Ok(_)) | Reply::Collections(_) => {}
            Reply::Search { .. } => {}
            Reply::Event(event) => match event {
                EngineEvent::Online => {
                    self.online = true;
                    self.banner = None;
                    self.send(Command::LoadList(Vec::new()));
                }
                EngineEvent::Offline => {
                    self.online = false;
                    self.banner = Some(ENGINE_OFFLINE.into());
                }
                EngineEvent::Changed(_) => {
                    self.send(Command::LoadList(Vec::new()));
                    if !self.dirty {
                        if let Some(id) = self.selected_id.clone() {
                            self.send(Command::LoadEntry {
                                id,
                                content_only: false,
                            });
                        }
                    }
                }
            },
        }
        cx.notify();
    }

    fn send(&mut self, command: Command) {
        if self
            .worker
            .as_ref()
            .is_some_and(|w| w.commands.send(command).is_ok())
        {
            self.busy = true;
        } else {
            self.banner = Some(ENGINE_OFFLINE.into());
        }
    }

    fn set_current(&mut self, entry: Entry, cx: &mut Context<Self>) {
        // Vue: `readEntryMarkdown(JSON.parse(entry.content_json))`.
        let raw = serde_json::from_str(&entry.content_json).unwrap_or(serde_json::Value::Null);
        let markdown = content::read_entry_markdown(&raw);
        self.pending_fill = Some((entry.title.clone(), markdown));
        self.current = Some(entry);
        self.dirty = false;
        self.status = None;
        cx.notify();
    }

    fn select(&mut self, id: &str, _cx: &mut Context<Self>) {
        self.selected_id = Some(id.to_string());
        self.send(Command::LoadEntry {
            id: id.to_string(),
            content_only: false,
        });
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(entry) = self.current.clone() else {
            return;
        };
        let title = self
            .title_input
            .as_ref()
            .map(|s| s.read(cx).value().to_string())
            .unwrap_or_else(|| entry.title.clone());
        let markdown = self
            .body_input
            .as_ref()
            .map(|s| s.read(cx).value().to_string())
            .unwrap_or_default();
        let mut next = entry.clone();
        next.title = title;
        next.content_json = content::write_entry_markdown(&markdown).to_string();
        next.content_loaded = Some(true);
        self.send(Command::SaveEntry(Box::new(next)));
    }

    fn title_state(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
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

    fn body_state(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<TextareaState> {
        if let Some(state) = &self.body_input {
            return state.clone();
        }
        let state = cx.new(|cx| TextareaState::new(window, cx).placeholder("Markdown…"));
        self._subs
            .push(cx.subscribe(&state, |this, _, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.dirty = true;
                    cx.notify();
                }
            }));
        self.body_input = Some(state.clone());
        state
    }

    fn list_row(&mut self, entry: &Entry, cx: &mut Context<Self>) -> impl IntoElement {
        let id = entry.id.clone();
        let selected = self.selected_id.as_deref() == Some(entry.id.as_str());
        let source = entry
            .header_props_json
            .as_deref()
            .map(serde_json::Value::from);
        let title = get_entry_display_title(Some(entry.title.as_str()), source.as_ref());
        div()
            .id(SharedString::from(id.clone()))
            .px_3()
            .py_2()
            .rounded(px(6.))
            .cursor_pointer()
            .when(selected, |d| d.bg(c(ACCENT_DIM())))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.select(&id, cx);
                }),
            )
            .child(div().text_size(px(13.)).text_color(c(FG())).child(title))
    }
}
