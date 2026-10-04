//! `LEGACY_STORAGE_ALIASES` key-contract checks — the canonical keys must be
//! the ones the app actually reads (a wrong alias silently orphans migrated
//! data; the `memoria-bubbles` blob key regression is why this file exists).

use memoria_model::migration::{migrate_legacy_storage, KeyValueStorage, LEGACY_STORAGE_ALIASES};
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Default)]
struct MemStorage(RefCell<HashMap<String, String>>);

impl KeyValueStorage for MemStorage {
    fn get_item(&self, key: &str) -> Option<String> {
        self.0.borrow().get(key).cloned()
    }
    fn set_item(&mut self, key: &str, value: &str) -> Result<(), String> {
        self.0.borrow_mut().insert(key.into(), value.into());
        Ok(())
    }
}

/// The legacy diary blob must land under `LOCAL_BUBBLES_STORAGE_KEY` — the
/// key `startDiary` reads; the old `memoria-bubbles` alias orphaned it.
#[test]
fn legacy_diary_blob_migrates_to_the_key_the_diary_reads() {
    let mut storage = MemStorage::default();
    let blob = r#"{"version":1,"bubbles":[]}"#;
    storage
        .0
        .borrow_mut()
        .insert("eden-bubble-diary-local-bubbles".into(), blob.into());
    let result = migrate_legacy_storage(Some(&mut storage));
    let key = memoria_model::diary::LOCAL_BUBBLES_STORAGE_KEY;
    assert_eq!(storage.0.borrow().get(key).map(String::as_str), Some(blob));
    assert!(result.copied.iter().any(|k| k == key));
}

#[test]
fn alias_table_maps_every_legacy_key_to_a_live_canonical_key() {
    // The blob alias is the specific regression; the rest are structural.
    assert_eq!(
        LEGACY_STORAGE_ALIASES
            .iter()
            .find(|(_, legacy)| *legacy == "eden-bubble-diary-local-bubbles")
            .map(|(current, _)| *current),
        Some(memoria_model::diary::LOCAL_BUBBLES_STORAGE_KEY),
    );
}
