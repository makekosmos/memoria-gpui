//! Port of `obsidianVaultImportImages.ts` — vault image asset index, markdown
//! image-reference extraction/rewrite, and legacy/canonical id helpers.

use std::sync::LazyLock;

use unicode_normalization::UnicodeNormalization;

// Re-exported so existing `images::…` paths keep working after the split.
pub use super::image_index::{
    build_image_asset_map, resolve_image_asset, ImageAssetCollision, ImageAssetIndex,
    ImageResolution,
};

/// `/!\[([^\]]*)\]\(([^)\n]+)\)|!\[\[([^|\]]+)(?:\|([^\]]+))?\]\]/g`.
pub(crate) static IMAGE_MARKDOWN_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"!\[([^\]]*)\]\(([^)\n]+)\)|!\[\[([^|\]]+)(?:\|([^\]]+))?\]\]")
        .expect("image markdown")
});

/// `extractMarkdownImageRefs`.
pub fn extract_markdown_image_refs(markdown: &str) -> Vec<String> {
    IMAGE_MARKDOWN_RE
        .captures_iter(markdown)
        .filter_map(|m| {
            let target = m
                .get(2)
                .or_else(|| m.get(3))
                .map(|g| g.as_str())
                .unwrap_or_default();
            let parsed = parse_markdown_image_target(target);
            (!parsed.is_empty()).then_some(parsed)
        })
        .collect()
}

/// `rewriteImageReferences` — resolved refs become `![label](fileUrl)`;
/// everything else keeps the raw match.
pub fn rewrite_image_references(
    markdown: &str,
    source_relative_path: &str,
    image_assets: &ImageAssetIndex,
    resolutions: &mut Vec<ImageResolution>,
) -> String {
    let normalized_source_path = normalize_path(source_relative_path);
    let source_dir = normalized_source_path
        .rfind('/')
        .map(|i| &normalized_source_path[..i])
        .unwrap_or_default();

    let mut out = String::with_capacity(markdown.len());
    let mut cursor = 0;
    for m in IMAGE_MARKDOWN_RE.captures_iter(markdown) {
        let whole = m.get(0).expect("full match");
        out.push_str(&markdown[cursor..whole.start()]);
        cursor = whole.end();
        let alt = m.get(1).map(|g| g.as_str()).unwrap_or_default();
        let raw_target = m
            .get(2)
            .or_else(|| m.get(3))
            .map(|g| g.as_str())
            .unwrap_or_default();
        let src = parse_markdown_image_target(raw_target);
        let resolution = resolve_image_asset(&src, source_dir, image_assets);
        let rewritten = resolution.asset.as_ref().map(|asset| {
            let label = if alt.is_empty() {
                title_from_path(&asset.relative_path)
            } else {
                alt.to_string()
            };
            format!("![{}]({})", label.trim(), asset.file_url)
        });
        resolutions.push(resolution);
        out.push_str(rewritten.as_deref().unwrap_or(whole.as_str()));
    }
    out.push_str(&markdown[cursor..]);
    out
}

/// `titleFromPath` — strips the last `.ext` (only when a suffix follows).
pub fn title_from_path(relative_path: &str) -> String {
    let normalized = normalize_path(relative_path);
    let file_name = normalized.rsplit('/').next().unwrap_or(relative_path);
    let stem = file_name
        .rfind('.')
        .filter(|i| *i + 1 < file_name.len())
        .map(|i| &file_name[..i])
        .unwrap_or(file_name);
    let trimmed = stem.trim();
    if trimmed.is_empty() {
        "Без названия".to_string()
    } else {
        trimmed.to_string()
    }
}

/// `stableIdFromPath` — FNV-1a with JS `for (const ch of s) hash ^=
/// ch.charCodeAt(0)` semantics: `for..of` iterates code points but
/// `charCodeAt(0)` reads the FIRST UTF-16 code unit, so astral characters
/// hash only their high surrogate (same rule as `sticker_window_key_for`).
pub fn stable_id_from_path(value: &str) -> String {
    let mut hash: u32 = 0x811c9dc5;
    let mut buf = [0u16; 2];
    for ch in normalize_path(value).chars() {
        let unit = ch.encode_utf16(&mut buf)[0];
        hash = (hash ^ unit as u32).wrapping_mul(0x01000193);
    }
    format!("{hash:08x}")
}

/// `legacyImageId`.
pub fn legacy_image_id(value: &str) -> String {
    format!("image:{}", stable_id_from_path(value))
}

pub(crate) fn safe_decode(value: &str) -> String {
    percent_encoding::percent_decode_str(value)
        .decode_utf8()
        .map(|v| v.into_owned())
        .unwrap_or_else(|_| value.to_string())
}

/// `parseMarkdownImageTarget` — `<…>` wrappers and trailing `"title"`/
/// `'title'` blocks are stripped.
pub fn parse_markdown_image_target(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Some(rest) = trimmed.strip_prefix('<') {
        if !rest.is_empty() {
            if let Some(close) = rest.find('>') {
                return rest[..close].trim().to_string();
            }
        }
    }
    static TITLE_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r#"\s+(?:"[^"]*"|'[^']*')\s*$"#).expect("title"));
    if let Some(m) = TITLE_RE.find(trimmed) {
        if m.start() > 0 {
            return trimmed[..m.start()].trim().to_string();
        }
    }
    trimmed.to_string()
}

/// `normalizePath` — `\` → `/`, leading `/` stripped, `.` skipped, `..` pops.
pub fn normalize_path(value: &str) -> String {
    let replaced = value.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for part in replaced.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    parts.join("/")
}

/// `canonicalPathIdentity` — `normalizePath` + NFC + lowercase.
pub fn canonical_path_identity(value: &str) -> String {
    normalize_path(value)
        .nfc()
        .collect::<String>()
        .to_lowercase()
}

pub(crate) fn fold_path(value: &str) -> String {
    canonical_path_identity(value)
}

pub(crate) fn normalize_relative_path(source_dir: &str, value: &str) -> Option<String> {
    let normalized_dir = normalize_path(source_dir);
    let mut parts: Vec<&str> = normalized_dir
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    let replaced = value.replace('\\', "/");
    for part in replaced.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.is_empty() {
                    return None;
                }
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

pub(crate) fn is_absolute_path(value: &str) -> bool {
    static RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^(?:[a-zA-Z]:[\\/]|[\\/]{2}|/)").expect("abs"));
    RE.is_match(value)
}
