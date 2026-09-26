//! `sticker.ts`/`stickerRoutes.ts` port — sticker route helpers and the
//! host window-key derivation (Vue-compatible `encodeURIComponent` surface).
//! `/sticker/<urlencoded-entry-id>` is distinct from `/note/<id>`.

use crate::routes::parse_entry_route_id;

const STICKER_ROUTE_PREFIX: &str = "/sticker/";
pub(crate) const MAX_ENTRY_ID_LENGTH: usize = 200;

/// `stickerRouteFor(entryId)` — `/sticker/` + `encodeURIComponent(id)`.
pub fn sticker_route_for(entry_id: &str) -> String {
    format!("{STICKER_ROUTE_PREFIX}{}", encode_uri_component(entry_id))
}

/// `stickerWindowKeyFor(entryId)` — host-safe `sticker:<id>` or a
/// deterministic fnv-1a base36 tag when the id isn't safe.
pub fn sticker_window_key_for(entry_id: &str) -> String {
    let raw = format!("sticker:{entry_id}");
    if is_host_safe_key(&raw) {
        return raw;
    }
    let mut h: u32 = 0x811c9dc5;
    for ch in entry_id.chars() {
        h = (h ^ ch as u32).wrapping_mul(0x01000193);
    }
    format!("sticker:{}", to_base36(h))
}

/// Vue `isHostSafeKey` — `/^[A-Za-z0-9][A-Za-z0-9._:%-]{0,63}$/`.
fn is_host_safe_key(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    if bytes.is_empty() || bytes.len() > 64 {
        return false;
    }
    bytes[0].is_ascii_alphanumeric()
        && bytes[1..]
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'%' | b'-'))
}

fn to_base36(mut n: u32) -> String {
    if n == 0 {
        return "0".into();
    }
    let mut out = Vec::new();
    while n > 0 {
        let d = (n % 36) as u8;
        out.push(if d < 10 { b'0' + d } else { b'a' + d - 10 });
        n /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap()
}

/// `parseKeplerStickerRoute` → entry id.
pub fn parse_sticker_route(route: &str) -> Option<String> {
    parse_entry_route_id(route, STICKER_ROUTE_PREFIX)
}

/// `canOpenInSticker` — stickers render only Tiptap-able types (note/book).
pub fn can_open_in_sticker(type_id: Option<&str>) -> bool {
    matches!(
        type_id,
        Some(crate::system_types_data::SYSTEM_TYPE_NOTE_ID)
            | Some(crate::system_types_data::SYSTEM_TYPE_BOOK_ID)
    )
}

/// `encodeURIComponent` — ECMA-262 unreserved set kept verbatim.
pub(crate) fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        let ok = b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
            );
        if ok {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `decodeURIComponent` — `%XX` → bytes → UTF-8 (None on malformed input).
pub(crate) fn decode_uri_component(s: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(s.len());
    let mut it = s.bytes().peekable();
    while let Some(b) = it.next() {
        if b == b'%' {
            let hi = it.next()?;
            let lo = it.next()?;
            let hex = |c: u8| (c as char).to_digit(16);
            bytes.push(((hex(hi)? << 4) | hex(lo)?) as u8);
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sticker_routes() {
        // Port of stickerRoutes.test.ts expectations.
        assert_eq!(
            parse_sticker_route("/sticker/note%3Aabc-123"),
            Some("note:abc-123".into())
        );
        assert_eq!(
            parse_sticker_route("/sticker/550e8400-e29b-41d4-a716-446655440000"),
            Some("550e8400-e29b-41d4-a716-446655440000".into())
        );
        // Vue "rejects malformed and unsafe sticker routes" table.
        assert_eq!(parse_sticker_route("/sticker/"), None);
        assert_eq!(parse_sticker_route("/sticker/%E0%A4%A"), None);
        assert_eq!(parse_sticker_route("/sticker/id/other"), None);
        assert_eq!(parse_sticker_route("/sticker/id%3Fquery"), None);
        assert_eq!(parse_sticker_route("/sticker/id%23fragment"), None);
        assert_eq!(parse_sticker_route("/sticker/id%5Cother"), None);
        assert_eq!(parse_sticker_route("/sticker/id%20other"), None);
        assert_eq!(parse_sticker_route("/sticker/id%00"), None);
        assert_eq!(parse_sticker_route("/note/abc"), None);
        // A decoded slash is legal — round-trip ids like `note:abc/one`.
        assert_eq!(
            parse_sticker_route("/sticker/note%3Aabc%2Fone"),
            Some("note:abc/one".into())
        );
        let long = "x".repeat(201);
        assert_eq!(parse_sticker_route(&format!("/sticker/{long}")), None);
    }

    #[test]
    fn sticker_roundtrip_and_window_keys() {
        let id = "note:xyz-42";
        let route = sticker_route_for(id);
        assert_eq!(route, "/sticker/note%3Axyz-42");
        assert_eq!(parse_sticker_route(&route), Some(id.into()));
        assert_eq!(sticker_window_key_for(id), "sticker:note:xyz-42");
        let unsafe_key = sticker_window_key_for("weird id/√");
        assert!(unsafe_key.starts_with("sticker:"));
        assert!(!unsafe_key.contains(' '));
        assert_eq!(sticker_window_key_for("weird id/√"), unsafe_key);
    }

    #[test]
    fn can_open_in_sticker_types() {
        assert!(can_open_in_sticker(Some(
            crate::system_types_data::SYSTEM_TYPE_NOTE_ID
        )));
        assert!(can_open_in_sticker(Some(
            crate::system_types_data::SYSTEM_TYPE_BOOK_ID
        )));
        assert!(!can_open_in_sticker(Some(
            crate::system_types_data::SYSTEM_TYPE_IMAGE_ID
        )));
        assert!(!can_open_in_sticker(None));
    }
}
