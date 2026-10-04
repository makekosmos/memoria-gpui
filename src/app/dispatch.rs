//! Reply plumbing + toast/pref lifecycle.
use std::time::Duration;

use gpui::Context;

use memoria_model::local_state::save_local_state;
use memoria_model::store::Command;

use super::types::{Toast, ENGINE_OFFLINE, TOAST_TTL};
use super::{Backend, Memoria};

impl Memoria {
    /// 100ms poll draining Engine replies (agenda-gpui Worker pattern).
    pub(crate) fn start_poll(&mut self, cx: &mut Context<Self>) {
        self._poll = Some(cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let alive = this.update(cx, |this, cx| this.drain(cx)).unwrap_or(false);
            if !alive {
                break;
            }
        }));
    }

    fn drain(&mut self, cx: &mut Context<Self>) -> bool {
        let mut disconnected = false;
        let mut replies = Vec::new();
        if let Backend::Engine(worker) = &self.backend {
            loop {
                match worker.replies.try_recv() {
                    Ok(reply) => replies.push(reply),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        } else {
            replies = self.backend.drain();
        }
        if disconnected {
            self.online = false;
            self.banner = Some(ENGINE_OFFLINE.into());
        }
        for reply in replies {
            self.on_reply(reply, cx);
        }
        true
    }

    /// «Отображаемые типы» (`visibleObjectTypeIds`) — an empty stored list
    /// means all; a stored subset filters entries by resolved type id. Applied
    /// at `Reply::List` so refetch and the settings toggle share the predicate.
    pub(crate) fn filter_visible_types(
        &self,
        list: Vec<memoria_model::model::Entry>,
    ) -> Vec<memoria_model::model::Entry> {
        if self.prefs.visible_object_type_ids.is_empty() {
            return list;
        }
        let all: Vec<String> = self.note_types.iter().map(|t| t.id.clone()).collect();
        let visible =
            memoria_model::local_state::visible_type_set(&self.prefs.visible_object_type_ids, &all);
        list.into_iter()
            .filter(|e| e.type_id.as_deref().is_none_or(|t| visible.contains(t)))
            .collect()
    }

    pub(crate) fn send(&mut self, command: Command, cx: &mut Context<Self>) {
        for reply in self.backend.send(command) {
            self.on_reply(reply, cx);
        }
    }

    pub(crate) fn toast(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.toast_seq += 1;
        let id = self.toast_seq;
        self.toasts.push(Toast {
            id,
            text: text.into(),
        });
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOAST_TTL).await;
            let _ = this.update(cx, |this, cx| {
                this.toasts.retain(|t| t.id != id);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn persist_prefs(&mut self) {
        if let Some(dir) = &self.state_dir {
            save_local_state(dir, &self.prefs);
        }
    }
}
