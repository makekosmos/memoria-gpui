//! Memoria M4 shell + M3 live-preview editor: `DesktopChrome`-style window
//! with an imago sidebar, routed screens, navigation history, search overlay,
//! settings + trash, conflict banner and toasts. Note routes embed the M3
//! `MemoriaEditor` (input, IME, code & images) with autosave + live refresh.
mod backend;
mod book;
mod chrome;
mod chrome_keys;
mod confirm;
mod conflict;
mod conflict_banner;
mod conflict_ops;
mod cover_modal;
mod demo;
mod diary;
mod diary_blocks;
mod diary_calendar;
mod diary_forms;
mod diary_item;
mod diary_ops;
mod diary_rail;
mod editor_host;
mod everything;
mod highlight;
mod image;
mod menus;
mod metadata_apply;
mod metadata_modal;
mod modal;
mod note;
mod objects;
mod prop_edit;
mod prop_field;
mod prop_picker;
mod refresh;
mod render;
mod search;
mod settings;
mod settings_widgets;
mod sidebar;
mod sticker;
mod toasts;
mod trash;
mod typed_header;
mod types;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use gpui::{Context, Entity, FocusHandle, Subscription, WindowHandle};
use gpui_component::input::InputState;
use memoria_editor_gpui::MemoriaEditor;

use memoria_model::conflict_store::ConflictRepository;
use memoria_model::local_state::{load_local_state, LocalState};
use memoria_model::model::{Entry, NoteType};
use memoria_model::nav_history::NavHistory;
use memoria_model::routes::Route;
use memoria_model::store::{Command, Engine, Worker};

use types::{
    Confirm, ConflictOp, CoverModal, CtxMenu, DataDirStore, MetadataModal, PropPicker, Toast,
};

mod dispatch;
mod doc;
mod nav;
mod replies;
mod replies_diary;

pub(crate) use backend::Backend;
pub(crate) use demo::DemoStore;
pub(crate) use doc::{DocEvent, NoteDoc};
pub(crate) use toasts::{icon, icon_name};

pub struct Memoria {
    pub(crate) backend: Backend,
    pub(crate) online: bool,
    pub(crate) banner: Option<String>,
    pub(crate) busy: bool,
    pub(crate) list: Vec<Entry>,
    pub(crate) trash: Vec<Entry>,
    pub(crate) trash_loaded: bool,
    pub(crate) note_types: Vec<NoteType>,
    pub(crate) current: Option<Entry>,
    pub(crate) loading_entry: Option<String>,
    pub(crate) route: Route,
    pub(crate) history: NavHistory,
    pub(crate) prefs: LocalState,
    pub(crate) state_dir: Option<PathBuf>,
    pub(crate) search_open: bool,
    pub(crate) search_input: Option<Entity<InputState>>,
    pub(crate) search_query_cache: String,
    pub(crate) search_results: Vec<memoria_model::model::SearchResult>,
    pub(crate) search_selected: usize,
    pub(crate) search_gen: u64,
    pub(crate) toasts: Vec<Toast>,
    pub(crate) toast_seq: u64,
    pub(crate) conflicts: ConflictRepository<DataDirStore>,
    pub(crate) conflict_op: Option<(String, ConflictOp)>,
    pub(crate) pending_copy_save: Option<(String, String)>,
    pub(crate) confirm: Option<Confirm>,
    pub(crate) ctx_menu: Option<CtxMenu>,
    // Typed-header state (M5): per-field text inputs live for the currently
    // open entry (`header_entry`); picker/modals are transient overlays.
    pub(crate) header_entry: String,
    pub(crate) header_inputs: HashMap<String, Entity<InputState>>,
    pub(crate) prop_picker: Option<PropPicker>,
    pub(crate) cover_modal: Option<CoverModal>,
    pub(crate) metadata_modal: Option<MetadataModal>,
    // Engine-fetched image cache: remote URL → stored local path, and the
    // dominant color per resolved image src (`images.dominantColor`).
    pub(crate) image_cache: HashMap<String, String>,
    pub(crate) image_pending: HashSet<String>,
    pub(crate) spine_colors: HashMap<String, Option<String>>,
    pub(crate) spine_pending: HashSet<String>,
    pub(crate) root_focus: FocusHandle,
    pub(crate) focused_once: bool,
    /// Unsaved edits in the note surface — remote refreshes must not clobber.
    pub(crate) dirty: bool,
    pub(crate) status: Option<String>,
    pub(crate) title_input: Option<Entity<InputState>>,
    pub(crate) editor: Option<Entity<MemoriaEditor>>,
    /// Zen mode (Vue `Ctrl+K Z`): hides the sidebar.
    pub(crate) zen: bool,
    /// Entry queued for the shared doc — `set_value` needs a `Window`, so
    /// replies stash it here and `render` applies it via `NoteDoc::apply_fill`.
    pub(crate) pending_fill: Option<Entry>,
    /// Shared per-entry documents — the main window and every sticker window
    /// showing a note edit the same `NoteDoc` (one buffer, one save path).
    pub(crate) docs: HashMap<String, Entity<NoteDoc>>,
    /// Open sticker windows by `stickerWindowKeyFor` id — reopening a key
    /// focuses the existing window instead of spawning a duplicate.
    pub(crate) stickers: HashMap<String, WindowHandle<gpui_component::Root>>,
    // ---- diary (M6) ---------------------------------------------------------
    /// Thread-normalized diary feed (`listBubbles` reply).
    pub(crate) bubbles: Vec<memoria_model::diary::BubbleTimelineNode>,
    pub(crate) bubbles_loaded: bool,
    /// `calendarOpen` — the titlebar toggle drives the calendar sidebar.
    pub(crate) diary_calendar_open: bool,
    /// `startDiary` ran once (migration + first list) for this session.
    pub(crate) diary_started: bool,
    /// Compact composer entity (Vue `composerEditor`).
    pub(crate) diary_composer: Option<Entity<MemoriaEditor>>,
    /// Thread root id whose reply box is open (`replyOpen`).
    pub(crate) reply_target: Option<String>,
    pub(crate) reply_input: Option<Entity<gpui_component::input::TextareaState>>,
    /// Bubble id in edit mode (`isEditing`) + its textarea.
    pub(crate) editing_bubble: Option<String>,
    pub(crate) edit_input: Option<Entity<gpui_component::input::TextareaState>>,
    /// Two-step delete arm (`deleteStep === 1`).
    pub(crate) bubble_delete_armed: Option<String>,
    /// Kind dropdown open for this bubble id, anchored at the click point
    /// (rendered as a root-level overlay like `render_ctx_menu`).
    pub(crate) kind_menu_for: Option<(String, gpui::Point<gpui::Pixels>)>,
    pub(crate) diary_scroll: gpui::ScrollHandle,
    /// Calendar click → scroll target date key, consumed by the next render.
    pub(crate) diary_jump: Option<String>,
    /// `labelNow` — occurrence labels re-resolve when this refreshes.
    pub(crate) label_now: i64,
    /// `BubbleCreated(Ok)` asks render to clear the composer + reply textarea
    /// (they need a `Window`, so the reset is deferred).
    pub(crate) pending_diary_reset: bool,
    /// Vue `activeBubbleWrites` — Engine `Changed` events skip the feed
    /// refresh while a bubble write is in flight.
    pub(crate) active_bubble_writes: u32,
    pub(crate) _subs: Vec<Subscription>,
    pub(crate) _poll: Option<gpui::Task<()>>,
}

impl Memoria {
    pub fn new(cx: &mut Context<Self>) -> Self {
        if std::env::var("MEMORIA_DEMO").as_deref() == Ok("1") || cfg!(test) {
            return Self::with_backend(Backend::Demo(DemoStore::seeded()), cx, None);
        }
        let worker = Worker::start();
        let engine = Engine::default();
        Self::with_backend(Backend::Engine(worker), cx, engine.resolved_data_dir())
    }

    pub(crate) fn with_backend(
        backend: Backend,
        cx: &mut Context<Self>,
        state_dir: Option<PathBuf>,
    ) -> Self {
        let prefs = state_dir
            .as_ref()
            .map(|d| load_local_state(d))
            .unwrap_or_default();
        let mut conflicts = ConflictRepository::new(state_dir.clone().map(DataDirStore));
        if let Some(store) = conflicts.store.as_ref() {
            conflicts.conflicts = memoria_model::conflict_store::load_entry_conflicts(store);
        }
        let mut this = Self {
            backend,
            online: true,
            banner: None,
            busy: true,
            list: Vec::new(),
            trash: Vec::new(),
            trash_loaded: false,
            note_types: Vec::new(),
            current: None,
            loading_entry: None,
            route: Route::Everything,
            history: NavHistory::new(),
            prefs,
            state_dir,
            search_open: false,
            search_input: None,
            search_query_cache: String::new(),
            search_results: Vec::new(),
            search_selected: 0,
            search_gen: 0,
            toasts: Vec::new(),
            toast_seq: 0,
            conflicts,
            conflict_op: None,
            pending_copy_save: None,
            confirm: None,
            ctx_menu: None,
            header_entry: String::new(),
            header_inputs: HashMap::new(),
            prop_picker: None,
            cover_modal: None,
            metadata_modal: None,
            image_cache: HashMap::new(),
            image_pending: HashSet::new(),
            spine_colors: HashMap::new(),
            spine_pending: HashSet::new(),
            root_focus: cx.focus_handle(),
            focused_once: false,
            dirty: false,
            status: None,
            title_input: None,
            editor: None,
            zen: false,
            pending_fill: None,
            docs: HashMap::new(),
            stickers: HashMap::new(),
            bubbles: Vec::new(),
            bubbles_loaded: false,
            diary_calendar_open: false,
            diary_started: false,
            diary_composer: None,
            reply_target: None,
            reply_input: None,
            editing_bubble: None,
            edit_input: None,
            bubble_delete_armed: None,
            kind_menu_for: None,
            diary_scroll: gpui::ScrollHandle::new(),
            diary_jump: None,
            label_now: 0,
            pending_diary_reset: false,
            active_bubble_writes: 0,
            _subs: Vec::new(),
            _poll: None,
        };
        this.history.record(Route::Everything);
        this.send(Command::LoadNoteTypes, cx);
        this.send(Command::EnsureCollections, cx);
        this.send(Command::LoadList(Vec::new()), cx);
        this.start_poll(cx);
        this
    }
}
