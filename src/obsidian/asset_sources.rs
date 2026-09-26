//! Port of `obsidianVaultExportAssetPaths.ts` — local asset *source*
//! resolution: `file:`/`kosmos-local-image:`/absolute-path handling and the
//! copyability preflight. Path planning lives in `export_paths.rs`.

use std::sync::LazyLock;

use super::export_paths::normalize_export_path;
pub(crate) fn canonical_local_asset_key(source_path: &str) -> String {
    let resolved = extract_copyable_local_asset_filesystem_path(source_path)
        .unwrap_or_else(|| source_path.trim().to_string());
    normalize_export_path(&resolved).to_lowercase()
}

fn extract_copyable_local_asset_filesystem_path(source_path: &str) -> Option<String> {
    let trimmed = source_path.trim();
    if trimmed.is_empty() {
        return None;
    }
    static SCHEME_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)^(file:|kosmos-local-image:)").expect("scheme"));
    if SCHEME_RE.is_match(trimmed) {
        return parse_url_pathname(trimmed).map(|p| decode_local_asset_url_path(&p));
    }
    Some(trimmed.to_string())
}

/// `new URL(source).pathname` — covers `file:///x` and
/// `kosmos-local-image://file/<encoded>` (opaque path after the authority).
fn parse_url_pathname(value: &str) -> Option<String> {
    let rest = value.split_once("://")?.1;
    let path = rest.find('/').map(|i| &rest[i..]).unwrap_or("");
    Some(path.to_string())
}

fn decode_local_asset_url_path(pathname: &str) -> String {
    let decoded = percent_encoding::percent_decode_str(pathname)
        .decode_utf8()
        .unwrap_or_else(|_| pathname.into());
    static WIN_DRIVE_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^/([A-Za-z]:[\\/])").expect("drive"));
    let decoded = WIN_DRIVE_RE.replace(&decoded, "$1").into_owned();
    static LEADING_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^/+").expect("leading"));
    LEADING_RE.replace(&decoded, "/").into_owned()
}

pub(crate) fn resolve_copyable_local_asset_source_path(source: &str) -> Option<String> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return None;
    }
    static SCHEME_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)^(file:|kosmos-local-image:)").expect("scheme"));
    if SCHEME_RE.is_match(trimmed) {
        return parse_url_pathname(trimmed).map(|_| trimmed.to_string());
    }
    static ABS_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^[A-Za-z]:[\\/]|^\\\\|^/").expect("abs"));
    if ABS_RE.is_match(trimmed) {
        return Some(trimmed.to_string());
    }
    None
}

pub(crate) fn extract_local_asset_path(source: &str) -> Option<String> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return None;
    }
    static SCHEME_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)^(file:|kosmos-local-image:)").expect("scheme"));
    if SCHEME_RE.is_match(trimmed) {
        let pathname = parse_url_pathname(trimmed)?;
        let decoded = percent_encoding::percent_decode_str(&pathname)
            .decode_utf8()
            .map(|c| c.into_owned())
            .unwrap_or(pathname);
        let decoded = decoded.trim_start_matches('/').to_string();
        return (!decoded.is_empty()).then_some(decoded);
    }
    static ABS_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^[A-Za-z]:[\\/]|^\\\\|^/").expect("abs"));
    if ABS_RE.is_match(trimmed) {
        return Some(trimmed.to_string());
    }
    static OTHER_SCHEME_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)^[a-z]+:").expect("scheme"));
    if OTHER_SCHEME_RE.is_match(trimmed) {
        return None;
    }
    Some(trimmed.to_string())
}
