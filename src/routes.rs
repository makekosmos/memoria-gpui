//! Routes + sticker-route helpers — ports of `kepler-navigation.ts` and
//! `stickerRoutes.ts` (Vue SoT). Route ids are the stable strings used by
//! navigation history and sticker windows.

use serde::{Deserialize, Serialize};

/// Vue `SettingsPage` tab id (`isSettingsTab`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettingsTab {
    General,
    Export,
    Trash,
}

impl SettingsTab {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Export => "export",
            Self::Trash => "trash",
        }
    }

    /// Vue `isSettingsTab(value)`.
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "general" => Self::General,
            "export" => Self::Export,
            "trash" => Self::Trash,
            _ => return None,
        })
    }

    /// Russian title shown in the settings tab list.
    pub fn title(self) -> &'static str {
        match self {
            Self::General => "Общие",
            Self::Export => "Экспорт",
            Self::Trash => "Корзина",
        }
    }
}

/// `isSettingsTab` — accepts any value, mirroring the Vue guard.
pub fn is_settings_tab(value: &str) -> bool {
    SettingsTab::parse(value).is_some()
}

/// App shell route. Vue keeps `activeScreen`/`currentEntry`/`activeNoteTypeId`
/// as separate refs; a single enum keeps history snapshots consistent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    /// «Всё» — everything grid (`activeScreen === "notes"`, no currentEntry).
    Everything,
    /// «Дневник» — bubble diary timeline (M6 placeholder surface).
    Diary,
    /// `type-collection` — object table for one note type.
    Collection(String),
    /// One object open — note reader, image view or typed header by type.
    Entry(String),
    /// Settings page on a tab.
    Settings(SettingsTab),
}

impl Route {
    /// Stable history key — dedupe + test assertions.
    pub fn key(&self) -> String {
        match self {
            Route::Everything => "everything".into(),
            Route::Diary => "diary".into(),
            Route::Collection(t) => format!("collection:{t}"),
            Route::Entry(id) => format!("entry:{id}"),
            Route::Settings(tab) => format!("settings:{}", tab.as_str()),
        }
    }

    /// Parse a `Route::key` back (tests/debug).
    pub fn parse_key(key: &str) -> Option<Route> {
        if key == "everything" {
            return Some(Route::Everything);
        }
        if key == "diary" {
            return Some(Route::Diary);
        }
        if let Some(id) = key.strip_prefix("collection:") {
            return Some(Route::Collection(id.to_string()));
        }
        if let Some(id) = key.strip_prefix("entry:") {
            return Some(Route::Entry(id.to_string()));
        }
        if let Some(tab) = key.strip_prefix("settings:") {
            return SettingsTab::parse(tab).map(Route::Settings);
        }
        None
    }
}

const NOTE_ROUTE_PREFIX: &str = "/note/";

/// `parseEntryRouteId` — Vue guards: no `/?#` encoded, decoded id ≤200
/// chars, no `\`/whitespace/`?`/`#`/control chars inside it.
pub(crate) fn parse_entry_route_id(route: &str, prefix: &str) -> Option<String> {
    let encoded = route.strip_prefix(prefix)?;
    if encoded.is_empty() || encoded.contains(['/', '?', '#']) {
        return None;
    }
    let entry_id = crate::sticker_route::decode_uri_component(encoded)?;
    if entry_id.is_empty()
        || entry_id.chars().count() > crate::sticker_route::MAX_ENTRY_ID_LENGTH
        || entry_id
            .chars()
            .any(|c| c == '\\' || c.is_whitespace() || c == '?' || c == '#' || c.is_control())
    {
        return None;
    }
    Some(entry_id)
}

/// `parseKeplerRoute` → entry id.
pub fn parse_note_route(route: &str) -> Option<String> {
    parse_entry_route_id(route, NOTE_ROUTE_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_tab_roundtrip() {
        for tab in [
            SettingsTab::General,
            SettingsTab::Export,
            SettingsTab::Trash,
        ] {
            assert_eq!(SettingsTab::parse(tab.as_str()), Some(tab));
        }
        assert!(is_settings_tab("general"));
        assert!(!is_settings_tab("notes"));
        assert_eq!(SettingsTab::parse(""), None);
    }

    #[test]
    fn route_key_roundtrip() {
        let routes = [
            Route::Everything,
            Route::Diary,
            Route::Collection("book_obj".into()),
            Route::Entry("abc-1".into()),
            Route::Settings(SettingsTab::Trash),
        ];
        for r in routes {
            assert_eq!(Route::parse_key(&r.key()), Some(r));
        }
        assert_eq!(Route::parse_key("bogus"), None);
        assert_eq!(Route::parse_key("settings:bogus"), None);
    }
}
