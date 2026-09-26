//! Port of `src/lib/memoria-migration.ts` — Eden→Memoria compatibility.
//! Legacy values are copied, never removed; a failed copy can be retried
//! without losing the only surviving state.

use std::collections::HashSet;

use serde_json::Value;

pub const LEGACY_ARK_OBJECT_TYPE_IDS: [&str; 3] = ["note_obj", "task_obj", "collection_obj"];

/// `LEGACY_STORAGE_ALIASES` — canonical key → primary legacy key.
pub const LEGACY_STORAGE_ALIASES: [(&str, &str); 12] = [
    ("memoria-preferences", "eden-preferences"),
    ("memoria-settings.json", "eden-settings.json"),
    ("memoria-sidebar-config", "eden-extension-sidebar-config"),
    (
        "memoria-visible-object-type-ids",
        "eden-extension-visible-object-type-ids",
    ),
    ("memoria-zoom", "eden-extension-zoom"),
    ("memoria-legacy-zoom", "eden-zoom"),
    ("memoria-legacy-theme", "eden-theme"),
    ("memoria:layout:zenMode", "eden:layout:zenMode"),
    ("memoria:nav:lastScreen", "eden:nav:lastScreen"),
    ("memoria:nav:lastEntryId", "eden:nav:lastEntryId"),
    ("memoria-theme", "vite-ui-theme"),
    ("memoria-bubbles", "eden-bubble-diary-local-bubbles"),
];

/// `LEGACY_STORAGE_FALLBACKS` — extra legacy keys consulted in order.
pub const LEGACY_STORAGE_FALLBACKS: [(&str, &[&str]); 2] = [
    ("memoria-zoom", &["eden-extension-zoom", "eden-zoom"]),
    ("memoria-theme", &["vite-ui-theme", "eden-theme"]),
];

pub const LEGACY_COMMAND_PREFIX: &str = "eden:";
pub const MEMORIA_COMMAND_PREFIX: &str = "memoria:";

/// `compatibilityArkTypeIds` — canonical + legacy + selected + discovered,
/// deduped with first-occurrence order.
pub fn compatibility_ark_type_ids(selected: &[String], discovered: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for id in ["com.kosmos.note"]
        .iter()
        .map(|s| s.to_string())
        .chain(LEGACY_ARK_OBJECT_TYPE_IDS.iter().map(|s| s.to_string()))
        .chain(selected.iter().cloned())
        .chain(discovered.iter().cloned())
    {
        if seen.insert(id.clone()) {
            out.push(id);
        }
    }
    out
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MigrationResult {
    pub copied: Vec<String>,
    pub failed: Vec<String>,
}

/// Minimal `Storage` shape (`getItem`/`setItem`) so the pure logic is testable.
pub trait KeyValueStorage {
    fn get_item(&self, key: &str) -> Option<String>;
    /// Return Err on write failure (mapped to `result.failed`).
    fn set_item(&mut self, key: &str, value: &str) -> Result<(), String>;
}

/// `migrateLegacyStorage` — copies legacy values under canonical keys when the
/// canonical key is absent.
pub fn migrate_legacy_storage<S: KeyValueStorage>(storage: Option<&mut S>) -> MigrationResult {
    let mut result = MigrationResult::default();
    let Some(storage) = storage else {
        return result;
    };

    let mut migrations: Vec<(&str, Vec<&str>)> = LEGACY_STORAGE_ALIASES
        .iter()
        .map(|(current, legacy)| (*current, vec![*legacy]))
        .collect();
    for (current, legacy_keys) in LEGACY_STORAGE_FALLBACKS {
        let entry = migrations
            .iter_mut()
            .find(|(key, _)| *key == current)
            .map(|(_, keys)| keys);
        if let Some(existing) = entry {
            for key in legacy_keys {
                if !existing.contains(key) {
                    existing.push(*key);
                }
            }
        }
    }

    for (current, legacy_keys) in migrations {
        if storage.get_item(current).is_some() {
            continue;
        }
        let value = legacy_keys
            .iter()
            .find_map(|legacy| storage.get_item(legacy));
        let Some(value) = value else { continue };
        match storage.set_item(current, &value) {
            Ok(()) => result.copied.push(current.to_string()),
            Err(_) => result.failed.push(current.to_string()),
        }
    }
    result
}

/// `UserDataMigrationBridge` — named JSON snapshots in user data.
pub trait UserDataBridge {
    fn read_json(&self, name: &str) -> Result<Option<Value>, String>;
    fn write_json(&mut self, name: &str, value: Value) -> Result<(), String>;
}

/// `migrateLegacyUserData` — the canonical file is authoritative; never
/// overwrite user changes with the older Eden snapshot.
pub fn migrate_legacy_user_data<B: UserDataBridge>(bridge: Option<&mut B>) -> MigrationResult {
    const KEY: &str = "memoria-settings.json";
    let mut result = MigrationResult::default();
    let Some(bridge) = bridge else { return result };
    let outcome = (|| -> Result<(), String> {
        if bridge.read_json(KEY)?.is_some() {
            return Ok(());
        }
        if let Some(value) = bridge.read_json("eden-settings.json")? {
            bridge.write_json(KEY, value)?;
            result.copied.push(KEY.into());
        }
        Ok(())
    })();
    if outcome.is_err() {
        result.failed.push(KEY.into());
    }
    result
}

/// `canonicalCommandChannel`.
pub fn canonical_command_channel(channel: &str) -> String {
    channel
        .strip_prefix(LEGACY_COMMAND_PREFIX)
        .map(|rest| format!("{MEMORIA_COMMAND_PREFIX}{rest}"))
        .unwrap_or_else(|| channel.to_string())
}
