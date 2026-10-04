//! Port of `tests/memoriaMigration.test.ts` — the pure parts. The Vue-only
//! manifest/compatibility.json assertions are package-contract checks with no
//! Rust manifest counterpart (documented in PORT_TESTS.md).

use memoria_model::mapping::{map_ark_object_to_entry, map_entry_to_ark_object};
use memoria_model::migration::{
    canonical_command_channel, compatibility_ark_type_ids, migrate_legacy_storage,
    migrate_legacy_user_data, KeyValueStorage, UserDataBridge, LEGACY_STORAGE_ALIASES,
};
use memoria_model::model::ArkObjectRecord;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashMap;

fn ark_object(props_json: Value) -> ArkObjectRecord {
    serde_json::from_value(json!({
        "id": "legacy-note",
        "typeId": "com.kosmos.note",
        "title": "Legacy note",
        "contentJson": { "type": "markdown", "version": 1, "text": "body" },
        "propsJson": props_json,
        "createdAt": "2026-01-01T00:00:00.000Z",
        "updatedAt": "2026-01-01T00:00:00.000Z",
        "deletedAt": null,
    }))
    .unwrap()
}

#[test]
fn legacy_metadata_reads_and_writes_the_canonical_extension_bag() {
    let legacy = map_ark_object_to_entry(
        &ark_object(json!({
            "description": "Legacy description",
            "memoria_type_id": "note_obj",
            "source_vault": "C:/Vault",
            "source_relative_path": "Notes/legacy.md",
        })),
        &[],
        None,
    );

    assert_eq!(legacy.type_id.as_deref(), Some("note_obj"));
    let props: Value = serde_json::from_str(legacy.header_props_json.as_deref().unwrap()).unwrap();
    assert_eq!(
        props,
        json!({
            "description": "Legacy description",
            "source_vault": "C:/Vault",
            "source_relative_path": "Notes/legacy.md",
            "related_notes": [],
        })
    );

    let canonical = map_entry_to_ark_object(&legacy);
    assert_eq!(canonical.type_version.as_deref(), Some("1.0.0"));
    assert_eq!(
        canonical.content_json,
        json!({
            "type": "doc",
            "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "body" }] }],
        })
    );
    assert_eq!(
        canonical.props_json,
        json!({
            "description": "Legacy description",
            "extensions": {
                "source_vault": "C:/Vault",
                "source_relative_path": "Notes/legacy.md",
                "memoria_type_id": "note_obj",
            },
        })
    );
}

#[test]
fn empty_and_legacy_content_map_to_canonical_docs_and_round_trip() {
    let legacy = map_ark_object_to_entry(&ark_object(json!({})), &[], None);
    let canonical = map_entry_to_ark_object(&legacy);
    assert_eq!(
        canonical.content_json,
        json!({
            "type": "doc",
            "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "body" }] }],
        })
    );

    let mut empty = legacy.clone();
    empty.id = "empty-content".into();
    empty.content_json = String::new();
    let empty = map_entry_to_ark_object(&empty);
    assert_eq!(
        empty.content_json,
        json!({ "type": "doc", "content": [{ "type": "paragraph" }] })
    );

    let round_tripped = map_entry_to_ark_object(&map_ark_object_to_entry(&canonical, &[], None));
    assert_eq!(round_tripped.content_json, canonical.content_json);
}

#[test]
fn compatibility_ids_cover_canonical_legacy_selected_and_discovered() {
    let ids = compatibility_ark_type_ids(
        &["custom_obj".to_string()],
        &["another_obj".to_string(), "task_obj".to_string()],
    );
    assert_eq!(
        ids,
        [
            "com.kosmos.note",
            "note_obj",
            "task_obj",
            "collection_obj",
            "custom_obj",
            "another_obj",
        ]
    );
}

#[test]
fn canonical_command_channel_rewrites_the_legacy_prefix() {
    assert_eq!(
        canonical_command_channel("eden:cmd:note:create"),
        "memoria:cmd:note:create"
    );
    assert_eq!(
        canonical_command_channel("memoria:cmd:note:open"),
        "memoria:cmd:note:open"
    );
}

// --- storage/user-data migration -------------------------------------------

#[derive(Default)]
struct MemStorage(RefCell<HashMap<String, String>>);

impl KeyValueStorage for MemStorage {
    fn get_item(&self, key: &str) -> Option<String> {
        self.0.borrow().get(key).cloned()
    }
    fn set_item(&mut self, key: &str, value: &str) -> Result<(), String> {
        if key == "memoria-preferences" && self.0.borrow().contains_key("__fail") {
            return Err("quota".into());
        }
        self.0.borrow_mut().insert(key.into(), value.into());
        Ok(())
    }
}

#[test]
fn legacy_storage_copies_without_deleting_and_is_restart_safe() {
    let mut storage = MemStorage::default();
    storage.0.borrow_mut().insert(
        "eden-preferences".into(),
        "{\"readerModeEnabled\":true}".into(),
    );
    storage
        .0
        .borrow_mut()
        .insert("eden:layout:zenMode".into(), "1".into());

    let first = migrate_legacy_storage(Some(&mut storage));
    let second = migrate_legacy_storage(Some(&mut storage));
    assert!(first.copied.contains(&"memoria-preferences".to_string()));
    assert!(first.copied.contains(&"memoria:layout:zenMode".to_string()));
    assert!(second.copied.is_empty());
    assert_eq!(
        storage.0.borrow().get("eden-preferences").unwrap(),
        "{\"readerModeEnabled\":true}"
    );
    assert_eq!(
        storage.0.borrow().get("memoria-preferences").unwrap(),
        "{\"readerModeEnabled\":true}"
    );
}

#[test]
fn zoom_and_theme_aliases_migrate_without_overwriting_canonical() {
    let mut storage = MemStorage::default();
    storage
        .0
        .borrow_mut()
        .insert("eden-extension-zoom".into(), "1.2".into());
    storage
        .0
        .borrow_mut()
        .insert("eden-zoom".into(), "1.1".into());
    storage
        .0
        .borrow_mut()
        .insert("eden-theme".into(), "light".into());

    let first = migrate_legacy_storage(Some(&mut storage));
    let second = migrate_legacy_storage(Some(&mut storage));

    assert!(first.copied.contains(&"memoria-zoom".to_string()));
    assert!(first.copied.contains(&"memoria-theme".to_string()));
    assert!(second.copied.is_empty());
    assert_eq!(storage.0.borrow().get("memoria-zoom").unwrap(), "1.2");
    assert_eq!(storage.0.borrow().get("memoria-theme").unwrap(), "light");
    assert_eq!(storage.0.borrow().get("eden-zoom").unwrap(), "1.1");
    assert_eq!(storage.0.borrow().get("eden-theme").unwrap(), "light");
}

#[test]
fn partial_local_storage_failure_reports_failed_and_copies_rest() {
    let mut storage = MemStorage::default();
    storage
        .0
        .borrow_mut()
        .insert("eden-preferences".into(), "legacy".into());
    storage.0.borrow_mut().insert("__fail".into(), "1".into());

    let result = migrate_legacy_storage(Some(&mut storage));
    assert_eq!(result.failed, vec!["memoria-preferences".to_string()]);
    assert!(!storage.0.borrow().contains_key("memoria-preferences"));
    assert_eq!(LEGACY_STORAGE_ALIASES[0].0, "memoria-preferences");
    assert_eq!(LEGACY_STORAGE_ALIASES[0].1, "eden-preferences");
}

// --- user-data migration -----------------------------------------------------

struct UserData {
    files: RefCell<HashMap<String, Value>>,
    fail_next_write: bool,
}

impl UserDataBridge for UserData {
    fn read_json(&self, name: &str) -> Result<Option<Value>, String> {
        Ok(self.files.borrow().get(name).cloned())
    }
    fn write_json(&mut self, name: &str, value: Value) -> Result<(), String> {
        if self.fail_next_write {
            self.fail_next_write = false;
            return Err("temporary failure".into());
        }
        self.files.borrow_mut().insert(name.into(), value);
        Ok(())
    }
}

#[test]
fn user_data_migration_retries_after_failed_write() {
    let mut bridge = UserData {
        files: RefCell::new(HashMap::from([(
            "eden-settings.json".into(),
            json!({ "readerModeEnabled": true }),
        )])),
        fail_next_write: true,
    };
    let failed = migrate_legacy_user_data(Some(&mut bridge));
    let retried = migrate_legacy_user_data(Some(&mut bridge));
    assert_eq!(failed.failed, vec!["memoria-settings.json".to_string()]);
    assert_eq!(retried.copied, vec!["memoria-settings.json".to_string()]);
    assert_eq!(
        bridge.files.borrow()["eden-settings.json"],
        json!({ "readerModeEnabled": true })
    );
    assert_eq!(
        bridge.files.borrow()["memoria-settings.json"],
        json!({ "readerModeEnabled": true })
    );
}

#[test]
fn canonical_user_data_is_authoritative_on_restart() {
    let mut bridge = UserData {
        files: RefCell::new(HashMap::from([
            (
                "eden-settings.json".into(),
                json!({ "readerModeEnabled": false }),
            ),
            (
                "memoria-settings.json".into(),
                json!({ "readerModeEnabled": true }),
            ),
        ])),
        fail_next_write: false,
    };
    let first = migrate_legacy_user_data(Some(&mut bridge));
    let second = migrate_legacy_user_data(Some(&mut bridge));
    assert_eq!(first, Default::default());
    assert_eq!(second, Default::default());
    assert_eq!(
        bridge.files.borrow()["memoria-settings.json"],
        json!({ "readerModeEnabled": true })
    );
    assert_eq!(
        bridge.files.borrow()["eden-settings.json"],
        json!({ "readerModeEnabled": false })
    );
}
