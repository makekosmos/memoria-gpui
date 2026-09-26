//! HTML tokenizer + entity decoding for `html_to_markdown`.

#[derive(Debug)]
pub(super) enum Tok {
    Open {
        name: String,
        attrs: Vec<(String, String)>,
    },
    Close {
        name: String,
    },
    Text {
        text: String,
    },
}

pub(super) fn tokenize(html: &str) -> Vec<Tok> {
    let mut toks = Vec::new();
    let b = html.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' {
            let s = i;
            while i < b.len() && b[i] != b'<' {
                i += 1;
            }
            toks.push(Tok::Text {
                text: html[s..i].into(),
            });
            continue;
        }
        if html[i..].starts_with("<!--") {
            i = html[i..].find("-->").map(|j| i + j + 3).unwrap_or(b.len());
            continue;
        }
        if html[i..].starts_with("<!") || html[i..].starts_with("<?") {
            i = html[i..].find('>').map(|j| i + j + 1).unwrap_or(b.len());
            continue;
        }
        let close = b.get(i + 1) == Some(&b'/');
        let ns = i + if close { 2 } else { 1 };
        let mut j = ns;
        while j < b.len() && (b[j].is_ascii_alphanumeric() || matches!(b[j], b'-' | b':')) {
            j += 1;
        }
        if j == ns {
            toks.push(Tok::Text { text: "<".into() });
            i += 1;
            continue;
        }
        let name = html[ns..j].to_ascii_lowercase();
        let mut attrs = Vec::new();
        let mut k = j;
        while k < b.len() && b[k] != b'>' {
            while k < b.len() && b[k] != b'>' && b[k].is_ascii_whitespace() {
                k += 1;
            }
            if k >= b.len() || b[k] == b'>' || b[k] == b'/' {
                if b[k] == b'/' {
                    k += 1;
                }
                continue;
            }
            let as_ = k;
            while k < b.len() && !b[k].is_ascii_whitespace() && !matches!(b[k], b'=' | b'>') {
                k += 1;
            }
            let aname = html[as_..k].to_ascii_lowercase();
            if k < b.len() && b[k] == b'=' {
                k += 1;
                while k < b.len() && b[k].is_ascii_whitespace() {
                    k += 1;
                }
                let (val, nk) = if k < b.len() && matches!(b[k], b'"' | b'\'') {
                    let q = b[k] as char;
                    let vs = k + 1;
                    let ve = html[vs..].find(q).map(|x| vs + x).unwrap_or(b.len());
                    (html[vs..ve].to_string(), ve + 1)
                } else {
                    let vs = k;
                    while k < b.len() && !b[k].is_ascii_whitespace() && b[k] != b'>' {
                        k += 1;
                    }
                    (html[vs..k].to_string(), k)
                };
                attrs.push((aname, decode_entities(&val)));
                k = nk;
            } else {
                attrs.push((aname, String::new()));
            }
        }
        i = (k + 1).min(b.len());
        if close {
            toks.push(Tok::Close { name });
        } else {
            toks.push(Tok::Open { name, attrs });
        }
    }
    toks
}

pub(super) fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        rest = &rest[p..];
        let end = rest[1..].find(';').map(|e| e + 2).unwrap_or(0);
        if end == 0 || end > 12 {
            out.push('&');
            rest = &rest[1..];
            continue;
        }
        let ent = &rest[1..end - 1];
        let rep: Option<String> = match ent {
            "amp" => Some("&".into()),
            "lt" => Some("<".into()),
            "gt" => Some(">".into()),
            "quot" => Some("\"".into()),
            "apos" | "#39" => Some("'".into()),
            "nbsp" => Some(" ".into()),
            "mdash" => Some("—".into()),
            "ndash" => Some("–".into()),
            "hellip" => Some("…".into()),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                u32::from_str_radix(&ent[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
                    .map(|c| c.to_string())
            }
            _ if ent.starts_with('#') => ent[1..]
                .parse::<u32>()
                .ok()
                .and_then(char::from_u32)
                .map(|c| c.to_string()),
            _ => None,
        };
        match rep {
            Some(r) => {
                out.push_str(&r);
                rest = &rest[end..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}
