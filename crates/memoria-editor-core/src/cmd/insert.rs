//! Insert commands: link, image, local-image src handling mirroring
//! `src/lib/localImages.ts` from the Vue editor.

use crate::cursor::Selection;
use crate::editor::Tx;

/// Same protocol as `localImages.ts`: `kosmos-local-image://file/<enc path>`.
pub const LOCAL_IMAGE_PROTOCOL: &str = "kosmos-local-image";

/// `localImageUrl(filePath)` — the display URL for a local file path.
pub fn local_image_url(file_path: &str) -> String {
    format!("{LOCAL_IMAGE_PROTOCOL}://file/{}", encode_path(file_path))
}

/// `toDisplayImageSrc(src)` — maps a stored markdown image src to the URL a
/// renderer should load (windows paths and `file:` URLs become
/// `kosmos-local-image://`).
pub fn display_image_src(src: &str) -> String {
    let t = src.trim();
    // `C:\...` or `\\share\...`
    if t.len() >= 3
        && t.as_bytes()[1] == b':'
        && matches!(t.as_bytes()[2], b'\\' | b'/')
        && t.as_bytes()[0].is_ascii_alphabetic()
    {
        return local_image_url(t);
    }
    if t.starts_with("\\\\") {
        return local_image_url(t);
    }
    if !t.starts_with("file:") {
        return t.to_string();
    }
    // file:// URL → decode path, drop `file://` + optional host
    let rest = &t[5..];
    let rest = rest.strip_prefix("//").unwrap_or(rest);
    let path = percent_decode(rest);
    let path = if path.len() > 2
        && path.as_bytes()[0] == b'/'
        && path.as_bytes()[2] == b':'
        && path.as_bytes()[1].is_ascii_alphabetic()
    {
        path[1..].to_string()
    } else {
        path
    };
    local_image_url(&path)
}

/// Insert `[label](dest)` / `[label](dest "title")`. Selected text becomes
/// the label; empty selection leaves the caret inside `[]`.
pub fn insert_link(src: &str, sel: Selection, dest: &str, title: Option<&str>) -> Tx {
    let (s, e) = (
        crate::cmd::util::snap_down(src, sel.start()),
        crate::cmd::util::snap_up(src, sel.end()),
    );
    let mut tx = Tx::new();
    let title_part = title.map(|t| format!(" \"{t}\"")).unwrap_or_default();
    if s == e {
        let text = format!("[]({dest}{title_part})");
        tx.insert(s, &text);
        tx.selection(Selection::caret(s + 1));
    } else {
        tx.insert(e, format!("]({dest}{title_part})"));
        tx.insert(s, "[");
        tx.selection(Selection::caret(e + 1 + dest.len() + title_part.len() + 1));
    }
    tx
}

/// Insert `![alt](src)` / `![alt](src "title")`; caret after the image.
pub fn insert_image(
    src: &str,
    sel: Selection,
    img_src: &str,
    alt: &str,
    title: Option<&str>,
) -> Tx {
    let (s, e) = (
        crate::cmd::util::snap_down(src, sel.start()),
        crate::cmd::util::snap_up(src, sel.end()),
    );
    let title_part = title.map(|t| format!(" \"{t}\"")).unwrap_or_default();
    let text = format!("![{alt}]({img_src}{title_part})");
    let mut tx = Tx::new();
    tx.replace(s, e, &text);
    tx.selection(Selection::caret(s + text.len()));
    tx
}

/// Insert an image referencing a local file path — stored markdown keeps the
/// bare path (like Vue `EdenImage` + `localImages.ts`); `display_image_src`
/// converts at render time.
pub fn insert_local_image(src: &str, sel: Selection, file_path: &str, alt: &str) -> Tx {
    insert_image(src, sel, file_path, alt, None)
}

fn encode_path(path: &str) -> String {
    // encodeURIComponent semantics: only RFC3986 unreserved chars stay
    // literal — `/` and `\` become `%2F`/`%5C`, matching Vue's
    // `encodeURIComponent(filePath)` byte-for-byte.
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        // Decode only `%HH` — operate on raw bytes: `s[i + 1..i + 3]` can
        // split a multi-byte UTF-8 char when `%` precedes it (`file:///%€`).
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = (bytes[i + 1] as char)
                .to_digit(16)
                .zip((bytes[i + 2] as char).to_digit(16));
            if let Some((hi, lo)) = hex {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
