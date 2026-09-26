//! Memoria M4 shell + M3 live-preview editor: `DesktopChrome`-style window
//! with an imago sidebar, routed screens, navigation history, search overlay,
//! settings + trash, conflict banner and toasts. Note routes embed the M3
//! `MemoriaEditor` (input, IME, code & images) with autosave + live refresh.
mod backend;
mod chrome;
mod confirm;
mod conflict;
mod conflict_banner;
mod conflict_ops;
mod demo;
mod editor_host;
mod everything;
mod highlight;
mod image;
mod menus;
mod note;
mod objects;
mod refresh;
mod render;
mod search;
mod settings;
mod settings_widgets;
mod sidebar;
mod sticker;
mod toasts;
mod trash;
mod types;

use std::path::PathBuf;

use gpui::{Context, Entity, FocusHandle, Subscription};
use gpui_component::input::InputState;
use memoria_editor_gpui::MemoriaEditor;

use memoria_gpui::conflict_store::ConflictRepository;
use memoria_gpui::local_state::{load_local_state, LocalState};
use memoria_gpui::model::{Entry, NoteType};
use memoria_gpui::nav_history::NavHistory;
use memoria_gpui::routes::Route;
use memoria_gpui::store::{Command, Engine, Worker};

use types::{Confirm, ConflictOp, CtxMenu, DataDirStore, Toast};

mod dispatch;
mod nav;
mod replies;

pub(crate) use backend::Backend;
pub(crate) use demo::DemoStore;
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
    pub(crate) search_results: Vec<memoria_gpui::model::SearchResult>,
    pub(crate) search_selected: usize,
    pub(crate) search_gen: u64,
    pub(crate) toasts: Vec<Toast>,
    pub(crate) toast_seq: u64,
    pub(crate) conflicts: ConflictRepository<DataDirStore>,
    pub(crate) conflict_op: Option<(String, ConflictOp)>,
    pub(crate) pending_copy_save: Option<String>,
    pub(crate) confirm: Option<Confirm>,
    pub(crate) ctx_menu: Option<CtxMenu>,
    pub(crate) root_focus: FocusHandle,
    pub(crate) focused_once: bool,
    /// Unsaved edits in the note surface — remote refreshes must not clobber.
    pub(crate) dirty: bool,
    pub(crate) status: Option<String>,
    pub(crate) title_input: Option<Entity<InputState>>,
    pub(crate) editor: Option<Entity<MemoriaEditor>>,
    /// Zen mode (Vue `Ctrl+K Z`): hides the sidebar.
    pub(crate) zen: bool,
    /// (title, markdown) queued for the inputs — `set_value` needs a `Window`,
    /// so replies stash values here and `render` applies them.
    pub(crate) pending_fill: Option<(String, String)>,
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
            conflicts.conflicts = memoria_gpui::conflict_store::load_entry_conflicts(store);
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
            root_focus: cx.focus_handle(),
            focused_once: false,
            dirty: false,
            status: None,
            title_input: None,
            editor: None,
            zen: false,
            pending_fill: None,
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
