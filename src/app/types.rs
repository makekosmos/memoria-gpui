//! App-level state types — confirm dialog, context menu, toasts, conflict
//! continuation ops, and the `ConflictStore` bridge over the Engine data dir.
use std::path::PathBuf;

use memoria_model::conflict_store::ConflictStore;

pub(crate) const ENGINE_OFFLINE: &str =
    "Engine не запущен. Запустите Mundus — список обновится автоматически.";
pub(crate) const TOAST_TTL: std::time::Duration = std::time::Duration::from_millis(3500);
pub(crate) const SEARCH_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(300);

/// `ConflictStore` over the Engine data dir — the `userData` bridge analog.
pub(crate) struct DataDirStore(pub PathBuf);

impl ConflictStore for DataDirStore {
    fn read_json(&self, name: &str) -> Result<Option<serde_json::Value>, String> {
        let bytes = match std::fs::read(self.0.join(name)) {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| e.to_string())
    }
    fn write_json(&mut self, name: &str, value: serde_json::Value) -> Result<(), String> {
        std::fs::create_dir_all(&self.0).map_err(|e| e.to_string())?;
        std::fs::write(
            self.0.join(name),
            serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }
}

/// Open property-picker overlay state (Vue `ObjectPropertyPicker` panel).
/// `options` are precomputed so the overlay render stays `&self`.
#[derive(Clone, Debug)]
pub(crate) struct PropPicker {
    pub entry_id: String,
    pub field_id: String,
    pub multiple: bool,
    pub options: Vec<(String, String)>,
    pub selected: Vec<String>,
    pub x: f32,
    pub y: f32,
}

/// Cover modal (Vue `typed-object-header__cover-modal`): URL field + file
/// dropzone. `saving` blocks close while `images.storeCover`/`images.fetch`
/// is in flight.
pub(crate) struct CoverModal {
    pub entry_id: String,
    pub image_field_id: String,
    pub url_input: gpui::Entity<gpui_component::input::InputState>,
    pub saving: bool,
    pub error: Option<String>,
}

/// Metadata import modal (Vue `BookMetadataImportModal`): ISBN/URL source,
/// loading → preview rows with per-field checkboxes → apply.
pub(crate) struct MetadataModal {
    pub entry_id: String,
    pub source_input: gpui::Entity<gpui_component::input::InputState>,
    pub loading: bool,
    pub error: Option<String>,
    pub metadata: Option<memoria_model::book_metadata::BookMetadata>,
    pub selected: Vec<String>,
    /// Set while a `fetchPage` result awaits its `lookupIsbn` enrichment.
    pub enrich_isbn: Option<String>,
}

/// Pending destructive action shown by the confirm dialog (Vue `confirm()`).
#[derive(Clone, Debug)]
pub(crate) enum Confirm {
    DeleteEntry(String),
    DeleteForever(String),
    EmptyTrash,
}

/// Right-click context menu (`EverythingItemCard` context menu port).
#[derive(Clone, Debug)]
pub(crate) struct CtxMenu {
    pub x: f32,
    pub y: f32,
    pub entry_id: String,
    pub pinned: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Toast {
    pub id: u64,
    pub text: String,
}

/// One async conflict step awaiting `Reply::Entry`/`Reply::Saved`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConflictOp {
    Recheck,
    Accept,
}
