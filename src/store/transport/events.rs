//! Engine change-event subscription over the `ws_port` WebSocket —
//! `subscribe_events` + the socket loop.

use super::*;

impl Engine {
    /// Subscribe to Engine change events. The thread reconnects until `stop`
    /// is set; the lock file is re-read on every attempt so a restarted Engine
    /// (new port/token) is found without an app restart. `Online` is emitted
    /// after each successful hello handshake, `Offline` once per drop.
    pub fn subscribe_events(&self, sink: Sender<EngineEvent>, stop: Arc<AtomicBool>) {
        let engine = self.clone();
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                match engine.lock() {
                    Ok(lock) => {
                        if let Some(ws_port) = lock.ws_port {
                            subscribe_socket(&lock, ws_port, &sink, &stop);
                            // Dropped socket (or no hello_ok) — one signal per
                            // transition, then retry after the delay.
                            let _ = sink.send(EngineEvent::Offline);
                        }
                    }
                    Err(_) => {
                        let _ = sink.send(EngineEvent::Offline);
                    }
                }
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(RECONNECT_DELAY);
            }
        });
    }
}

fn subscribe_socket(
    lock: &EngineLock,
    ws_port: u16,
    sink: &Sender<EngineEvent>,
    stop: &Arc<AtomicBool>,
) {
    let url = format!("ws://127.0.0.1:{ws_port}/");
    let Ok((mut socket, _)) = tungstenite::connect(&url) else {
        return;
    };
    // Poll `stop` between frames: a pure blocking read would keep the
    // subscription alive past shutdown.
    if let tungstenite::stream::MaybeTlsStream::Plain(stream) = socket.get_ref() {
        let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    }
    let hello = json!({
        "kind": "hello",
        "apiVersion": "1.0.0",
        "token": lock.auth_token,
        "pid": std::process::id(),
        "clientId": "memoria-gpui",
        "clientClass": "memoria-gpui",
        "clientVersion": env!("CARGO_PKG_VERSION"),
    });
    if socket
        .send(tungstenite::Message::Text(hello.to_string().into()))
        .is_err()
    {
        return;
    }
    let mut greeted = false;
    loop {
        if stop.load(Ordering::Relaxed) {
            let _ = socket.close(None);
            return;
        }
        match socket.read() {
            Ok(tungstenite::Message::Text(text)) => {
                let Ok(value) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                match value.get("kind").and_then(Value::as_str) {
                    Some("hello_error") => return,
                    Some("hello_ok") => {
                        greeted = true;
                        if sink.send(EngineEvent::Online).is_err() {
                            return;
                        }
                        continue;
                    }
                    _ => {}
                }
                if !greeted {
                    continue;
                }
                if value.get("event").is_some() && sink.send(EngineEvent::Changed(value)).is_err() {
                    return;
                }
            }
            Ok(tungstenite::Message::Ping(payload)) => {
                let _ = socket.send(tungstenite::Message::Pong(payload));
            }
            Ok(tungstenite::Message::Close(_)) => return,
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return,
            _ => {}
        }
    }
}
