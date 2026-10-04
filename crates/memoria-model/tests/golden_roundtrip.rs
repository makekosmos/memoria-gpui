//! Golden round-trip: every record in `fixtures/ark-snapshot.json` must
//! survive decode → encode byte-for-byte (unknown keys ride along in `extra`
//! and preserve wire order via serde_json's `preserve_order`).

use memoria_model::model::{ArkObjectLink, ArkObjectRecord};
use serde_json::Value;

const FIXTURE: &str = include_str!("../fixtures/ark-snapshot.json");

#[test]
fn every_object_decodes_and_reencodes_identically() {
    let doc: Value = serde_json::from_str(FIXTURE).unwrap();
    for object in doc["objects"].as_array().unwrap() {
        let record: ArkObjectRecord = serde_json::from_value(object.clone())
            .unwrap_or_else(|e| panic!("decode {}: {e}", object["id"]));
        let encoded = serde_json::to_value(&record).unwrap();
        assert_eq!(&encoded, object, "object {}", record.id);
    }
    for link in doc["links"].as_array().unwrap() {
        let record: ArkObjectLink = serde_json::from_value(link.clone())
            .unwrap_or_else(|e| panic!("decode {}: {e}", link["id"]));
        let encoded = serde_json::to_value(&record).unwrap();
        assert_eq!(&encoded, link, "link {}", record.id);
    }
}

#[test]
fn whole_document_reencodes_byte_equal() {
    let doc: Value = serde_json::from_str(FIXTURE).unwrap();
    let encoded = serde_json::to_string_pretty(&doc).unwrap();
    assert_eq!(
        format!("{encoded}\n"),
        FIXTURE,
        "key order / formatting must match JSON.stringify(_, null, 2)"
    );
}
