//! Port of `src/lib/kepler-command-bus.ts` — in-memory command bridge. Host
//! commands arrive on `memoria:…` channels and are routed to `eden:…`
//! listeners during the transition; dispatches with no listener are queued
//! and flushed to the first subscriber.

use std::collections::HashMap;

use serde_json::Value;

use crate::migration::{LEGACY_COMMAND_PREFIX, MEMORIA_COMMAND_PREFIX};

/// `edenChannel` — canonical commands normalize to the legacy channel name.
pub fn eden_channel(channel: &str) -> String {
    channel
        .strip_prefix(MEMORIA_COMMAND_PREFIX)
        .map(|rest| format!("{LEGACY_COMMAND_PREFIX}{rest}"))
        .unwrap_or_else(|| channel.to_string())
}

type Handler = Box<dyn FnMut(&Value) + Send>;

/// `commandListeners` + `pendingDispatches` as an owned instance (the Vue
/// module uses process globals; tests isolate per-instance instead).
#[derive(Default)]
pub struct CommandBus {
    listeners: HashMap<String, Vec<(usize, Handler)>>,
    pending: HashMap<String, Vec<Value>>,
    next_id: usize,
}

/// `onCommand` unsubscription handle — dropping/unsubscribing is explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subscription(pub usize);

impl CommandBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// `dispatchEdenCommand` — deliver to every listener on the normalized
    /// channel, else queue for the next subscriber.
    pub fn dispatch(&mut self, channel: &str, params: Value) {
        let normalized = eden_channel(channel);
        let handlers: Vec<usize> = self
            .listeners
            .get(&normalized)
            .map(|set| set.iter().map(|(id, _)| *id).collect())
            .unwrap_or_default();
        if handlers.is_empty() {
            self.pending.entry(normalized).or_default().push(params);
            return;
        }
        for id in handlers {
            if let Some(set) = self.listeners.get_mut(&normalized) {
                if let Some((_, handler)) = set.iter_mut().find(|(hid, _)| *hid == id) {
                    handler(&params);
                }
            }
        }
    }

    /// `onCommand` — subscribe on the normalized channel; pending dispatches
    /// flush to the new handler immediately (Vue uses `queueMicrotask`).
    pub fn on_command(
        &mut self,
        channel: &str,
        handler: impl FnMut(&Value) + Send + 'static,
    ) -> Subscription {
        let normalized = eden_channel(channel);
        let id = self.next_id;
        self.next_id += 1;
        let mut handler: Handler = Box::new(handler);
        if let Some(pending) = self.pending.remove(&normalized) {
            for params in pending {
                handler(&params);
            }
        }
        self.listeners
            .entry(normalized)
            .or_default()
            .push((id, handler));
        Subscription(id)
    }

    /// The `off()` closure returned by `onCommand`.
    pub fn unsubscribe(&mut self, channel: &str, subscription: Subscription) {
        let normalized = eden_channel(channel);
        if let Some(set) = self.listeners.get_mut(&normalized) {
            set.retain(|(id, _)| *id != subscription.0);
        }
    }
}
