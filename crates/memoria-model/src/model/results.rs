//! Vue result-record types: `DeleteEntryResult`, `SaveNoteTypeResult`,
//! `VaultStorageInfo`

use super::NoteType;
use serde::{Deserialize, Serialize};
/// Vue `DeleteEntryResult`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteEntryResult {
    pub ok: bool,
    #[serde(default, rename = "entryId", skip_serializing_if = "Option::is_none")]
    pub entry_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Vue `SaveNoteTypeResult`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SaveNoteTypeResult {
    Ok {
        ok: bool,
        #[serde(rename = "noteType")]
        note_type: Box<NoteType>,
    },
    Failed {
        ok: bool,
        reason: String,
        message: String,
    },
}

impl SaveNoteTypeResult {
    pub fn ok(note_type: NoteType) -> Self {
        Self::Ok {
            ok: true,
            note_type: Box::new(note_type),
        }
    }

    pub fn failed(reason: &str, message: impl Into<String>) -> Self {
        Self::Failed {
            ok: false,
            reason: reason.into(),
            message: message.into(),
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok { .. })
    }
}

/// Vue `VaultStorageInfo`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VaultStorageInfo {
    #[serde(default, rename = "textBytes")]
    pub text_bytes: usize,
    #[serde(default, rename = "trashBytes")]
    pub trash_bytes: usize,
    #[serde(default, rename = "dbBytes")]
    pub db_bytes: usize,
    #[serde(default, rename = "vaultBytes")]
    pub vault_bytes: usize,
    #[serde(default, rename = "entryCount")]
    pub entry_count: usize,
    #[serde(default, rename = "trashCount")]
    pub trash_count: usize,
}
