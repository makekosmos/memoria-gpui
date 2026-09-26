use serde::Deserialize;
use serde_json::{json, Value};
use std::{path::PathBuf, time::Duration};

#[derive(Deserialize)]
struct Lock {
    format_version: u32,
    api_version: Version,
    http_port: u16,
    auth_token: String,
}

#[derive(Deserialize)]
struct Version {
    major: u32,
}

pub struct Engine {
    pub data_dir: Option<PathBuf>,
    agent: ureq::Agent,
}

impl Default for Engine {
    fn default() -> Self {
        Self {
            data_dir: None,
            agent: ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(15))
                .redirects(0)
                .build(),
        }
    }
}

impl Engine {
    pub fn rpc(&self, operation: &str, mut params: Value) -> Result<Value, String> {
        let directory = self.data_dir.clone().map(Ok).unwrap_or_else(data_dir)?;
        // Re-read discovery on every request: Engine may have restarted.
        let bytes = std::fs::read(directory.join("engine.lock.json"))
            .map_err(|_| "Engine не запущен. Запустите Kosmos и обновите список.".to_string())?;
        let lock: Lock =
            serde_json::from_slice(&bytes).map_err(|_| "Некорректный файл состояния Engine")?;
        if lock.format_version != 1
            || lock.api_version.major != 1
            || lock.http_port == 0
            || lock.auth_token.len() != 64
            || !lock.auth_token.bytes().all(|v| v.is_ascii_hexdigit())
        {
            return Err("Несовместимое состояние Engine. Обновите Kosmos.".into());
        }
        if !params.is_object() {
            return Err("Некорректный запрос Engine".into());
        }
        params["operation"] = json!(operation);
        params["_req_id"] = json!(uuid::Uuid::new_v4().to_string());
        let response = self
            .agent
            .post(&format!("http://127.0.0.1:{}/v1/rpc", lock.http_port))
            .set("Authorization", &format!("Bearer {}", lock.auth_token))
            .set("X-Kosmos-Api-Version", "1.0.0")
            .set("X-Kosmos-Client-Class", "memoria-gpui")
            .set("X-Kosmos-Client-Version", env!("CARGO_PKG_VERSION"))
            .set("X-Kosmos-Client-Pid", &std::process::id().to_string())
            .send_json(params)
            .map_err(|_| {
                "Нет подтверждения от Engine. Обновите список перед повтором.".to_string()
            })?;
        let value: Value = response
            .into_json()
            .map_err(|_| "Некорректный ответ Engine")?;
        if value["ok"] != true {
            return Err("Engine отклонил операцию. Изменение не подтверждено.".into());
        }
        value
            .get("data")
            .cloned()
            .ok_or("Некорректный ответ Engine".into())
    }
}

fn data_dir() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("KOSMOS_DATA_DIR").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base =
        std::env::var_os("HOME").map(|v| PathBuf::from(v).join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|v| PathBuf::from(v).join(".config")));
    base.map(|v| v.join("Kosmos"))
        .ok_or("Не найдена папка данных Kosmos".into())
}
