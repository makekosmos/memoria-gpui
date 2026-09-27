//! Clipboard paste tests: plain text, markdown as-is, HTML→markdown.

use memoria_editor_core::html2md::html_to_markdown;
use memoria_editor_core::{Editor, Selection};

// --- paste --------------------------------------------------------------------

#[test]
fn plain_text() {
    let mut e = Editor::new("ab");
    e.set_selection(Selection::caret(1));
    e.paste(Some("XY"), None);
    assert_eq!(e.text(), "aXYb");
}

#[test]
fn markdown_as_is() {
    let mut e = Editor::new("");
    e.paste(Some("- [ ] t\n**b**\n"), None);
    assert_eq!(e.text(), "- [ ] t\n**b**\n");
}

#[test]
fn html_browser_fragment() {
    let html = "<p>Hello <b>bold</b> and <i>it</i></p><ul><li>one</li><li>two</li></ul>";
    let md = html_to_markdown(html);
    assert!(md.contains("Hello **bold** and *it*"), "{md}");
    assert!(md.contains("- one\n- two"), "{md}");
}

#[test]
fn html_heading_table_link() {
    let html = "<h2>T</h2><p><a href=\"https://x.y\">L</a></p><table><tr><th>A</th></tr><tr><td>1</td></tr></table>";
    let md = html_to_markdown(html);
    assert!(md.contains("## T"), "{md}");
    assert!(md.contains("[L](https://x.y)"), "{md}");
    assert!(md.contains("| A |"), "{md}");
    assert!(md.contains("| 1 |"), "{md}");
}

#[test]
fn html_table_delimiter_matches_column_count() {
    // KOS-219: the delimiter row used to be a single `| ---` regardless of
    // the header's cell count, so pasted multi-column tables reparsed as a
    // plain paragraph of literal pipes instead of a table.
    let html = "<table><tr><th>a</th><th>b</th></tr><tr><td>1</td><td>2</td></tr></table>";
    let md = html_to_markdown(html);
    assert_eq!(md, "| a | b |\n| --- | --- |\n| 1 | 2 |\n");
    let doc = memoria_editor_core::md::parse::parse(&md);
    assert!(doc.children.iter().any(|n| matches!(
        n,
        memoria_editor_core::md::ast::Node::Block {
            kind: memoria_editor_core::md::ast::BlockKind::Table { .. },
            ..
        }
    )));
}

#[test]
fn html_word_checkbox_quote_pre() {
    let html = "<blockquote><p>q</p></blockquote><ul><li><input type=\"checkbox\" checked>t</li></ul><pre>code\nline2</pre>";
    let md = html_to_markdown(html);
    assert!(md.contains("> q"), "{md}");
    assert!(md.contains("- [x] t"), "{md}");
    assert!(md.contains("```\ncode\nline2\n```"), "{md}");
}

#[test]
fn html_entities_and_script() {
    let html = "<p>a &amp; b&nbsp;c</p><script>bad()</script>";
    let md = html_to_markdown(html);
    assert_eq!(md.trim(), "a & b c");
}

#[test]
fn prefers_html_over_plain() {
    let mut e = Editor::new("");
    e.paste(Some("plain"), Some("<b>bold</b>"));
    assert_eq!(e.text(), "**bold**\n");
}

#[test]
fn meta_link_void_tags_do_not_drop_rest() {
    // `<meta>`/`<link>` are void elements — the tokenizer only emits Open for
    // them, so the renderer's skip counter must not wait for a Close.
    // (KOS-190: browser clipboard markup starts with `<head><meta…>` and
    // previously the whole body pasted as an empty string.)
    let html = "<html><head><meta charset=\"utf-8\"><link rel=\"x\"></head><body><p>kept</p></body></html>";
    let md = html_to_markdown(html);
    assert!(md.contains("kept"), "{md:?}");
}
