//! Port of `src/store/liveListFilter.ts` — should a live-updated object join
//! the Eden list? Mirrors `listEntries()` filtering.

use serde_json::Value;

use crate::mapping::read_memoria_props;

const COLLECTION_TYPE_ID: &str = "collection_obj";
const JOURNAL_TYPE_ID: &str = "system-type-journal";

/// `HIDDEN_COLLECTION_TARGET_IDS` — mirrors `HIDDEN_EDEN_COLLECTION_TYPE_IDS`.
fn is_hidden_collection_target(id: &str) -> bool {
    matches!(
        id,
        COLLECTION_TYPE_ID | "blocklist_obj" | "tag_obj" | "task_obj" | "time_entry_obj"
    )
}

/// `shouldIncludeTypeInEdenListForLiveUpdate` — collections additionally pass
/// through the `object_type_id` visibility filter.
pub fn should_include_type_in_eden_list_for_live_update(
    type_id: &str,
    props_json: &Value,
    visible_type_ids: &[String],
) -> bool {
    let props = read_memoria_props(props_json);

    if type_id == JOURNAL_TYPE_ID
        && props.get("entry_kind").and_then(Value::as_str) == Some("bubble")
    {
        return false;
    }

    if type_id == COLLECTION_TYPE_ID {
        let Some(object_type_id) = props.get("object_type_id").and_then(Value::as_str) else {
            return false;
        };
        if object_type_id.is_empty() || is_hidden_collection_target(object_type_id) {
            return false;
        }
        if !visible_type_ids.is_empty() && !visible_type_ids.iter().any(|id| id == object_type_id) {
            return false;
        }
        return true;
    }

    if visible_type_ids.is_empty() {
        return true;
    }
    visible_type_ids.iter().any(|id| id == type_id)
}
