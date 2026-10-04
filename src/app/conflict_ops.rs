//! Conflict-operation continuations — `acceptConflict`/`recheckConflict`
//! finish steps that run once the canonical `loadEntry` reply arrives.
use gpui::Context;

use memoria_model::entry_conflicts::{EntryConflict, EntryConflictState};
use memoria_model::live_refresh::entry_visible_fingerprint;
use memoria_model::model::Entry;
use memoria_model::routes::Route;

use super::types::ConflictOp;
use super::Memoria;

impl Memoria {
    /// Continue a pending conflict op on the canonical `loadEntry` reply.
    pub(crate) fn conflict_entry_reply(
        &mut self,
        id: String,
        result: Result<Option<Entry>, String>,
        cx: &mut Context<Self>,
    ) {
        let Some((conflict_id, op)) = self.conflict_op.take() else {
            return;
        };
        let canonical = result.ok().flatten();
        let Some(conflict) = self
            .conflicts
            .conflicts
            .iter()
            .find(|c| c.id == conflict_id)
            .cloned()
        else {
            return;
        };
        if conflict.entry_id != id {
            self.conflict_op = Some((conflict_id, op));
            return;
        }
        match op {
            ConflictOp::Recheck => self.finish_recheck(&conflict, canonical),
            ConflictOp::Accept => self.finish_accept(&conflict, canonical, cx),
        }
    }

    /// `recheckConflict` resolution step — same fingerprint chain as Vue.
    fn finish_recheck(&mut self, conflict: &EntryConflict, remote: Option<Entry>) {
        let local_fp = entry_visible_fingerprint(&conflict.local);
        match &remote {
            Some(remote) if entry_visible_fingerprint(remote) == local_fp => {
                self.apply_remote(conflict, remote.clone());
                self.conflicts.resolve(&conflict.id, Some("accept-remote"));
            }
            _ => {
                self.conflicts.record(
                    &conflict.local,
                    remote.as_ref(),
                    if remote.is_some() {
                        EntryConflictState::RemoteUpdated
                    } else {
                        EntryConflictState::RemoteDeleted
                    },
                );
            }
        }
    }

    /// `acceptConflict` — verifies the canonical remote still matches the
    /// recorded one before applying it (the `sameRevision` guard).
    fn finish_accept(
        &mut self,
        conflict: &EntryConflict,
        canonical: Option<Entry>,
        cx: &mut Context<Self>,
    ) {
        let same = match (&canonical, &conflict.remote) {
            (None, None) => true,
            (Some(c), Some(r)) => {
                c.updated_at == r.updated_at
                    && entry_visible_fingerprint(c) == entry_visible_fingerprint(r)
            }
            _ => false,
        };
        if !same {
            self.conflicts.record(
                &conflict.local,
                canonical.as_ref(),
                if canonical.is_some() {
                    EntryConflictState::RemoteUpdated
                } else {
                    EntryConflictState::RemoteDeleted
                },
            );
            self.toast("Конфликт изменился — проверьте снова", cx);
            return;
        }
        match canonical {
            Some(remote) => self.apply_remote(conflict, remote),
            // accept-remote when canonical is gone = accept the deletion.
            None => {
                self.list.retain(|e| e.id != conflict.entry_id);
                if self.current.as_ref().map(|e| e.id.as_str()) == Some(conflict.entry_id.as_str())
                {
                    self.current = None;
                    self.navigate(Route::Everything, cx);
                }
            }
        }
        self.conflicts.resolve(&conflict.id, Some("accept-remote"));
        self.toast("Принята удалённая версия", cx);
    }

    /// `applyConflictRemote` — remote becomes current + list baseline.
    fn apply_remote(&mut self, conflict: &EntryConflict, remote: Entry) {
        if let Some(slot) = self.list.iter_mut().find(|e| e.id == conflict.entry_id) {
            *slot = remote.clone();
        }
        if self.current.as_ref().map(|e| e.id.as_str()) == Some(conflict.entry_id.as_str()) {
            self.current = Some(remote);
        }
    }
}
