//! `EntryConflictBanner` port — banner render + the Vue conflict actions
//! (accept remote / keep local copy / copy local / recheck / cancel).
use gpui::Context;

use memoria_model::content::read_entry_markdown;
use memoria_model::entry_conflicts::{
    conflict_for_entry, unresolved_entry_conflicts, EntryConflict, EntryConflictState,
};
use memoria_model::store::Command;

use super::types::ConflictOp;
use super::Memoria;

impl Memoria {
    /// `recheckConflicts()` on navigation — replay the pending set for the
    /// entry being opened (`navigateTo` calls it after landing).
    pub(crate) fn start_conflict_recheck(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.current.as_ref().map(|e| e.id.clone()) {
            if let Some(c) = conflict_for_entry(&self.conflicts.conflicts, &id) {
                self.conflict_op = Some((c.id.clone(), ConflictOp::Recheck));
                self.send(
                    Command::LoadEntry {
                        id,
                        content_only: true,
                    },
                    cx,
                );
            }
        }
    }

    /// `acceptConflict` — async verify happens in `conflict_entry_reply`.
    pub(crate) fn send_accept_remote(&mut self, conflict_id: String, cx: &mut Context<Self>) {
        let Some(c) = self
            .conflicts
            .conflicts
            .iter()
            .find(|c| c.id == conflict_id)
            .cloned()
        else {
            return;
        };
        if c.state.is_closed() {
            return;
        }
        self.conflict_op = Some((conflict_id, ConflictOp::Accept));
        self.send(
            Command::LoadEntry {
                id: c.entry_id,
                content_only: true,
            },
            cx,
        );
    }

    /// `keepConflictLocalAsCopy` — save `<title> (локальная копия …)` then
    /// accept the remote revision.
    pub(crate) fn keep_conflict_copy(&mut self, conflict_id: String, cx: &mut Context<Self>) {
        let Some(c) = self
            .conflicts
            .conflicts
            .iter()
            .find(|c| c.id == conflict_id)
            .cloned()
        else {
            return;
        };
        let suffix = uuid::Uuid::new_v4().to_string();
        let mut copy = c.local.clone();
        copy.id = uuid::Uuid::new_v4().to_string();
        copy.title = format!(
            "{} (локальная копия {})",
            if c.local.title.is_empty() {
                "Без названия"
            } else {
                &c.local.title
            },
            &suffix[..8]
        );
        copy.created_at = memoria_model::time::now_millis();
        copy.updated_at = copy.created_at;
        copy.deleted_at = None;
        copy.content_loaded = Some(true);
        self.pending_copy_save = Some((conflict_id, copy.id.clone()));
        self.send(Command::SaveEntry(Box::new(copy)), cx);
    }

    /// `copyConflictLocal` — local markdown to the clipboard.
    pub(crate) fn copy_conflict_local(&mut self, conflict_id: &str, cx: &mut Context<Self>) {
        if let Some(c) = self
            .conflicts
            .conflicts
            .iter()
            .find(|c| c.id == conflict_id)
        {
            let md = read_entry_markdown(
                &serde_json::from_str(&c.local.content_json).unwrap_or_default(),
            );
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(md));
            self.toast("Скопировано", cx);
        }
    }

    /// `cancelConflict` — keep the local draft open, drop nothing durable.
    pub(crate) fn cancel_conflict(&mut self, conflict_id: &str, cx: &mut Context<Self>) {
        if let Some(c) = self
            .conflicts
            .conflicts
            .iter()
            .find(|c| c.id == conflict_id)
            .cloned()
        {
            self.current = Some(c.local.clone());
            self.conflict_op = Some((c.id.clone(), ConflictOp::Recheck));
            self.send(
                Command::LoadEntry {
                    id: c.entry_id,
                    content_only: true,
                },
                cx,
            );
        }
    }

    pub(crate) fn active_conflict(&self) -> Option<EntryConflict> {
        let pending = unresolved_entry_conflicts(&self.conflicts.conflicts);
        // Prefer the conflict attached to the open entry; else the freshest.
        let current_id = self.current.as_ref().map(|e| e.id.as_str());
        pending
            .iter()
            .find(|c| Some(c.entry_id.as_str()) == current_id)
            .or_else(|| pending.first())
            .cloned()
    }

    pub(crate) fn conflict_state_label(state: EntryConflictState) -> &'static str {
        match state {
            EntryConflictState::RemoteUpdated => "Версия изменилась удалённо",
            EntryConflictState::RemoteDeleted => "Удалена удалённо",
            EntryConflictState::StaleSave => "Сохранено по устаревшей версии",
            EntryConflictState::Merged => "Объединено",
            EntryConflictState::Resolved => "Решено",
        }
    }
}
