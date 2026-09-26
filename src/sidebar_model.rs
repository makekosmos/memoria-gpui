//! Sidebar model — the fixture-snapshotable structure behind the shell
//! sidebar (types, collections, pinned) with deterministic ordering for
//! snapshot tests. Note: Vue 0.6.9 does not render `EdenSidebar.vue` in
//! App.vue (top-nav only) — KOS-151 asks for the sidebar explicitly.

use crate::model::{Entry, NoteType};
use crate::note_types::get_note_type_collection_name;
use crate::object_views::{collection_target_type_id, entry_display_title};
use crate::routes::Route;
use crate::system_types::should_show_as_eden_collection;
use crate::system_types_data::SYSTEM_TYPE_COLLECTION_ID;

/// Icon names come from the GPUI Lucide catalog (`gpui::assets::IconName` is
/// generated per bundled svg). We map the Vue `iconResolver` names to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconId {
    File,
    FileText,
    Gamepad2,
    Image,
    Dumbbell,
    Activity,
    BookOpen,
    Calendar,
    Orbit,
    Library,
    Folder,
    Sparkles,
    User,
    Pin,
    Search,
    Settings,
    Trash,
    LayoutGrid,
    NotebookPen,
    ChevronLeft,
    ChevronRight,
    Plus,
    X,
    EllipsisVertical,
    RotateCcw,
    Check,
    StickyNote,
    Minus,
    Square,
    Maximize2,
    Minimize2,
    Copy,
    RefreshCw,
    ExternalLink,
    House,
    Tag,
    Boxes,
}

/// `objectIconUri` name → `IconId` (same anytype-name domain as
/// `icon_resolver.rs`; unknown falls back to `document` → FileText).
pub fn icon_for_name(name: Option<&str>) -> IconId {
    match name {
        Some("page") => IconId::File,
        Some("document" | "document-text") => IconId::FileText,
        Some("game-controller") => IconId::Gamepad2,
        Some("image") => IconId::Image,
        Some("barbell") => IconId::Dumbbell,
        Some("fitness") => IconId::Activity,
        Some("book") => IconId::BookOpen,
        Some("calendar") => IconId::Calendar,
        Some("planet") => IconId::Orbit,
        Some("library") => IconId::Library,
        Some("folder") => IconId::Folder,
        Some("sparkles") => IconId::Sparkles,
        Some("user" | "person") => IconId::User,
        _ => IconId::FileText,
    }
}

/// One sidebar row.
#[derive(Clone, Debug, PartialEq)]
pub struct SidebarRow {
    /// Stable DOM/test id (`sb-type-book_obj`, `sb-pin-<id>` …).
    pub id: String,
    pub label: String,
    pub icon: IconId,
    /// Object counter shown on type/collection rows.
    pub count: Option<usize>,
    /// Where the row navigates.
    pub route: Route,
    /// Pinned rows and the entry they point at (for unpin menus).
    pub entry_id: Option<String>,
}

/// A titled group. `title` is the Russian section header; `None` = the
/// unlabeled nav block at the top.
#[derive(Clone, Debug, PartialEq)]
pub struct SidebarSection {
    pub key: &'static str,
    pub title: Option<&'static str>,
    pub rows: Vec<SidebarRow>,
}

/// Build the full sidebar: nav → pinned → types → collections.
///
/// * Pinned: `pinned_ids` order (local state), entries resolved by id.
/// * Types: every `should_show_as_eden_collection` note type, sorted by
///   collection name, with a live object counter.
/// * Collections: `collection_obj` entries sorted by `updated_at` desc.
pub fn build_sidebar(
    note_types: &[NoteType],
    entries: &[Entry],
    pinned_ids: &[String],
) -> Vec<SidebarSection> {
    let mut sections = Vec::new();

    sections.push(SidebarSection {
        key: "nav",
        title: None,
        rows: vec![
            SidebarRow {
                id: "sb-nav-everything".into(),
                label: "Всё".into(),
                icon: IconId::LayoutGrid,
                count: Some(entries.iter().filter(|e| e.deleted_at.is_none()).count()),
                route: Route::Everything,
                entry_id: None,
            },
            SidebarRow {
                id: "sb-nav-diary".into(),
                label: "Дневник".into(),
                icon: IconId::NotebookPen,
                count: None,
                route: Route::Diary,
                entry_id: None,
            },
        ],
    });

    if !pinned_ids.is_empty() {
        let mut rows = Vec::new();
        for id in pinned_ids {
            let Some(entry) = entries.iter().find(|e| &e.id == id) else {
                continue;
            };
            rows.push(SidebarRow {
                id: format!("sb-pin-{id}"),
                label: entry_display_title(entry),
                icon: IconId::Pin,
                count: None,
                route: Route::Entry(id.clone()),
                entry_id: Some(id.clone()),
            });
        }
        sections.push(SidebarSection {
            key: "pinned",
            title: Some("Закреплённые"),
            rows,
        });
    }

    let mut type_rows: Vec<SidebarRow> = note_types
        .iter()
        .filter(|t| should_show_as_eden_collection(&t.id))
        .map(|t| {
            let count = entries
                .iter()
                .filter(|e| e.type_id.as_deref() == Some(t.id.as_str()))
                .filter(|e| e.deleted_at.is_none())
                .count();
            SidebarRow {
                id: format!("sb-type-{}", t.id),
                label: get_note_type_collection_name(Some(t)),
                icon: icon_for_name(t.icon.as_deref()),
                count: Some(count),
                route: Route::Collection(t.id.clone()),
                entry_id: None,
            }
        })
        .collect();
    type_rows.sort_by(|a, b| a.label.cmp(&b.label).then(a.id.cmp(&b.id)));
    if !type_rows.is_empty() {
        sections.push(SidebarSection {
            key: "types",
            title: Some("Типы"),
            rows: type_rows,
        });
    }

    let mut collections: Vec<&Entry> = entries
        .iter()
        .filter(|e| e.type_id.as_deref() == Some(SYSTEM_TYPE_COLLECTION_ID))
        .filter(|e| e.deleted_at.is_none())
        .filter(|e| collection_target_type_id(Some(e)).is_some())
        .collect();
    collections.sort_by_key(|e| std::cmp::Reverse(e.updated_at));
    if !collections.is_empty() {
        sections.push(SidebarSection {
            key: "collections",
            title: Some("Коллекции"),
            rows: collections
                .iter()
                .map(|e| {
                    let target = collection_target_type_id(Some(e)).unwrap_or_default();
                    let type_name = note_types
                        .iter()
                        .find(|t| t.id == target)
                        .map(|t| get_note_type_collection_name(Some(t)))
                        .unwrap_or_else(|| entry_display_title(e));
                    SidebarRow {
                        id: format!("sb-coll-{}", e.id),
                        label: type_name,
                        icon: IconId::Folder,
                        count: None,
                        route: Route::Collection(target),
                        entry_id: Some(e.id.clone()),
                    }
                })
                .collect(),
        });
    }

    sections
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_types_data::{SYSTEM_TYPE_BOOK_ID, SYSTEM_TYPE_IMAGE_ID};

    fn note_type(id: &str, name: &str, collection: &str) -> NoteType {
        NoteType {
            id: id.into(),
            name: name.into(),
            icon: Some("book".into()),
            ui_schema_json: Some(format!(r#"{{"collection_name":"{collection}"}}"#)),
            ..Default::default()
        }
    }

    #[test]
    fn sidebar_fixture_snapshot() {
        let types = vec![
            note_type(SYSTEM_TYPE_BOOK_ID, "Книга", "Книги"),
            note_type(SYSTEM_TYPE_IMAGE_ID, "Изображение", "Изображения"),
            note_type(SYSTEM_TYPE_COLLECTION_ID, "Коллекция", "Коллекции"),
        ];
        let entries = vec![
            Entry {
                id: "b1".into(),
                title: "Война и мир".into(),
                type_id: Some(SYSTEM_TYPE_BOOK_ID.into()),
                updated_at: 5,
                ..Default::default()
            },
            Entry {
                id: "b2".into(),
                title: "Анна".into(),
                type_id: Some(SYSTEM_TYPE_BOOK_ID.into()),
                updated_at: 3,
                ..Default::default()
            },
            Entry {
                id: "c1".into(),
                title: "Коллекция книг".into(),
                type_id: Some(SYSTEM_TYPE_COLLECTION_ID.into()),
                header_props_json: Some(r#"{"object_type_id":"book_obj"}"#.into()),
                updated_at: 9,
                ..Default::default()
            },
        ];
        let sections = build_sidebar(&types, &entries, &["b1".to_string()]);
        // Structure snapshot: nav → pinned → types → collections.
        let keys: Vec<&str> = sections.iter().map(|s| s.key).collect();
        assert_eq!(keys, ["nav", "pinned", "types", "collections"]);
        let nav = &sections[0];
        assert_eq!(nav.rows[0].label, "Всё");
        assert_eq!(nav.rows[0].count, Some(3));
        let pinned = &sections[1];
        assert_eq!(pinned.rows[0].label, "Война и мир");
        let types_s = &sections[2];
        // Sorted by collection name; collection_obj itself is hidden.
        let labels: Vec<&str> = types_s.rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["Изображения", "Книги"]);
        assert_eq!(types_s.rows[1].count, Some(2));
        assert_eq!(types_s.rows[1].route, Route::Collection("book_obj".into()));
        let collections = &sections[3];
        assert_eq!(collections.rows[0].label, "Книги");
        assert_eq!(
            collections.rows[0].route,
            Route::Collection("book_obj".into())
        );
    }

    #[test]
    fn empty_pinned_and_collections_drop_sections() {
        let sections = build_sidebar(&[], &[], &[]);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].key, "nav");
    }
}
