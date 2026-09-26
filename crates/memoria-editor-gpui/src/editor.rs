//! `MemoriaEditor` — the view entity wrapping `memoria_editor_core::Editor`.
//! Owns everything the pure core does not: focus + caret blink, projection→row
//! caches, scroll position, zoom, image/highlight caches, the language picker
//! state, autosave debounce, and the `EditorEvent` surface the app consumes.

use gpui::{
    px, Bounds, Context, EventEmitter, FocusHandle, Pixels, SharedString, Subscription, Task,
    Window,
};
use memoria_editor_core::{Editor, Projection, Selection};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use crate::highlight::HighlightCache;
use crate::layout::DocLayout;
use crate::picker::LangPicker;
use crate::rows::Row;
use crate::style::{self, EditorScale};

/// Vue `autosave` debounce — `edenStoreSaveActions` uses 300 ms.
pub const AUTOSAVE_DEBOUNCE: Duration = Duration::from_millis(300);
/// Vue `Placeholder` text.
pub const PLACEHOLDER: &str = "Начните писать...";
/// Ctrl+K,Z zen chord window — Vue `CHORD_WINDOW_MS = 700`.
pub(crate) const ZEN_CHORD_TIMEOUT: Duration = Duration::from_millis(700);

/// Events the app subscribes to.
#[derive(Debug, Clone)]
pub enum EditorEvent {
    /// Buffer content changed (typing, paste, commands, undo).
    Edited,
    /// Caret/selection changed without an edit.
    SelectionChanged,
    /// Debounced save — carries the markdown source to persist.
    Autosave(SharedString),
    /// `Ctrl+K Z` — zen mode toggle (app owns chrome).
    ZenToggled,
    /// `Ctrl+K` pressed — Vue opens search and arms the chord. Emitted so the
    /// app can wire search; the chord stays armed regardless.
    CtrlK,
    /// Zoom factor changed (`Ctrl +/-/0`).
    ZoomChanged(f32),
}

pub struct MemoriaEditor {
    pub core: Editor,
    pub focus: FocusHandle,
    pub(crate) cursor_visible: bool,
    blink: Task<()>,
    /// True while `ctrl-k` waits for the zen `z`.
    pub(crate) zen_armed: Option<Task<()>>,
    // ---- caches ------------------------------------------------------------
    /// Projection + rows snapshot for the frame.
    pub(crate) proj: Option<Arc<Projection>>,
    pub(crate) rows: Rc<Vec<Row>>,
    pub(crate) layout: DocLayout,
    /// `row index → shaped line` (visible window only, cleared on rebuild).
    pub(crate) shaped: std::collections::HashMap<usize, WrappedSlot>,
    built_rev: u64,
    built_sel: Selection,
    pub(crate) scroll_y: Pixels,
    pub(crate) zoom: EditorScale,
    /// Last element bounds — for IME bounds + scroll clamping.
    pub(crate) bounds: Bounds<Pixels>,
    /// When set, next paint scrolls the caret into view.
    pub(crate) follow_caret: bool,
    /// Mouse drag anchor (source offset).
    pub(crate) drag_anchor: Option<usize>,
    /// Base dir for relative image paths (app sets the vault/note dir).
    pub image_root: Option<std::path::PathBuf>,
    // ---- features ----------------------------------------------------------
    pub high: HighlightCache,
    /// Code-block language picker (open state).
    pub picker: Option<LangPicker>,
    // ---- save --------------------------------------------------------------
    pub(crate) save_task: Task<()>,
    pub(crate) dirty: bool,
    /// Revision at the last `Autosave` emit — `mark_saved` only clears
    /// `dirty` when nothing was typed since (stale `Saved` replies must
    /// not un-dirty edits that raced an in-flight save).
    pub(crate) autosaved_rev: u64,
    _subs: Vec<Subscription>,
}

/// Pixels shaved off a row's wrap width (code rows are inset by panel padding).
fn row_wrap_delta(row: &Row, z: EditorScale) -> Pixels {
    if crate::rows::is_code_row(row) {
        px(2. * style::CODE_PAD_X * z.0)
    } else {
        px(0.)
    }
}

/// Cached shaped line plus the wrap width it was shaped at.
pub(crate) struct WrappedSlot {
    pub line: gpui::WrappedLine,
    pub width: Pixels,
}

impl MemoriaEditor {
    pub fn new(src: impl Into<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        let _subs = vec![
            cx.on_focus(&focus, window, |this, _w, cx| {
                this.cursor_visible = true;
                this.blink = crate::lifecycle::blink_task(cx);
                cx.notify();
            }),
            cx.on_blur(&focus, window, |this, _w, cx| {
                this.cursor_visible = false;
                this.blink = Task::ready(());
                this.core.break_undo_group();
                cx.notify();
            }),
            crate::lifecycle::picker_key_interceptor(cx),
        ];
        Self {
            core: Editor::new(&src.into()),
            focus,
            cursor_visible: false,
            blink: Task::ready(()),
            zen_armed: None,
            proj: None,
            rows: Rc::new(Vec::new()),
            layout: DocLayout::default(),
            shaped: Default::default(),
            built_rev: u64::MAX,
            built_sel: Selection::default(),
            scroll_y: px(0.),
            zoom: EditorScale::default(),
            bounds: Bounds::default(),
            follow_caret: true,
            drag_anchor: None,
            image_root: None,
            high: HighlightCache::new(),
            picker: None,
            save_task: Task::ready(()),
            dirty: false,
            autosaved_rev: 0,
            _subs,
        }
    }

    // ---- content -----------------------------------------------------------

    /// Markdown source (what gets persisted).
    pub fn markdown(&self) -> String {
        self.core.serialize()
    }

    /// Live-refresh path: replace content wholesale. Caller must already have
    /// decided the swap is safe (M1 `should_apply_remote_entry`); selection is
    /// clamped and the undo boundary breaks before it.
    pub fn set_markdown(&mut self, md: &str, cx: &mut Context<Self>) {
        let mut tx = memoria_editor_core::Tx::new();
        let head = self.core.selection().head.min(md.len());
        tx.replace(0, self.core.len(), md.to_string());
        tx.selection(Selection::caret(head));
        self.core.apply(tx, memoria_editor_core::EditKind::Command);
        // Reset AFTER applying: the swap itself must not be undoable either —
        // Ctrl+Z in note B would otherwise resurrect note A's text (and the
        // autosave would persist it under B's entry).
        self.core.reset_history();
        // The fill defines the new baseline: drop the dirty flag, the pending
        // debounce and the last emitted-revision marker.
        self.dirty = false;
        self.autosaved_rev = self.core.revision();
        self.save_task = Task::ready(());
        self.after_edit(false, cx);
    }

    /// Character count — `charCount.ts` parity (visible chars, markup excluded).
    pub fn char_count(&mut self) -> usize {
        let src = self.core.text();
        memoria_editor_core::charcount::count_chars(self.core.doc(), &src)
    }

    /// Whether content changed since the last `mark_saved`.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Call after a successful save. Only clears `dirty` when the buffer
    /// hasn't changed since the autosave emit — a `Saved` reply that lands
    /// after fresh typing must not wipe the dirty flag.
    pub fn mark_saved(&mut self) {
        if self.core.revision() == self.autosaved_rev {
            self.dirty = false;
        }
    }

    /// Save the current buffer immediately if dirty (note-switch flush).
    pub fn flush_autosave(&mut self, cx: &mut Context<Self>) {
        if self.dirty {
            self.dirty = false;
            self.autosaved_rev = self.core.revision();
            let md: SharedString = self.markdown().into();
            cx.emit(EditorEvent::Autosave(md));
        }
    }

    // ---- projection / rows -------------------------------------------------

    /// Rebuild projection + rows when the buffer or selection changed.
    pub(crate) fn ensure_rows(&mut self) {
        let (rev, sel) = (self.core.revision(), self.core.selection());
        if self.built_rev == rev && self.built_sel == sel && self.proj.is_some() {
            return;
        }
        let (proj, doc) = self.core.project_and_doc();
        let proj = Arc::new(proj.clone());
        self.rows = Rc::new(crate::rows::build_rows(&proj, doc));
        self.proj = Some(proj);
        self.shaped.clear();
        self.built_rev = rev;
        self.built_sel = sel;
        self.relayout();
    }

    pub(crate) fn relayout(&mut self) {
        let z = self.zoom;
        let shaped = &self.shaped;
        let global = self.layout.wrap_width;
        let rows = self.rows.clone();
        self.layout.rebuild(&rows, z, |i| {
            let slot = shaped.get(&i)?;
            let row = &rows[i];
            let want = global? - row_wrap_delta(row, z);
            (slot.width == want).then(|| {
                slot.line
                    .size(style::block_style(&row.tag, z).line_height)
                    .height
            })
        });
    }

    /// Ensure `row` is shaped for `wrap_width`; returns whether its measured
    /// height changed (caller re-runs layout if so).
    pub(crate) fn shape_row(&mut self, i: usize, window: &mut Window) -> bool {
        let Some(global_wrap) = self.layout.wrap_width else {
            return false;
        };
        let old_h = self.layout.heights.get(i).copied();
        let row = self.rows.get(i).cloned().unwrap_or_else(|| Row {
            vis: 0..0,
            block_ix: 0,
            tag: memoria_editor_core::project::BlockTag::Paragraph,
            spans: vec![],
            kind: crate::rows::RowKind::Text,
            first_in_block: true,
            last_in_block: true,
            code_origin: 0,
        });
        let z = self.zoom;
        let st = style::block_style(&row.tag, z);
        // Code rows are inset by the panel padding on both sides.
        let wrap = global_wrap - row_wrap_delta(&row, z);
        if let Some(slot) = self.shaped.get(&i) {
            if slot.width == wrap {
                return false;
            }
        }
        let runs = self.runs_for_row(i, &row);
        // NB: H5's CSS `text-transform: uppercase` is NOT reproduced —
        // uppercasing here would desync shaped-glyph indices for
        // case-expanding chars (ß→SS). Documented GAP in PARITY.md.
        let text = self
            .proj
            .as_ref()
            .map(|p| gpui::SharedString::from(p.text[row.vis.clone()].to_string()))
            .unwrap_or_default();
        let ts = window.text_system();
        let line = ts
            .shape_text(text, st.size, &runs, Some(wrap), None)
            .unwrap_or_default()
            .into_iter()
            .next()
            .unwrap_or_default();
        let h = line.size(st.line_height).height
            + if crate::rows::is_code_row(&row) && (row.first_in_block || row.last_in_block) {
                crate::layout::row_pad(&row, z)
            } else {
                px(0.)
            };
        self.shaped.insert(i, WrappedSlot { line, width: wrap });
        Some(h) != old_h
    }
}

impl EventEmitter<EditorEvent> for MemoriaEditor {}
