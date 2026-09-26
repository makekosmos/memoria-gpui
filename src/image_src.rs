//! `ImageObjectView.vue` `imageSrc` / `getEntryImageSrc` ports — resolves the
//! displayable image source for an image-type entry: `headerProps.image` when
//! it already looks resolvable (URL/absolute/drive path), else `source_path`,
//! else the first `![[wikilink]]`/`![](src)` image in the markdown body.
use serde_json::Value;

use crate::model::Entry;
use crate::object_views::parse_entry_header_props;

/// `toDisplayImageSrc` input — raw candidate source for an image entry.
pub fn entry_image_src(e: &Entry) -> Option<String> {
    let props = parse_entry_header_props(e);
    let get = |k: &str| {
        props
            .get(k)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    };
    let image = get("image").unwrap_or_default();
    let body = body_markdown(e);
    let raw = if looks_resolvable(image) {
        image
    } else {
        get("source_path")
            .or_else(|| first_markdown_image_src(&body))
            .unwrap_or(image)
    };
    if raw.is_empty() {
        None
    } else {
        Some(raw.to_string())
    }
}

/// Thumbnail src for object rows — `presentation.imageFieldId` header prop.
/// Relation-to-image resolution (TypeObjectsView `resolveObjectImageSrc`)
/// needs the collection map; that lookup is M5 — non-string values yield
/// no thumbnail here.
pub fn entry_thumbnail_src(e: &Entry, image_field_id: Option<&str>) -> Option<String> {
    let fid = image_field_id?;
    let props = parse_entry_header_props(e);
    props
        .get(fid)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| entry_image_src(e))
}

/// `^[a-z][a-z0-9+.-]*:` scheme / drive letter / UNC — ImageObjectView.vue.
fn looks_resolvable(s: &str) -> bool {
    let bytes = s.as_bytes();
    let scheme = s
        .find(':')
        .map(|i| {
            i > 0
                && s[..i].chars().next().map(|c| c.is_ascii_alphabetic()) == Some(true)
                && s[..i]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        })
        .unwrap_or(false);
    let drive = bytes.len() > 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/');
    scheme || drive || s.starts_with("\\\\")
}

fn body_markdown(e: &Entry) -> String {
    crate::content::read_entry_markdown(
        &serde_json::from_str(&e.content_json).unwrap_or(Value::Null),
    )
}

/// First `![[target]]` / `![alt](src)` src in markdown.
fn first_markdown_image_src(md: &str) -> Option<&str> {
    let bytes = md.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'!' && bytes[i + 1] == b'[' {
            if bytes.get(i + 2) == Some(&b'[') {
                let start = i + 3;
                let end = md[start..]
                    .find(|c| ['|', ']'].contains(&c))
                    .map(|o| start + o)?;
                let target = md[start..end].trim();
                if !target.is_empty() {
                    return Some(target);
                }
            } else if let Some(start) = md[i..].find("](").map(|o| i + o + 2) {
                if let Some(end) = md[start..].find(')') {
                    let src = md[start..start + end]
                        .split_whitespace()
                        .next()
                        .unwrap_or("");
                    if !src.is_empty() {
                        return Some(src);
                    }
                }
            }
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn image_entry(props: Value, body: &str) -> Entry {
        Entry {
            id: "e1".into(),
            title: "img".into(),
            content_json: crate::content::write_entry_markdown(body).to_string(),
            header_props_json: Some(props.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn prefers_resolvable_header_image() {
        let e = image_entry(
            json!({"image": "file:///a.png", "source_path": "/b.png"}),
            "",
        );
        assert_eq!(entry_image_src(&e).as_deref(), Some("file:///a.png"));
    }

    #[test]
    fn falls_back_to_source_path_then_body() {
        let e = image_entry(json!({"source_path": "/pics/a.png"}), "![[other.png]]");
        assert_eq!(entry_image_src(&e).as_deref(), Some("/pics/a.png"));
        let e2 = image_entry(json!({}), "text ![alt](body.png) rest");
        assert_eq!(entry_image_src(&e2).as_deref(), Some("body.png"));
        let e3 = image_entry(json!({}), "text ![[wiki.png|100]] rest");
        assert_eq!(entry_image_src(&e3).as_deref(), Some("wiki.png"));
    }

    #[test]
    fn empty_means_no_image() {
        let e = image_entry(json!({}), "no images");
        assert_eq!(entry_image_src(&e), None);
    }
}
