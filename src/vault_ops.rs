//! Engine-mediated vault IO (KOS-155): the `filesystem.vault.*` RPC surface
//! over `ArkBridge`. Memoria never opens files itself — Engine owns every
//! byte, grant-checked against this process (`memoria-gpui:{pid}`).

use serde::Deserialize;
use serde_json::{json, Value};

use crate::obsidian::{ObsidianExportFile, ObsidianVaultImageFile, ObsidianVaultMarkdownFile};
use crate::store::transport::{ArkBridge, EngineError};

/// `kosmos-local-image://file/…` URLs carry the percent-encoded absolute path
/// so Engine can serve/copy the file again; the app treats them as opaque but
/// must decode them to fill `source_path`/`path` fields (Vue parity).
const LOCAL_IMAGE_PREFIX: &str = "kosmos-local-image://file/";

/// A vault root opened through Engine (`filesystem.vault.open`).
#[derive(Clone, Debug, Default)]
pub struct OpenedVault {
    /// Grant-scoped handle for follow-up `vault.read`/`vault.close` calls.
    pub root_id: String,
    /// Vault directory name (never a path).
    pub name: String,
    /// Stable per-directory identity (hash of the canonical path, Engine-side).
    pub vault_key: String,
    pub files: Vec<ObsidianVaultMarkdownFile>,
    pub images: Vec<ObsidianVaultImageFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VaultTextRecord {
    relative_path: String,
    name: String,
    content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VaultImageRecord {
    relative_path: String,
    name: String,
    file_url: String,
    mime_type: String,
    size_bytes: u64,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenVaultResponse {
    root_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    vault_key: String,
    #[serde(default)]
    files: Vec<VaultTextRecord>,
    #[serde(default)]
    images: Vec<VaultImageRecord>,
}

fn decode_local_image_file_url(file_url: &str) -> String {
    file_url
        .strip_prefix(LOCAL_IMAGE_PREFIX)
        .and_then(|rest| {
            percent_encoding::percent_decode_str(rest)
                .decode_utf8()
                .ok()
        })
        .map(|decoded| decoded.into_owned())
        .unwrap_or_else(|| file_url.to_string())
}

fn read_string<'a>(value: &'a Value, key: &str) -> Result<&'a str, EngineError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(EngineError::Malformed)
}

/// `filesystem.vault.*` client. One instance per app process; grants are
/// owner-bound in Engine, so a `rootId` is only usable through this bridge.
#[derive(Clone)]
pub struct EngineVault<B> {
    pub bridge: B,
}

impl<B: ArkBridge> EngineVault<B> {
    pub fn new(bridge: B) -> Self {
        Self { bridge }
    }

    /// `filesystem.vault.open` — register a grant + scan markdown/images in
    /// one call. The returned files carry vault-relative paths; `path`/`fileUrl`
    /// hold the Engine-servable image URL (decoded into `path` for parity).
    pub fn open_vault(&self, dir: &str) -> Result<OpenedVault, EngineError> {
        let data = self
            .bridge
            .rpc("filesystem.vault.open", json!({ "path": dir }))?;
        let response: OpenVaultResponse =
            serde_json::from_value(data).map_err(|_| EngineError::Malformed)?;
        let vault_key = response.vault_key.clone();
        let files = response
            .files
            .into_iter()
            .map(|f| ObsidianVaultMarkdownFile {
                // No raw path exists app-side; `{vaultKey}/{relative}` keeps
                // `obsidianNoteId`/`inferVaultIdentity` stable across sessions.
                path: format!("{vault_key}/{}", f.relative_path),
                relative_path: f.relative_path,
                name: f.name,
                content: f.content,
            })
            .collect();
        let images = response
            .images
            .into_iter()
            .map(|i| {
                let path = decode_local_image_file_url(&i.file_url);
                ObsidianVaultImageFile {
                    path,
                    relative_path: i.relative_path,
                    name: i.name,
                    file_url: i.file_url,
                    mime_type: i.mime_type,
                    size_bytes: i.size_bytes,
                    width: i.width,
                    height: i.height,
                }
            })
            .collect();
        Ok(OpenedVault {
            root_id: response.root_id,
            name: response.name,
            vault_key,
            files,
            images,
        })
    }

    /// `filesystem.vault.register` — open a grant without scanning (export
    /// target). Returns `(root_id, vault_key)`.
    pub fn register_vault(&self, dir: &str) -> Result<(String, String), EngineError> {
        let data = self
            .bridge
            .rpc("filesystem.vault.register", json!({ "path": dir }))?;
        Ok((
            read_string(&data, "rootId")?.to_string(),
            data.get("vaultKey")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        ))
    }

    /// `filesystem.vault.read` — bytes for one vault-relative path.
    pub fn read_file(&self, root_id: &str, path: &str) -> Result<Vec<u8>, EngineError> {
        let data = self.bridge.rpc(
            "filesystem.vault.read",
            json!({ "rootId": root_id, "path": path }),
        )?;
        decode_base64(read_string(&data, "bytesBase64")?).ok_or(EngineError::Malformed)
    }

    /// `filesystem.vault.export` — write/copy the planned files under a
    /// registered root. Returns `exportedCount` (skipped copies don't count).
    pub fn export_vault(
        &self,
        root_id: &str,
        files: &[ObsidianExportFile],
    ) -> Result<u64, EngineError> {
        let data = self.bridge.rpc(
            "filesystem.vault.export",
            json!({
                "rootId": root_id,
                "files": serde_json::to_value(files).map_err(|_| EngineError::Malformed)?,
            }),
        )?;
        data.get("exportedCount")
            .and_then(Value::as_u64)
            .ok_or(EngineError::Malformed)
    }

    /// `filesystem.vault.close` — release the root grant.
    pub fn close_vault(&self, root_id: &str) -> Result<(), EngineError> {
        self.bridge
            .rpc("filesystem.vault.close", json!({ "rootId": root_id }))?;
        Ok(())
    }
}

/// Minimal base64 decoder for `bytesBase64` payloads (avoids a base64 dep for
/// one RPC field).
fn decode_base64(input: &str) -> Option<Vec<u8>> {
    fn value(byte: u8) -> Option<u8> {
        match byte {
            b'A'..=b'Z' => Some(byte - b'A'),
            b'a'..=b'z' => Some(byte - b'a' + 26),
            b'0'..=b'9' => Some(byte - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let clean: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if !clean.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(clean.len() / 4 * 3);
    for chunk in clean.chunks(4) {
        let pad = chunk.iter().filter(|&&b| b == b'=').count();
        if pad > 2 || chunk.iter().take(4 - pad).any(|&b| b == b'=') {
            return None;
        }
        let mut acc: u32 = 0;
        for (i, &byte) in chunk.iter().enumerate() {
            let v = if byte == b'=' { 0 } else { value(byte)? } as u32;
            acc |= v << (18 - 6 * i);
        }
        out.push((acc >> 16) as u8);
        if pad < 2 {
            out.push((acc >> 8) as u8);
        }
        if pad < 1 {
            out.push(acc as u8);
        }
    }
    Some(out)
}
