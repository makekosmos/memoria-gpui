//! Note-card preview text — `EverythingItemCard.vue` shows up to 800 chars of
//! plain text extracted from the entry body. Content is stored as markdown
//! (`read_entry_markdown`), so the preview strips markdown syntax rather than
//! walking a tiptap doc.

use serde_json::Value;

use crate::content::read_entry_markdown;

/// `PREVIEW_LIMIT` in EverythingItemCard.vue.
pub const PREVIEW_LIMIT: usize = 800;
mod scan;
mod syntax;

/// Markdown → plain text. Blocks collapse to single newlines; inline markers
/// (`**`, `` ` ``, `[[…]]`, `![…](…)`, `#`, `>`) are dropped. Not a full
/// markdown parser — a card preview only needs legible text.
pub fn markdown_plain_text(markdown: &str) -> String {
    scan::plain_text(markdown)
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
    fn keeps_literal_text_chars() {
        // KOS-249: `# > * _ ~ ! [ ] ( )` were dropped unconditionally, so
        // prose lost real characters.
        assert_eq!(
            markdown_plain_text("используй C# и F#"),
            "используй C# и F#"
        );
        assert_eq!(markdown_plain_text("a (b) c"), "a (b) c");
        assert_eq!(
            markdown_plain_text("snake_case и 2*3 и a~b"),
            "snake_case и 2*3 и a~b"
        );
        assert_eq!(markdown_plain_text("see file a*"), "see file a*");
        assert_eq!(markdown_plain_text("wow! done"), "wow! done");
        assert_eq!(
            markdown_plain_text("экранированный \\* не italic"),
            "экранированный * не italic"
        );
    }

    #[test]
    fn link_and_wiki_and_image_syntax() {
        assert_eq!(
            markdown_plain_text("см. [текст](https://example.com) после"),
            "см. текст после"
        );
        assert_eq!(
            markdown_plain_text("wiki [[страница|метка]] и [[другая]]"),
            "wiki метка и другая"
        );
        assert_eq!(
            markdown_plain_text("картинка ![alt](x.png) конец"),
            "картинка  конец"
        );
        assert_eq!(markdown_plain_text("a ] b ) c"), "a ] b ) c");
    }

    #[test]
    fn emphasis_pairs_drop() {
        assert_eq!(
            markdown_plain_text("**b** и *e* и _u_ и ~~s~~"),
            "b и e и u и s"
        );
        assert_eq!(markdown_plain_text("# Foo #\n\nок"), "Foo\nок");
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
