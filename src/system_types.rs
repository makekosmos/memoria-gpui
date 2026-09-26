//! Port of `src/lib/systemTypes.ts` — system-type lookup, hidden Eden
//! collections and `normalizeSystemNoteType` (system record wins over the
//! stored one; the legacy game type gets schema/UI upgrades).

use crate::model::NoteType;
use crate::system_types_data::{
    should_upgrade_legacy_game_presentation, should_upgrade_legacy_game_schema, system_types,
    GAME_HEADER_TEMPLATE_JSON, GAME_SCHEMA_JSON, GAME_UI_SCHEMA_JSON, SYSTEM_TYPE_BOOK_ID,
    SYSTEM_TYPE_COLLECTION_ID, SYSTEM_TYPE_EXERCISE_ID, SYSTEM_TYPE_GAME_ID, SYSTEM_TYPE_IMAGE_ID,
    SYSTEM_TYPE_JOURNAL_ID, SYSTEM_TYPE_NOTE_ID, SYSTEM_TYPE_PERSON_ID, SYSTEM_TYPE_WORKOUT_ID,
};

/// `HIDDEN_EDEN_COLLECTION_TYPE_IDS`.
fn is_hidden_collection_type(id: &str) -> bool {
    matches!(
        id,
        SYSTEM_TYPE_COLLECTION_ID
            | SYSTEM_TYPE_JOURNAL_ID
            | "blocklist_obj"
            | "tag_obj"
            | "task_obj"
            | "time_entry_obj"
    )
}

/// `normalizeSystemNoteType` — canonical system definitions always win over
/// what Engine stored; timestamps come from the stored record when set.
/// The game type upgrades only when the stored schema/presentation is legacy.
pub fn normalize_system_note_type(note_type: &NoteType) -> NoteType {
    let system = system_types().into_iter().find(|t| t.id == note_type.id);
    if let Some(system) = system {
        if note_type.id != SYSTEM_TYPE_GAME_ID {
            let mut out = system;
            out.created_at = if note_type.created_at != 0 {
                note_type.created_at
            } else {
                out.created_at
            };
            out.updated_at = if note_type.updated_at != 0 {
                note_type.updated_at
            } else {
                out.updated_at
            };
            return out;
        }
    }
    if note_type.id != SYSTEM_TYPE_GAME_ID {
        return note_type.clone();
    }
    if should_upgrade_legacy_game_schema(note_type)
        || note_type
            .ui_schema_json
            .as_deref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
        || should_upgrade_legacy_game_presentation(note_type)
    {
        let mut out = note_type.clone();
        out.schema_json = GAME_SCHEMA_JSON.into();
        out.header_template_json = GAME_HEADER_TEMPLATE_JSON.into();
        out.ui_schema_json = Some(GAME_UI_SCHEMA_JSON.into());
        return out;
    }
    note_type.clone()
}

/// `isSystemType`.
pub fn is_system_type(note_type_id: &str) -> bool {
    matches!(
        note_type_id,
        SYSTEM_TYPE_NOTE_ID
            | SYSTEM_TYPE_BOOK_ID
            | SYSTEM_TYPE_COLLECTION_ID
            | SYSTEM_TYPE_JOURNAL_ID
            | SYSTEM_TYPE_IMAGE_ID
            | SYSTEM_TYPE_PERSON_ID
            | SYSTEM_TYPE_GAME_ID
            | SYSTEM_TYPE_WORKOUT_ID
            | SYSTEM_TYPE_EXERCISE_ID
    )
}

/// `shouldShowAsEdenCollection`.
pub fn should_show_as_eden_collection(note_type_id: &str) -> bool {
    !is_hidden_collection_type(note_type_id)
}
