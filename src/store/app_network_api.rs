//! Engine-owned app network ops (KOS-152) — the worker-side wrappers over
//! `ArkBridge` that turn the raw `{path|color|finalUrl|html}` payloads into
//! typed results. Apps never do IO themselves; these five ops are the whole
//! surface the header/cover UI needs.

use serde_json::Value;

use crate::book_metadata::{BookMetadata, BookMetadataPage};

use super::transport::{ArkBridge, EngineError};

fn str_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// `bookMetadata.lookupIsbn` → `Option<BookMetadata>` (null = not found).
pub fn lookup_isbn<B: ArkBridge>(
    bridge: &B,
    isbn: &str,
) -> Result<Option<BookMetadata>, EngineError> {
    let raw = bridge.lookup_isbn(isbn.trim())?;
    if raw.is_null() {
        return Ok(None);
    }
    serde_json::from_value::<BookMetadata>(raw)
        .map(Some)
        .map_err(|_| EngineError::Malformed)
}

/// `bookMetadata.fetchPage` → `{finalUrl, html}`; the app extracts metadata
/// locally (same split as Vue: Engine fetches, page code parses).
pub fn fetch_book_page<B: ArkBridge>(
    bridge: &B,
    url: &str,
) -> Result<Option<BookMetadataPage>, EngineError> {
    let raw = bridge.fetch_book_page(url.trim())?;
    if raw.is_null() {
        return Ok(None);
    }
    let (Some(final_url), Some(html)) = (
        str_field(&raw, "finalUrl"),
        raw.get("html").and_then(Value::as_str),
    ) else {
        return Err(EngineError::Malformed);
    };
    Ok(Some(BookMetadataPage {
        final_url,
        html: html.to_string(),
    }))
}

/// `images.dominantColor` → `Option<"rgb(r g b)">` for a URL or local path.
pub fn dominant_color<B: ArkBridge>(
    bridge: &B,
    source: &str,
) -> Result<Option<String>, EngineError> {
    let raw = bridge.image_dominant_color(source)?;
    Ok(str_field(&raw, "color"))
}

/// `images.storeCover` — Engine copies the dropped file under
/// `extension-data/<app>/book-covers/` and returns the stored path.
pub fn store_cover<B: ArkBridge>(
    bridge: &B,
    source_path: &str,
    entry_id: &str,
) -> Result<String, EngineError> {
    let raw = bridge.image_store_cover(source_path, entry_id)?;
    str_field(&raw, "path").ok_or(EngineError::Malformed)
}

/// `images.fetch` → `(stored_path, color)` — Engine downloads the remote
/// image so the app never holds remote bytes or does DNS itself.
pub fn fetch_image<B: ArkBridge>(
    bridge: &B,
    url: &str,
) -> Result<(String, Option<String>), EngineError> {
    let raw = bridge.image_fetch(url.trim())?;
    let path = str_field(&raw, "path").ok_or(EngineError::Malformed)?;
    Ok((path, str_field(&raw, "color")))
}
