//! Diary (M6) reply handling — the `Reply::*Bubble*`/`DiaryMigrated` arms of
//! `on_reply`, extracted for size. `Reply::Bubbles` fills the feed; write
//! replies re-list; `DiaryMigrated` writes remaining local-blob sources back
//! (Vue `localStorage.setItem`/`removeItem` on the same key).
use gpui::Context;
use memoria_model::diary::LOCAL_BUBBLES_STORAGE_KEY;
use memoria_model::store::{Command, Reply};

use super::Memoria;

impl Memoria {
    /// Diary arms of `on_reply` — see `replies.rs` for the rest.
    pub(crate) fn on_bubble_reply(&mut self, reply: Reply, cx: &mut Context<Self>) {
        match reply {
            Reply::Bubbles(result) => match result {
                Ok(nodes) => {
                    self.bubbles = nodes;
                    self.bubbles_loaded = true;
                    self.label_now = memoria_model::time::now_millis();
                }
                Err(e) => self.toast(e, cx),
            },
            Reply::BubbleCreated(result) => {
                // Vue `runBubbleWrite` `finally` — decrement before refresh.
                self.active_bubble_writes = self.active_bubble_writes.saturating_sub(1);
                match result {
                    Ok(_) => {
                        // `composerEditor.clearContent()` / `cancelReply()` — the
                        // entities need a `Window`, so the reset is deferred to
                        // the next render via `pending_diary_reset`.
                        self.pending_diary_reset = true;
                        self.reply_target = None;
                        self.send(Command::ListBubbles, cx);
                    }
                    Err(e) => self.toast(e, cx),
                }
            }
            Reply::BubbleUpdated { result, .. } => {
                self.active_bubble_writes = self.active_bubble_writes.saturating_sub(1);
                match result {
                    Ok(()) => {
                        self.editing_bubble = None;
                        self.bubble_delete_armed = None;
                        self.send(Command::ListBubbles, cx);
                    }
                    Err(e) => self.toast(e, cx),
                }
            }
            Reply::BubbleDeleted { result, .. } => {
                self.active_bubble_writes = self.active_bubble_writes.saturating_sub(1);
                match result {
                    Ok(()) => self.send(Command::ListBubbles, cx),
                    Err(e) => self.toast(e, cx),
                }
            }
            Reply::BubbleMigrated { result, .. } => {
                if let Err(e) = result {
                    self.toast(format!("Миграция записи: {e}"), cx);
                }
            }
            Reply::DiaryMigrated(result) => {
                self.on_diary_migrated(result, cx);
            }
            _ => unreachable!("on_bubble_reply only sees bubble replies"),
        }
    }

    /// `Reply::DiaryMigrated` — write the remaining (failed) local sources
    /// back to the blob, then load the feed. `Some([])` removes the key like
    /// Vue's `localStorage.removeItem`; `None` means there was no valid blob
    /// and the key stays untouched.
    fn on_diary_migrated(
        &mut self,
        result: Result<Option<Vec<serde_json::Value>>, String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(remaining) => {
                match &remaining {
                    None => {}
                    Some(remaining) if remaining.is_empty() => {
                        self.prefs.extra.remove(LOCAL_BUBBLES_STORAGE_KEY);
                    }
                    Some(remaining) => {
                        let mut record = self
                            .prefs
                            .extra
                            .get(LOCAL_BUBBLES_STORAGE_KEY)
                            .cloned()
                            .filter(|v| v.is_object())
                            .unwrap_or_else(|| serde_json::json!({}));
                        record["bubbles"] = serde_json::Value::Array(remaining.clone());
                        self.prefs
                            .extra
                            .insert(LOCAL_BUBBLES_STORAGE_KEY.to_string(), record);
                    }
                }
                if remaining.is_some() {
                    self.persist_prefs();
                }
                self.send(Command::ListBubbles, cx);
            }
            Err(e) => {
                // Leave `diary_started` false so the next diary visit
                // retries the migration (Vue retries on journalEntries
                // change; a failed first run shouldn't stick all session).
                self.diary_started = false;
                self.toast(e, cx);
                self.send(Command::ListBubbles, cx);
            }
        }
    }
}
