//! Engine `/v1/rpc` transport (ureq) + change subscription over the Engine
//! WebSocket (`ws_port` in `engine.lock.json`). Errors stay typed; user-facing
//! strings are Russian.

use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

use crate::model::ark::ensure_list;

mod bridge;
mod discovery;
mod events;
pub use bridge::*;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// Test seam: `MEMORIA_RPC_TIMEOUT_MS` shortens the request deadline.
fn rpc_timeout() -> Duration {
    std::env::var("MEMORIA_RPC_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(Duration::from_secs(15))
}
const RETRY_BACKOFF: Duration = Duration::from_millis(300);
const RECONNECT_DELAY: Duration = Duration::from_secs(1);

/// Engine RPC failure classes. `Display` strings are user-facing Russian.
#[derive(Debug, Error)]
pub enum EngineError {
    #[error("Engine не запущен. Запустите Mundus и обновите список.")]
    LockMissing,
    #[error("Не найдена папка данных Mundus")]
    DataDirMissing,
    #[error("Некорректный файл состояния Engine")]
    LockInvalid,
    #[error("Несовместимое состояние Engine. Обновите Mundus.")]
    LockIncompatible,
    #[error("Нет подтверждения от Engine. Обновите список перед повтором.")]
    Unreachable,
    #[error("Engine не ответил вовремя. Попробуйте ещё раз.")]
    Timeout,
    #[error("Engine отклонил операцию. Изменение не подтверждено.")]
    Rejected,
    #[error("Engine отклонил операцию: {0}")]
    Rpc(String),
    #[error("Конфликт версий: объект уже изменён в Engine ({0})")]
    Conflict(String),
    #[error("Некорректный ответ Engine")]
    Malformed,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EngineLock {
    pub format_version: u32,
    pub api_version: LockApiVersion,
    #[serde(default)]
    pub pid: Option<u32>,
    pub http_port: u16,
    #[serde(default)]
    pub ws_port: Option<u16>,
    pub auth_token: String,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

/// `ProtocolVersion` from `runtime/src/protocol_version.rs`.
#[derive(Debug, Clone, Deserialize)]
pub struct LockApiVersion {
    pub major: u32,
    #[serde(default)]
    pub minor: u32,
    #[serde(default)]
    pub patch: u32,
}

impl EngineLock {
    /// MAJOR must match (`is_compatible_with_server`); token is 32 bytes
    /// hex-encoded (`auth::generate_token`).
    fn validate(&self) -> Result<(), EngineError> {
        if self.format_version != 1
            || self.api_version.major != 1
            || self.http_port == 0
            || self.auth_token.len() != 64
            || !self.auth_token.bytes().all(|v| v.is_ascii_hexdigit())
        {
            return Err(EngineError::LockIncompatible);
        }
        Ok(())
    }
}

/// Shared Engine configuration: the data dir is re-read for every request so
/// an Engine restart (new lock file) is picked up without an app restart.
#[derive(Clone, Default)]
pub struct Engine {
    pub data_dir: Option<PathBuf>,
}

/// One WS change event pushed to the UI loop.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineEvent {
    /// Engine `{"event": …}` frame (`object_upserted`, `object_deleted`,
    /// `entity_changed`, `db_restored`, …).
    Changed(Value),
    /// Socket dropped or the lock file became unreadable — HTTP may still
    /// work, but realtime updates stopped; the worker keeps retrying.
    Offline,
    /// A fresh `hello_ok` after a connect attempt.
    Online,
}

/// `MUNDUS_DATA_DIR` → `KOSMOS_DATA_DIR` (legacy) → `<config>/Mundus` →
/// `<config>/Kosmos` (legacy); the first dir holding `engine.lock.json` wins.
fn data_dir() -> Result<PathBuf, EngineError> {
    discovery::data_dir()
}

impl Engine {
    /// The resolved Engine data dir (lock file lives here). App-local state
    /// (`memoria-settings.json`, `memoria-local-state.json`) sits beside it —
    /// the GPUI equivalent of the Vue host `userData` bridge.
    pub fn resolved_data_dir(&self) -> Option<PathBuf> {
        self.data_dir.clone().or_else(|| data_dir().ok())
    }

    /// Read + validate `engine.lock.json` (fresh each call — Engine may have
    /// restarted under a new token/port).
    pub fn lock(&self) -> Result<EngineLock, EngineError> {
        let directory = self.data_dir.clone().map(Ok).unwrap_or_else(data_dir)?;
        let bytes = std::fs::read(directory.join("engine.lock.json"))
            .map_err(|_| EngineError::LockMissing)?;
        let lock: EngineLock =
            serde_json::from_slice(&bytes).map_err(|_| EngineError::LockInvalid)?;
        lock.validate()?;
        Ok(lock)
    }

    /// Raw RPC. `params` must be a JSON object; `_req_id` is always attached
    /// (KOS-147: request correlation must not rely on a payload `id`).
    pub fn rpc(&self, operation: &str, mut params: Value) -> Result<Value, EngineError> {
        let lock = self.lock()?;
        if !params.is_object() {
            return Err(EngineError::Rejected);
        }
        params["operation"] = json!(operation);
        params["_req_id"] = json!(uuid::Uuid::new_v4().to_string());
        self.rpc_with_lock(&lock, params)
    }

    fn rpc_with_lock(&self, lock: &EngineLock, params: Value) -> Result<Value, EngineError> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(CONNECT_TIMEOUT)
            .timeout(rpc_timeout())
            .redirects(0)
            .build();
        let url = format!("http://127.0.0.1:{}/v1/rpc", lock.http_port);
        let send = || -> Result<ureq::Response, ureq::Error> {
            agent
                .post(&url)
                .set("Authorization", &format!("Bearer {}", lock.auth_token))
                .set("X-Kosmos-Api-Version", "1.0.0")
                .set("X-Kosmos-Client-Class", "memoria-gpui")
                .set("X-Kosmos-Client-Version", env!("CARGO_PKG_VERSION"))
                .set("X-Kosmos-Client-Pid", &std::process::id().to_string())
                .send_json(params.clone())
        };
        let response = match send() {
            Ok(response) => response,
            Err(error) if is_retryable(&error) => {
                std::thread::sleep(RETRY_BACKOFF);
                send().map_err(classify_transport)?
            }
            Err(error) => return Err(classify_transport(error)),
        };
        let status = response.status();
        let value: Value = response.into_json().map_err(|_| EngineError::Malformed)?;
        Self::decode_envelope(status, value)
    }

    /// Envelope semantics from `kepler-task-sync`'s `ark()` shim: no `ok` key →
    /// payload itself; `ok:false` → error (`object_conflict:*` → Conflict).
    fn decode_envelope(status: u16, value: Value) -> Result<Value, EngineError> {
        match value.get("ok") {
            Some(Value::Bool(true)) => Ok(value.get("data").cloned().unwrap_or(Value::Null)),
            Some(Value::Bool(false)) => Err(classify_rpc_error(&error_text(&value))),
            _ if status >= 400 => Err(EngineError::Rpc(format!("HTTP {status}"))),
            _ => Ok(value),
        }
    }
}

fn error_text(value: &Value) -> String {
    value
        .get("error")
        .or_else(|| value.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

/// `object_conflict:*` upstream failures surface as a typed conflict so the
/// save path can record a `stale-save` conflict instead of a hard failure.
fn classify_rpc_error(text: &str) -> EngineError {
    if text.starts_with("object_conflict:") {
        EngineError::Conflict(text.to_string())
    } else {
        EngineError::Rpc(text.to_string())
    }
}

fn is_retryable(error: &ureq::Error) -> bool {
    match error {
        ureq::Error::Transport(transport) => matches!(
            transport.kind(),
            ureq::ErrorKind::ConnectionFailed | ureq::ErrorKind::Dns | ureq::ErrorKind::Io
        ),
        _ => false,
    }
}

fn classify_transport(error: ureq::Error) -> EngineError {
    match error {
        ureq::Error::Status(_, response) => {
            let text = response
                .into_json::<Value>()
                .ok()
                .map(|v| error_text(&v))
                .unwrap_or_default();
            if text.is_empty() {
                EngineError::Rejected
            } else {
                classify_rpc_error(&text)
            }
        }
        ureq::Error::Transport(transport) => {
            let timed_out = transport.kind() == ureq::ErrorKind::Io
                && std::error::Error::source(&transport)
                    .and_then(|s| s.downcast_ref::<std::io::Error>())
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::TimedOut);
            if timed_out {
                EngineError::Timeout
            } else {
                EngineError::Unreachable
            }
        }
    }
}
