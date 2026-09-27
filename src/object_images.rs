//! Ports of `src/lib/objectImages.ts` + `src/lib/localImages.ts` — image
//! header-prop values resolve to a displayable source: either the raw
//! reference or the linked image object's stored file.

use std::collections::HashMap;

use serde_json::Value;

use crate::model::Entry;
use crate::system_types_data::SYSTEM_TYPE_IMAGE_ID;

const LOCAL_IMAGE_PROTOCOL: &str = "kosmos-local-image";

fn parse_header_props(entry: &Entry) -> serde_json::Map<String, Value> {
    entry
        .header_props_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

fn first_image_candidate(value: &Value) -> String {
    match value {
        Value::Array(items) => items
            .iter()
            .find(|item| item.as_str().map(|s| !s.trim().is_empty()).unwrap_or(false))
            .map(|item| match item {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .unwrap_or_default(),
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// `isDirectImageRef` — a `scheme:`/`X:\`/`\\` prefixed value is already a
/// resolvable source, not a bare relative path.
fn is_direct_image_ref(value: &str) -> bool {
    let bytes = value.as_bytes();
    let scheme = bytes
        .first()
        .map(|b| b.is_ascii_alphabetic())
        .unwrap_or(false)
        && value
            .find(':')
            .map(|i| {
                (1..=32).contains(&i)
                    && value[1..i]
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'.' | b'-'))
            })
            .unwrap_or(false);
    let windows_drive = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/');
    scheme || windows_drive || value.starts_with("\\\\")
}

/// `toDisplayImageSrc` — Windows paths and `file:` URLs go through the local
/// image protocol; everything else (https, data:, kosmos-local-image:) passes
/// through untouched.
pub fn to_display_image_src(src: &str) -> String {
    let trimmed = src.trim();
    let windows_drive = trimmed.len() >= 3
        && trimmed.as_bytes()[0].is_ascii_alphabetic()
        && trimmed.as_bytes()[1] == b':'
        && matches!(trimmed.as_bytes()[2], b'\\' | b'/');
    if windows_drive || trimmed.starts_with("\\\\") {
        return local_image_url(trimmed);
    }
    if !trimmed.starts_with("file:") {
        return trimmed.to_string();
    }
    // `file:///C:/…` / `file:///home/…` — decode to a plain path and wrap.
    let path = trimmed.trim_start_matches("file:").trim_start_matches('/');
    let path = percent_decode(path);
    let path = path
        .strip_suffix('/')
        .map(|s| s.to_string())
        .unwrap_or(path);
    let windows = path
        .strip_prefix('/')
        .filter(|rest| {
            rest.len() >= 2
                && rest.as_bytes()[0].is_ascii_alphabetic()
                && rest.as_bytes()[1] == b':'
        })
        .map(|rest| rest.to_string())
        .unwrap_or(path);
    if windows.is_empty() {
        trimmed.to_string()
    } else {
        local_image_url(&windows)
    }
}

fn local_image_url(file_path: &str) -> String {
    format!(
        "{LOCAL_IMAGE_PROTOCOL}://file/{}",
        percent_encode(file_path)
    )
}

/// Decode a `kosmos-local-image://file/…` payload back to a plain fs path —
/// the GPUI renderer feeds `img()` real paths, not the Vue custom protocol.
pub fn percent_decode_path(encoded: &str) -> String {
    percent_decode(encoded)
}

/// `encodeURIComponent` — only `- _ . ! ~ * ' ( )` stay unescaped.
fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
            )
        {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = |b: u8| -> Option<u8> {
                match b {
                    b'0'..=b'9' => Some(b - b'0'),
                    b'a'..=b'f' => Some(b - b'a' + 10),
                    b'A'..=b'F' => Some(b - b'A' + 10),
                    _ => None,
                }
            };
            if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(hi * 16 + lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `resolveObjectImageSrc` — the prop value may hold a direct ref, an entry
/// id pointing at an image object, or an array of either (first wins).
pub fn resolve_object_image_src(value: &Value, entries_by_id: &HashMap<String, Entry>) -> String {
    let candidate = first_image_candidate(value).trim().to_string();
    if candidate.is_empty() {
        return String::new();
    }
    if let Some(linked) = entries_by_id.get(&candidate) {
        if linked.type_id.as_deref() == Some(SYSTEM_TYPE_IMAGE_ID) {
            let props = parse_header_props(linked);
            let image = props
                .get("image")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("");
            let source_path = props
                .get("source_path")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("");
            let chosen = if is_direct_image_ref(image) {
                image
            } else if !source_path.is_empty() {
                source_path
            } else {
                image
            };
            return to_display_image_src(chosen);
        }
    }
    to_display_image_src(&candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn display_src_passes_through_remote_and_wraps_local() {
        assert_eq!(
            to_display_image_src("https://example.com/c.png"),
            "https://example.com/c.png"
        );
        assert!(
            to_display_image_src("C:\\covers\\a b.png").starts_with("kosmos-local-image://file/")
        );
        assert!(
            to_display_image_src("file:///home/x/c.png").starts_with("kosmos-local-image://file/")
        );
        assert!(to_display_image_src("file:///C:/covers/c.png").contains("C%3A"));
    }

    #[test]
    fn resolves_linked_image_object() {
        let mut entries = HashMap::new();
        let mut img = Entry {
            id: "img-1".into(),
            type_id: Some(SYSTEM_TYPE_IMAGE_ID.into()),
            ..Default::default()
        };
        img.header_props_json =
            Some(r#"{"image":"cover.png","source_path":"D:\\pics\\cover.png"}"#.into());
        entries.insert("img-1".to_string(), img);
        let src = resolve_object_image_src(&json!("img-1"), &entries);
        assert!(src.starts_with("kosmos-local-image://file/"));

        // Direct refs pass through; arrays take the first non-empty item.
        assert_eq!(
            resolve_object_image_src(&json!(["", "https://x/y.png"]), &entries),
            "https://x/y.png"
        );
        assert_eq!(resolve_object_image_src(&json!(""), &entries), "");
    }
}
