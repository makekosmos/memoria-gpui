//! Reply plumbing + toast/pref lifecycle.
use std::time::Duration;

use gpui::Context;

use memoria_gpui::local_state::save_local_state;
use memoria_gpui::store::Command;

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
        if let Backend::Engine(worker) = &self.backend {
            if worker
                .replies
                .try_recv()
                .is_err_and(|e| matches!(e, std::sync::mpsc::TryRecvError::Disconnected))
            {
                self.online = false;
                self.banner = Some(ENGINE_OFFLINE.into());
            }
        }
        for reply in self.backend.drain() {
            self.on_reply(reply, cx);
        }
        true
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
