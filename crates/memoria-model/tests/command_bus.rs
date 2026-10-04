//! Port of the `kepler-command-bus` halves of `tests/memoriaMigration.test.ts` —
//! canonical `memoria:` dispatches reach legacy `eden:` listeners.

use memoria_model::command_bus::CommandBus;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

// --- command bus --------------------------------------------------------------

#[test]
fn canonical_commands_route_to_legacy_listeners() {
    let received = Arc::new(Mutex::new(Vec::<Value>::new()));
    let received_c = received.clone();
    let mut bus = CommandBus::new();
    let sub = bus.on_command("eden:cmd:note:create", move |params| {
        received_c.lock().unwrap().push(params.clone());
    });
    bus.dispatch("memoria:cmd:note:create", json!({ "source": "host" }));
    bus.unsubscribe("eden:cmd:note:create", sub);
    assert_eq!(*received.lock().unwrap(), vec![json!({ "source": "host" })]);
}

#[test]
fn canonical_note_open_carries_the_entry_id() {
    let received = Arc::new(Mutex::new(Vec::<Value>::new()));
    let received_c = received.clone();
    let mut bus = CommandBus::new();
    let sub = bus.on_command("eden:cmd:note:open", move |params| {
        received_c.lock().unwrap().push(params.clone());
    });
    bus.dispatch("memoria:cmd:note:open", json!({ "entryId": "note-123" }));
    bus.unsubscribe("eden:cmd:note:open", sub);
    assert_eq!(
        *received.lock().unwrap(),
        vec![json!({ "entryId": "note-123" })]
    );
}

#[test]
fn a_panicking_handler_does_not_abort_the_dispatch() {
    // Vue wraps each listener call in try/catch — one bad handler never
    // stops the rest of the loop.
    let received = Arc::new(Mutex::new(Vec::<Value>::new()));
    let received_c = received.clone();
    let mut bus = CommandBus::new();
    bus.on_command("eden:cmd:note:open", |_| panic!("boom"));
    bus.on_command("eden:cmd:note:open", move |params| {
        received_c.lock().unwrap().push(params.clone());
    });
    bus.dispatch("memoria:cmd:note:open", json!({ "entryId": "note-1" }));
    assert_eq!(
        *received.lock().unwrap(),
        vec![json!({ "entryId": "note-1" })]
    );
}
