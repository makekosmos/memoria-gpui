//! App-local persisted state — the `usePreferences` + localStorage surface.
//!
//! Vue writes preferences through the Kepler host `userData` bridge
//! (`memoria-settings.json`, legacy `eden-settings.json`) and mirrors to a
//! `memoria-preferences` localStorage blob. There is no host bridge in the
//! GPUI app, so the Engine data dir is the equivalent per-user store (the
//! transport already reads `engine.lock.json` from it):
//!
//!   <Engine data dir>/memoria-settings.json   — authoritative prefs
//!   <Engine data dir>/eden-settings.json      — legacy read fallback
//!   <Engine data dir>/memoria-local-state.json — localStorage-analog blob
//!     { preferences, pinned_entry_ids, sidebar_collapsed }
//!
//! Load order mirrors `hydrate()`: local blob first (instant), then the
//! settings file wins if present (authoritative), legacy file last.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const PREFS_FILE_NAME: &str = "memoria-settings.json";
pub const LEGACY_PREFS_FILE_NAME: &str = "eden-settings.json";
pub const LOCAL_STATE_FILE_NAME: &str = "memoria-local-state.json";

/// `EdenPreferences` — camelCase keys match the JSON the Vue bridge writes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preferences {
    /// `spellcheckEnabled` — false by default.
    #[serde(default, rename = "spellcheckEnabled")]
    pub spellcheck_enabled: bool,
    /// `readerModeEnabled` — false by default.
    #[serde(default, rename = "readerModeEnabled")]
    pub reader_mode_enabled: bool,
}

impl Preferences {
    /// `mergeIntoState` — only boolean members merge; other keys are ignored.
    pub fn merge_patch(&mut self, patch: &serde_json::Value) {
        if let Some(v) = patch.get("spellcheckEnabled").and_then(|v| v.as_bool()) {
            self.spellcheck_enabled = v;
        }
        if let Some(v) = patch.get("readerModeEnabled").and_then(|v| v.as_bool()) {
            self.reader_mode_enabled = v;
        }
    }
}

/// LocalStorage-analog blob (`memoria-local-state.json`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LocalState {
    /// `memoria-preferences` mirror (sync fallback under the authoritative file).
    #[serde(default)]
    pub preferences: Preferences,
    /// Sidebar «Закреплённые» — local per-workspace pin list.
    #[serde(default)]
    pub pinned_entry_ids: Vec<String>,
    /// Sidebar collapse state (imago sidebar width toggle).
    #[serde(default)]
    pub sidebar_collapsed: bool,
    /// `visibleObjectTypeIds` — GeneralSettings type filter; empty = all.
    #[serde(default)]
    pub visible_object_type_ids: Vec<String>,
    /// Anything else lands here round-trip (forward-compat like `extra`).
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

pub fn prefs_path(data_dir: &Path) -> PathBuf {
    data_dir.join(PREFS_FILE_NAME)
}
pub fn legacy_prefs_path(data_dir: &Path) -> PathBuf {
    data_dir.join(LEGACY_PREFS_FILE_NAME)
}
pub fn local_state_path(data_dir: &Path) -> PathBuf {
    data_dir.join(LOCAL_STATE_FILE_NAME)
}

fn read_json_file(path: &Path) -> Option<serde_json::Value> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_json_file(path: &Path, value: &serde_json::Value) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(value)?)
}

/// `hydrate()` — loads local state then merges the authoritative prefs file
/// (Vue order: local blob first, then `memoria-settings.json`, with
/// `eden-settings.json` only when the current file is absent).
pub fn load_local_state(data_dir: &Path) -> LocalState {
    let mut state: LocalState = read_json_file(&local_state_path(data_dir))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    // Vue `hydrate()`: userData.readJson(memoria-settings.json) ??
    // readJson(legacy). The legacy file applies only when the current file
    // is entirely absent.
    let remote = read_json_file(&prefs_path(data_dir))
        .or_else(|| read_json_file(&legacy_prefs_path(data_dir)));
    if let Some(v) = remote {
        state.preferences.merge_patch(&v);
    }
    state
}

/// Persist watcher's write — `memoria-settings.json` authoritative plus the
/// local blob mirror (`writeLocalStorage` equivalent).
pub fn save_local_state(data_dir: &Path, state: &LocalState) {
    let prefs = serde_json::to_value(&state.preferences).unwrap_or_default();
    let _ = write_json_file(&prefs_path(data_dir), &prefs);
    if let Ok(blob) = serde_json::to_value(state) {
        let _ = write_json_file(&local_state_path(data_dir), &blob);
    }
}

/// `visibleObjectTypeSet` — the effective visible type id set; an empty
/// stored list means "all types" (Vue treats [] as "load everything").
pub fn visible_type_set(
    stored: &[String],
    all_ids: &[String],
) -> std::collections::BTreeSet<String> {
    if stored.is_empty() {
        all_ids.iter().cloned().collect()
    } else {
        stored.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn prefs_merge_patch_bool_only() {
        // preferences.test.ts semantics: only boolean values merge.
        let mut p = Preferences::default();
        p.merge_patch(&json!({"spellcheckEnabled": true, "readerModeEnabled": "yes"}));
        assert!(p.spellcheck_enabled);
        assert!(!p.reader_mode_enabled);
        p.merge_patch(&json!({"readerModeEnabled": true}));
        assert!(p.reader_mode_enabled);
    }

    #[test]
    fn load_order_prefers_settings_file_over_legacy() {
        let dir = tempfile::tempdir().unwrap();
        write_json_file(
            &dir.path().join(PREFS_FILE_NAME),
            &json!({"spellcheckEnabled": true}),
        )
        .unwrap();
        write_json_file(
            &dir.path().join(LEGACY_PREFS_FILE_NAME),
            &json!({"spellcheckEnabled": false, "readerModeEnabled": true}),
        )
        .unwrap();
        let state = load_local_state(dir.path());
        // `readJson(current) ?? readJson(legacy)` — the whole legacy file is
        // skipped when the current file exists.
        assert!(state.preferences.spellcheck_enabled);
        assert!(!state.preferences.reader_mode_enabled);
    }

    #[test]
    fn legacy_only_loads_when_current_missing() {
        let dir = tempfile::tempdir().unwrap();
        write_json_file(
            &dir.path().join(LEGACY_PREFS_FILE_NAME),
            &json!({"spellcheckEnabled": true}),
        )
        .unwrap();
        let state = load_local_state(dir.path());
        assert!(state.preferences.spellcheck_enabled);
    }

    #[test]
    fn save_roundtrip_both_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = LocalState {
            preferences: Preferences {
                spellcheck_enabled: true,
                reader_mode_enabled: false,
            },
            pinned_entry_ids: vec!["e1".into(), "e2".into()],
            sidebar_collapsed: true,
            visible_object_type_ids: vec![],
            extra: Default::default(),
        };
        save_local_state(dir.path(), &state);
        let reloaded = load_local_state(dir.path());
        assert_eq!(reloaded.preferences, state.preferences);
        assert_eq!(reloaded.pinned_entry_ids, state.pinned_entry_ids);
        assert!(reloaded.sidebar_collapsed);
        state.pinned_entry_ids.clear();
        save_local_state(dir.path(), &state);
        assert!(load_local_state(dir.path()).pinned_entry_ids.is_empty());
    }
}
