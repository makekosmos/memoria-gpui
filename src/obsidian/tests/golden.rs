//! Golden cross-check vs the Vue `obsidianVault.ts` implementation.
//!
//! `golden/vault-input.json` is the serialized file/image list built from the
//! committed `fixtures/obsidian-vault/` tree; `golden/vault-plan.json` is the
//! output of the real Vue `importObsidianVault` + `buildObsidianRelatedImportPlan`
//! (bundled from `memoria` SoT at `7ccbb9f` and run under Node — see
//! `gen-vault-plan.mjs` in the KOS-155 artifacts). This test replays the same
//! input through the Rust port and requires byte-level JSON equality.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use super::{custom_note_type, make_note_type};
use crate::obsidian::draft::{build_obsidian_related_import_plan, obsidian_note_id};
use crate::obsidian::vault::import_obsidian_vault;
use crate::obsidian::{ImportObsidianVaultArgs, ObsidianVaultImageFile, ObsidianVaultMarkdownFile};

const INPUT_JSON: &str = include_str!("golden/vault-input.json");
const PLAN_JSON: &str = include_str!("golden/vault-plan.json");

fn title_key(title: &str) -> String {
    title.trim().to_lowercase()
}

fn note_types() -> Vec<crate::model::NoteType> {
    vec![
        custom_note_type(),
        make_note_type("journal-type-id", "Дневник", "journal"),
    ]
}

#[test]
fn import_plan_matches_vue_golden_byte_for_byte() {
    let input: Value = serde_json::from_str(INPUT_JSON).unwrap();
    let golden: Value = serde_json::from_str(PLAN_JSON).unwrap();

    let files: Vec<ObsidianVaultMarkdownFile> =
        serde_json::from_value(input["files"].clone()).unwrap();
    let images: Vec<ObsidianVaultImageFile> =
        serde_json::from_value(input["images"].clone()).unwrap();

    let result = import_obsidian_vault(&ImportObsidianVaultArgs {
        files,
        images,
        note_types: note_types(),
        default_type_id: "custom-note".into(),
        image_type_id: "image_obj".into(),
        type_id_mapping: Map::new(),
        vault_identity: Some("vault".into()),
        existing_images: Vec::new(),
    });

    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        golden["result"],
        "Rust import plan diverges from the Vue golden"
    );
}

#[test]
fn related_plans_match_vue_golden() {
    let input: Value = serde_json::from_str(INPUT_JSON).unwrap();
    let golden: Value = serde_json::from_str(PLAN_JSON).unwrap();

    let files: Vec<ObsidianVaultMarkdownFile> =
        serde_json::from_value(input["files"].clone()).unwrap();
    let images: Vec<ObsidianVaultImageFile> =
        serde_json::from_value(input["images"].clone()).unwrap();
    let result = import_obsidian_vault(&ImportObsidianVaultArgs {
        files,
        images,
        note_types: note_types(),
        default_type_id: "custom-note".into(),
        image_type_id: "image_obj".into(),
        type_id_mapping: Map::new(),
        vault_identity: Some("vault".into()),
        existing_images: Vec::new(),
    });

    // `apply_import` id assignment: existing/draft id, else stable uuidv5.
    let entry_ids: Vec<String> = result
        .entries
        .iter()
        .map(|e| {
            e.id.clone()
                .unwrap_or_else(|| obsidian_note_id("vault", &e.relative_path))
        })
        .collect();
    let mut imported_title_ids: HashMap<String, String> = HashMap::new();
    for (e, id) in result.entries.iter().zip(&entry_ids) {
        let key = title_key(&e.title);
        if !key.is_empty() {
            imported_title_ids.entry(key).or_insert_with(|| id.clone());
        }
    }

    let plans: Vec<Value> = result
        .entries
        .iter()
        .zip(&entry_ids)
        .map(|(e, id)| {
            json!({
                "entryId": id,
                "plan": serde_json::to_value(
                    build_obsidian_related_import_plan(e, id, &imported_title_ids, None),
                )
                .unwrap(),
            })
        })
        .collect();
    assert_eq!(
        Value::Array(plans),
        golden["relatedPlans"],
        "Rust related-link plans diverge from the Vue golden"
    );
}
