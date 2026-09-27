//! Note-card preview text — `EverythingItemCard.vue` shows up to 800 chars of
//! plain text extracted from the entry body. Content is stored as markdown
//! (`read_entry_markdown`), so the preview strips markdown syntax rather than
//! walking a tiptap doc.

use serde_json::Value;

use crate::content::read_entry_markdown;

/// `PREVIEW_LIMIT` in EverythingItemCard.vue.
pub const PREVIEW_LIMIT: usize = 800;

/// Skip leading whitespace plus an optional `- `/`+ `/`N. ` list marker at
/// a line start.
fn skip_list_marker(chars: &mut std::iter::Peekable<std::str::Chars>) {
    while matches!(chars.clone().next(), Some(' ') | Some('\t')) {
        chars.next();
    }
    match chars.clone().next() {
        Some('-') | Some('+') => {
            let mut clone = chars.clone();
            clone.next();
            if clone.next() == Some(' ') {
                chars.next();
                chars.next();
            }
        }
        Some(d) if d.is_ascii_digit() => {
            // "1. " ordered list marker.
            let mut clone = chars.clone();
            let mut digits = 0usize;
            while matches!(clone.clone().next(), Some(c) if c.is_ascii_digit()) {
                clone.next();
                digits += 1;
            }
            if digits > 0 && clone.next() == Some('.') && clone.next() == Some(' ') {
                for _ in 0..digits + 2 {
                    chars.next();
                }
            }
        }
        _ => {}
    }
}

/// Markdown → plain text. Blocks collapse to single newlines; inline markers
/// (`**`, `` ` ``, `[[…]]`, `![…](…)`, `#`, `>`) are dropped. Not a full
/// markdown parser — a card preview only needs legible text.
pub fn markdown_plain_text(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut chars = markdown.chars().peekable();
    let mut line_start = true;
    let mut in_code_fence = false;
    skip_list_marker(&mut chars);
    while let Some(ch) = chars.next() {
        if in_code_fence {
            if ch == '`' && chars.clone().take(2).eq("``".chars()) {
                chars.next();
                chars.next();
                in_code_fence = false;
            } else {
                out.push(ch);
            }
            continue;
        }
        match ch {
            '`' => {
                // ``` fence → code mode; single ` → skip the marker only.
                if chars.clone().take(2).eq("``".chars()) {
                    chars.next();
                    chars.next();
                    in_code_fence = true;
                }
            }
            '#' | '>' | '*' | '_' | '~' => {
                if !line_start {
                    // Emphasis markers mid-line are dropped unless escaped.
                }
            }
            '\n' => {
                if !out.ends_with('\n') && !out.is_empty() {
                    out.push('\n');
                }
                line_start = true;
                skip_list_marker(&mut chars);
                continue;
            }
            '!' | '[' | ']' | '(' | ')' => {}
            _ => {
                out.push(ch);
            }
        }
        if ch != '\n' {
            line_start = false;
        }
    }
    // Collapse 3+ newlines (Vue `tiptapPlainText` does the same).
    let mut collapsed = String::with_capacity(out.len());
    let mut newlines = 0;
    for ch in out.chars() {
        if ch == '\n' {
            newlines += 1;
            if newlines <= 2 {
                collapsed.push(ch);
            }
        } else {
            newlines = 0;
            collapsed.push(ch);
        }
    }
    collapsed.trim().to_string()
}

/// `previewForEntry` — markdown body → ≤800 chars of plain text.
pub fn entry_preview(content_json: &str, limit: usize) -> String {
    let raw: Value = serde_json::from_str(content_json).unwrap_or(Value::Null);
    let markdown = read_entry_markdown(&raw);
    let text = markdown_plain_text(&markdown);
    text.chars().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strips_markdown() {
        let md = "# Заголовок\n\nПервый **абзац** с `кодом`.\n- пункт один\n- пункт два\n\n> цитата\n\n![img](x.png)";
        let text = markdown_plain_text(md);
        assert!(text.contains("Заголовок"));
        assert!(text.contains("абзац"));
        assert!(text.contains("пункт один"));
        assert!(!text.contains("**"));
        assert!(!text.contains('!'), "{text}");
    }

    #[test]
    fn strips_ordered_list_markers() {
        // KOS-219: `1. ` markers leaked into card previews — the lookahead
        // checked the wrong char after `.`, so only `1.  x` (two spaces)
        // was ever stripped; and a list marker on the first line was never
        // stripped at all.
        let text = markdown_plain_text("1. first\n2. second\n\n- bullet\nplain");
        assert_eq!(text, "first\nsecond\nbullet\nplain");
        let text = markdown_plain_text("12. dozen\n13.b not a marker");
        assert_eq!(text, "dozen\n13.b not a marker");
    }

    #[test]
    fn respects_limit() {
        let long = "слово ".repeat(500);
        let out = entry_preview(
            &json!({"kind":"markdown","text": long}).to_string(),
            PREVIEW_LIMIT,
        );
        assert!(out.chars().count() <= PREVIEW_LIMIT);
    }

    #[test]
    fn tiptap_doc_projects_to_markdown_first() {
        // content::read_entry_markdown handles tiptap docs too.
        let doc = json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Привет мир"}]}]
        });
        let out = entry_preview(&doc.to_string(), PREVIEW_LIMIT);
        assert_eq!(out, "Привет мир");
    }
}
