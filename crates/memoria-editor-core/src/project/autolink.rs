//! GFM-style bare autolinks (`https://x`, `www.x`, `a@b.c`) — pulldown-cmark
//! 0.13 does not implement the autolink extension, so we scan leaf text
//! ourselves. Only applied to plain `Text` leaves whose resolved text length
//! equals the source range (no escapes/entities inside), so byte offsets map
//! 1:1.

/// `(offset_in_text, len, dest)` per found bare link.
pub fn scan_bare_links(text: &str) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if let Some(len) = url_len(&text[i..]) {
            let end = trim_trailing(&text[i..i + len]);
            if end > 0 {
                let m = &text[i..i + end];
                let dest = if m.starts_with("www.") {
                    format!("http://{m}")
                } else {
                    m.to_string()
                };
                out.push((i, end, dest));
                i += end;
                continue;
            }
        }
        if let Some(len) = email_len(&text[i..]) {
            let end = trim_trailing(&text[i..i + len]);
            if end > 0 {
                out.push((i, end, format!("mailto:{}", &text[i..i + end])));
                i += end;
                continue;
            }
        }
        i += char_len(bytes[i]);
    }
    out
}

fn char_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b < 0xE0 {
        2
    } else if b < 0xF0 {
        3
    } else {
        4
    }
}

/// Length of an `http(s)://` / `www.` match at position 0, if any.
fn url_len(text: &str) -> Option<usize> {
    let lower = text.as_bytes();
    let (scheme_len, is_www) = if lower.starts_with(b"http://") {
        (7, false)
    } else if lower.starts_with(b"https://") {
        (8, false)
    } else if lower.starts_with(b"www.") {
        (4, true)
    } else {
        return None;
    };
    let rest = &text[scheme_len..];
    // Must start with a non-space, non-punctuation char (the domain).
    let first = rest.chars().next()?;
    if !first.is_alphanumeric() {
        return None;
    }
    // GFM: a www. link needs a dot in the domain part.
    let len = scheme_len
        + rest
            .char_indices()
            .take_while(|(_, c)| is_url_char(*c))
            .map(|(i, c)| i + c.len_utf8())
            .last()
            .unwrap_or(0);
    if is_www && !text[..len].contains('.') {
        return None;
    }
    // Require at least `x.y` or a path after the scheme for usefulness.
    let host = &text[scheme_len..len];
    if !is_www && !host.contains('.') && !host.contains('/') {
        return None;
    }
    Some(len)
}

fn is_url_char(c: char) -> bool {
    !(c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'' | '`' | '[' | ']' | '{' | '}'))
}

/// Simple `a@b.c` matcher: local part then domain with a dot.
fn email_len(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() && is_local(bytes[i]) {
        i += 1;
    }
    if i == 0 || i >= bytes.len() || bytes[i] != b'@' {
        return None;
    }
    i += 1;
    let dom_start = i;
    while i < bytes.len()
        && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'.' | b'-' | b'_'))
    {
        i += 1;
    }
    let dom = &text[dom_start..i];
    (dom.contains('.') && !dom.starts_with('.') && !dom.ends_with('.')).then_some(i)
}

fn is_local(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'%' | b'+' | b'-')
}

/// GFM strips trailing punctuation and unbalanced `)`/`]` from the link.
fn trim_trailing(m: &str) -> usize {
    let mut end = m.len();
    loop {
        let Some(c) = m[..end].chars().last() else {
            return 0;
        };
        let strip = match c {
            '.' | ',' | ':' | ';' | '!' | '?' | '\'' | '*' | '_' | '~' => true,
            ')' => m[..end].matches('(').count() < m[..end].matches(')').count(),
            ']' => m[..end].matches('[').count() < m[..end].matches(']').count(),
            _ => false,
        };
        if !strip {
            return end;
        }
        end -= c.len_utf8();
    }
}
