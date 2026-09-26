//! Engine access behind a small enum so tests and `MEMORIA_DEMO=1` can run a
//! deterministic in-memory store. `Backend::send` returns replies the caller
//! feeds through `on_reply` — for `Engine` the replies arrive asynchronously
//! via `drain` instead.

use memoria_gpui::store::{Command, Reply, Worker};

use super::demo::DemoStore;

pub(crate) enum Backend {
    Engine(Worker),
    Demo(DemoStore),
}

impl Backend {
    /// Fire a command. Demo replies are produced inline; Engine replies drain
    /// on the poll task.
    pub(crate) fn send(&mut self, command: Command) -> Vec<Reply> {
        match self {
            Backend::Engine(worker) => {
                let _ = worker.commands.send(command);
                Vec::new()
            }
            Backend::Demo(store) => store.dispatch(command),
        }
    }

    /// Drain pending async replies (Engine only; demo is synchronous).
    pub(crate) fn drain(&mut self) -> Vec<Reply> {
        match self {
            Backend::Engine(worker) => {
                let mut out = Vec::new();
                while let Ok(reply) = worker.replies.try_recv() {
                    out.push(reply);
                }
                out
            }
            Backend::Demo(_) => Vec::new(),
        }
    }

    pub(crate) fn is_demo(&self) -> bool {
        matches!(self, Backend::Demo(_))
    }
}
