//! Transport tests against a fake Engine: minimal HTTP/1.1 server on
//! `TcpListener` + a temp `engine.lock.json`. Covers success, rejected ops,
//! version conflicts, HTTP errors, timeouts, malformed bodies, lock-file
//! validation and the request envelope (`operation`, `_req_id`, headers).

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;

use serde_json::{json, Value};

use super::transport::{ArkBridge, Engine, ErrorKind};

fn token() -> String {
    "a".repeat(64)
}

fn write_lock(dir: &std::path::Path, port: u16) {
    std::fs::create_dir_all(dir).unwrap();
    let lock = json!({
        "format_version": 1,
        "api_version": { "major": 1, "minor": 0, "patch": 0 },
        "pid": 1234,
        "http_port": port,
        "ws_port": null,
        "auth_token": token(),
    });
    std::fs::write(dir.join("engine.lock.json"), lock.to_string()).unwrap();
}

fn engine_at(dir: &std::path::Path) -> Engine {
    Engine {
        data_dir: Some(dir.to_path_buf()),
    }
}

/// Minimal fake Engine: reads one POST request, replies `status` + `body`.
/// The captured request line/headers/body go back over the channel.
struct FakeEngine {
    _thread: std::thread::JoinHandle<()>,
    requests: mpsc::Receiver<(String, String, String)>,
}

fn fake_engine(
    respond: impl Fn(&Value) -> (u16, String) + Send + Sync + 'static,
) -> (tempfile::TempDir, FakeEngine) {
    let respond = std::sync::Arc::new(respond);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, requests) = mpsc::channel();
    let thread = std::thread::spawn(move || loop {
        let Ok((mut socket, _)) = listener.accept() else {
            return;
        };
        let tx = tx.clone();
        let respond = respond.clone();
        std::thread::spawn(move || {
            let _ = socket.set_read_timeout(Some(std::time::Duration::from_secs(5)));
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            let mut head_end = None;
            while head_end.is_none() {
                match socket.read(&mut chunk) {
                    Ok(0) | Err(_) => return,
                    Ok(n) => {
                        buf.extend_from_slice(&chunk[..n]);
                        head_end = find_subslice(&buf, b"\r\n\r\n").map(|i| i + 4);
                    }
                }
            }
            let head_end = head_end.unwrap();
            let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
            let content_length = head
                .lines()
                .find_map(|l| {
                    l.strip_prefix("Content-Length:")
                        .or_else(|| l.strip_prefix("content-length:"))
                })
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            while buf.len() < head_end + content_length {
                match socket.read(&mut chunk) {
                    Ok(0) | Err(_) => return,
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                }
            }
            let body = String::from_utf8_lossy(&buf[head_end..]).to_string();
            let (method_line, headers) = head.split_once("\r\n").unwrap_or((&head, ""));
            let _ = tx.send((method_line.to_string(), headers.to_string(), body.clone()));
            let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            let (status, response) = respond(&parsed);
            let reply = format!(
                "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.len(),
                response
            );
            let _ = socket.write_all(reply.as_bytes());
        });
    });
    let dir = tempfile::tempdir().unwrap();
    write_lock(dir.path(), port);
    (
        dir,
        FakeEngine {
            _thread: thread,
            requests,
        },
    )
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn rpc_success_unwraps_data_and_sends_envelope() {
    let (dir, fake) = fake_engine(|_| {
        (
            200,
            json!({ "ok": true, "data": [{ "id": "n1" }] }).to_string(),
        )
    });
    let engine = engine_at(dir.path());
    let items = engine.list_objects_by_type("com.kosmos.note").unwrap();
    assert_eq!(items, vec![json!({ "id": "n1" })]);

    let (request_line, headers, body) = fake.requests.recv().unwrap();
    let headers = headers.to_lowercase();
    assert!(request_line.starts_with("POST /v1/rpc "));
    assert!(headers.contains(&format!("authorization: bearer {}", token())));
    assert!(headers.contains("x-kosmos-api-version: 1.0.0"));
    assert!(headers.contains("x-kosmos-client-class: memoria-gpui"));
    let value: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(value["operation"], "list_objects_by_type");
    assert_eq!(value["type_id"], "com.kosmos.note");
    assert!(
        value["_req_id"].as_str().unwrap().len() == 36,
        "uuid req id"
    );
}

#[test]
fn rpc_ok_false_maps_to_typed_error() {
    let (dir, _fake) =
        fake_engine(|_| (200, json!({ "ok": false, "error": "denied" }).to_string()));
    let error = engine_at(dir.path())
        .list_objects("com.kosmos.note")
        .unwrap_err();
    // The raw wire code stays in `detail` (it lands in the app log via
    // `Display`); the user sees only the class text.
    assert_eq!(error.detail, "denied");
    // "denied" is not a documented Engine class — Unknown, and its user
    // text must not claim "temporarily unavailable" (that would be a lie).
    assert_eq!(error.kind, ErrorKind::Unknown);
    assert_eq!(
        error.message(),
        "Engine сообщил о неизвестной ошибке. Подробности записаны в журнал приложения."
    );
    assert!(!error.message().contains("denied"));
}

#[test]
fn rpc_object_conflict_maps_to_conflict() {
    let (dir, _fake) = fake_engine(|_| {
        (
            200,
            json!({ "ok": false, "error": "object_conflict:version" }).to_string(),
        )
    });
    let error = engine_at(dir.path())
        .upsert_object(json!({ "id": "n1" }))
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Conflict);
    assert_eq!(error.detail, "object_conflict:version");
}

#[test]
fn http_5xx_is_rejected_not_success() {
    let (dir, _fake) = fake_engine(|_| (500, "oops".into()));
    let error = engine_at(dir.path())
        .list_objects("com.kosmos.note")
        .unwrap_err();
    assert!(!matches!(
        error.kind,
        ErrorKind::NotRunning | ErrorKind::NotCompatible
    ));
}

#[test]
fn malformed_body_is_not_success() {
    let (dir, _fake) = fake_engine(|_| (200, "{not json".into()));
    let error = engine_at(dir.path())
        .list_objects("com.kosmos.note")
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Malformed);
}

#[test]
fn timeout_is_typed() {
    std::env::set_var("MEMORIA_RPC_TIMEOUT_MS", "200");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    // Accept and never reply.
    std::thread::spawn(move || {
        if let Ok((_s, _)) = listener.accept() {
            std::thread::sleep(std::time::Duration::from_secs(10));
        }
    });
    let dir = tempdir();
    write_lock(dir.path(), port);
    let error = engine_at(dir.path())
        .list_objects("com.kosmos.note")
        .unwrap_err();
    std::env::remove_var("MEMORIA_RPC_TIMEOUT_MS");
    assert!(matches!(
        error.kind,
        ErrorKind::Timeout | ErrorKind::Transport
    ));
}

#[test]
fn lock_missing_is_typed() {
    let dir = tempdir();
    let error = engine_at(dir.path()).list_objects("x").unwrap_err();
    assert_eq!(error.kind, ErrorKind::NotRunning);
}

#[test]
fn lock_incompatible_api_major() {
    let dir = tempdir();
    std::fs::write(
        dir.path().join("engine.lock.json"),
        json!({
            "format_version": 1,
            "api_version": { "major": 2, "minor": 0, "patch": 0 },
            "http_port": 1,
            "auth_token": token(),
        })
        .to_string(),
    )
    .unwrap();
    let error = engine_at(dir.path()).list_objects("x").unwrap_err();
    assert_eq!(error.kind, ErrorKind::NotCompatible);
}

#[test]
fn lock_bad_token_is_incompatible() {
    let dir = tempdir();
    std::fs::write(
        dir.path().join("engine.lock.json"),
        json!({
            "format_version": 1,
            "api_version": { "major": 1, "minor": 0, "patch": 0 },
            "http_port": 1,
            "auth_token": "short",
        })
        .to_string(),
    )
    .unwrap();
    let error = engine_at(dir.path()).list_objects("x").unwrap_err();
    assert_eq!(error.kind, ErrorKind::NotCompatible);
}

#[test]
fn lock_invalid_json_is_typed() {
    let dir = tempdir();
    std::fs::write(dir.path().join("engine.lock.json"), "{nope").unwrap();
    let error = engine_at(dir.path()).list_objects("x").unwrap_err();
    assert_eq!(error.kind, ErrorKind::NotCompatible);
}

#[test]
fn list_unwraps_items_key() {
    let (dir, _fake) = fake_engine(|_| {
        (
            200,
            json!({ "ok": true, "data": { "items": [{ "id": "a" }] } }).to_string(),
        )
    });
    let items = engine_at(dir.path()).list_objects_by_type("t").unwrap();
    assert_eq!(items, vec![json!({ "id": "a" })]);
}

#[test]
fn upsert_never_reports_unconfirmed_write() {
    // ok:false on upsert MUST surface as Err — not a silent success.
    let (dir, _fake) = fake_engine(|_| {
        (
            200,
            json!({ "ok": false, "error": "write_denied" }).to_string(),
        )
    });
    assert!(engine_at(dir.path()).upsert_object(json!({})).is_err());
}
