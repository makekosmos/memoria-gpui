//! Live-preview projection tests: every hide-marker rule, the
//! cursor-touch reveal, table layout, and the source↔visible map.

use memoria_editor_core::md::parse;
use memoria_editor_core::project::{project, Marks, Payload};
use memoria_editor_core::Selection;

fn vis(src: &str, caret: usize) -> String {
    let doc = parse(src);
    project(&doc, src, Selection::caret(caret)).text
}

fn vis_sel(src: &str, a: usize, b: usize) -> String {
    let doc = parse(src);
    project(&doc, src, Selection::new(a, b)).text
}

// --- hide-marker rules ------------------------------------------------------

#[test]
fn hides_bold_markers() {
    assert_eq!(vis("a **b** c", 0), "a b c");
}

#[test]
fn hides_italic_underscore() {
    assert_eq!(vis("a _b_ c", 0), "a b c");
}

#[test]
fn hides_inline_code_ticks() {
    assert_eq!(vis("a `b` c", 0), "a b c");
}

#[test]
fn hides_heading_hash() {
    assert_eq!(vis("# T\n", 40), "T");
    assert_eq!(vis("### T\nrest", 60), "T\nrest");
}

#[test]
fn hides_quote_marker() {
    assert_eq!(vis("> q\n> r\n", 40), "q\nr");
}

#[test]
fn hides_strike() {
    assert_eq!(vis("a ~~b~~ c", 0), "a b c");
}

#[test]
fn hides_task_mark_into_checkbox() {
    assert_eq!(vis("- [ ] t\n", 40), "☐ t");
    assert_eq!(vis("- [x] t\n", 40), "☑ t");
}

#[test]
fn bullet_becomes_dot() {
    assert_eq!(vis("- a\n- b\n", 40), "• a\n• b");
}

#[test]
fn ordered_marker_stays() {
    assert_eq!(vis("1. a\n2. b\n", 40), "1. a\n2. b");
}

#[test]
fn hides_fences() {
    let src = "```rs\nx\n```\n";
    assert_eq!(vis(src, src.len()), "x");
}

#[test]
fn hides_link_syntax() {
    assert_eq!(vis("a [t](https://x.y) b", 0), "a t b");
    // autolink
    assert_eq!(vis("<https://x.y>", 40), "https://x.y");
}

#[test]
fn image_shows_alt_as_caption() {
    let src = "![pic](img.png)\n";
    assert_eq!(vis(src, src.len()), "pic");
    let src = "a ![p](i.png) b\n";
    assert_eq!(vis(src, src.len()), "a p b");
}

#[test]
fn rule_stays_visible() {
    assert!(vis("a\n\n---\n\nb", 0).contains("---"));
}

// --- reveal on touch --------------------------------------------------------

#[test]
fn caret_inside_bold_reveals() {
    assert_eq!(vis("a **bc** d", 6), "a **bc** d");
}

#[test]
fn caret_before_word_reveals() {
    // caret at `**` boundary touches the construct
    assert_eq!(vis("a **bc** d", 3), "a **bc** d");
}

#[test]
fn selection_touching_reveals() {
    assert_eq!(vis_sel("a **bc** d", 6, 7), "a **bc** d");
}

#[test]
fn selection_outside_keeps_hidden() {
    assert_eq!(vis_sel("a **bc** d", 0, 1), "a bc d");
}

#[test]
fn caret_inside_heading_reveals() {
    assert_eq!(vis("# T\nx", 1), "# T\nx");
}

#[test]
fn caret_inside_quote_reveals() {
    assert_eq!(vis("> a\n> b\n", 3), "> a\n> b");
}

#[test]
fn caret_inside_item_reveals_task_mark() {
    assert_eq!(vis("- [ ] t\nx", 6), "- [ ] t\nx");
}

#[test]
fn caret_inside_link_reveals_syntax() {
    let src = "a [t](u) b";
    let caret = src.find('t').unwrap();
    assert_eq!(vis(src, caret), "a [t](u) b");
}

#[test]
fn caret_inside_image_reveals_syntax() {
    assert_eq!(vis("![p](i.png)", 2), "![p](i.png)");
}

#[test]
fn caret_inside_code_block_reveals_fences() {
    let src = "```rs\nx\n```\n";
    assert_eq!(vis(src, 6), "```rs\nx\n```");
}

// --- tables -----------------------------------------------------------------

#[test]
fn table_outside_cursor_shows_grid() {
    let src = "| A | B |\n|---|---|\n| 1 | 2 |\n";
    assert_eq!(vis(src, 500), "│ A │ B │\n│ 1 │ 2 │");
}

#[test]
fn table_under_cursor_shows_raw() {
    let src = "| A | B |\n|---|---|\n| 1 | 2 |\n\nx";
    let caret = src.find('A').unwrap();
    assert_eq!(vis(src, caret), "| A | B |\n|---|---|\n| 1 | 2 |\nx");
}

#[test]
fn table_spans_have_cell_blocks() {
    let src = "| A | B |\n|---|---|\n| 1 | 2 |\n";
    let doc = parse(src);
    let p = project(&doc, src, Selection::caret(200));
    let a = p
        .spans
        .iter()
        .find(|s| p.text[s.vis.clone()] == *"A")
        .unwrap();
    assert!(matches!(
        a.block,
        memoria_editor_core::project::BlockTag::TableCell { header: true }
    ));
}

// --- source↔visible map ------------------------------------------------------

#[test]
fn map_round_trip_through_hidden_marker() {
    let src = "a **b** c";
    let doc = parse(src);
    let p = project(&doc, src, Selection::caret(0));
    // visible "b" at index 2 -> source index of 'b' (after `**`)
    let sb = src.find('b').unwrap();
    assert_eq!(p.to_source(2), sb);
    assert_eq!(p.to_visible(sb), 2);
    // inside the hidden `**` snaps to the removal point
    assert_eq!(p.to_visible(src.find("**").unwrap() + 1), 2);
}

#[test]
fn map_atomic_checkbox_snaps() {
    let src = "- [ ] t";
    let doc = parse(src);
    let p = project(&doc, src, Selection::caret(99));
    // the ☐ glyph maps back to the task mark
    let vis_box = p.text.find('☐').unwrap();
    let s = p.to_source(vis_box);
    assert!(src[s..].starts_with('[') || src[..s].ends_with('-'));
}

// --- styling -----------------------------------------------------------------

#[test]
fn marks_propagate() {
    let src = "**b** *i* ~~s~~ `c` [l](u)";
    let doc = parse(src);
    let p = project(&doc, src, Selection::caret(999));
    let flags = |word: &str| {
        p.spans
            .iter()
            .find(|s| p.text[s.vis.clone()] == *word)
            .map(|s| s.marks)
            .unwrap()
    };
    assert!(flags("b").has(Marks::BOLD));
    assert!(flags("i").has(Marks::ITALIC));
    assert!(flags("s").has(Marks::STRIKE));
    assert!(flags("c").has(Marks::CODE));
    assert!(flags("l").has(Marks::LINK));
}

#[test]
fn link_payload() {
    let src = "[t](https://x.y \"T\")";
    let doc = parse(src);
    let p = project(&doc, src, Selection::caret(999));
    let s = p
        .spans
        .iter()
        .find(|s| matches!(s.payload, Payload::Link { .. }))
        .unwrap();
    match &s.payload {
        Payload::Link { dest, title } => {
            assert_eq!(&**dest, "https://x.y");
            assert_eq!(&**title, "T");
        }
        _ => unreachable!(),
    }
}

#[test]
fn image_payload() {
    let src = "![alt](i.png)";
    let doc = parse(src);
    let p = project(&doc, src, Selection::caret(999));
    let s = p
        .spans
        .iter()
        .find(|s| matches!(s.payload, Payload::Image { .. }))
        .unwrap();
    match &s.payload {
        Payload::Image { src } => assert_eq!(&**src, "i.png"),
        _ => unreachable!(),
    }
}

#[test]
fn bare_url_autolink() {
    let src = "see https://a.b/c?d=1 now";
    let doc = parse(src);
    let p = project(&doc, src, Selection::caret(0));
    let s = p
        .spans
        .iter()
        .find(|s| matches!(s.payload, Payload::Link { .. }))
        .expect("bare url should linkify");
    assert_eq!(&p.text[s.vis.clone()], "https://a.b/c?d=1");
}

#[test]
fn www_autolink_requires_dotted_domain() {
    // GFM/linkify: `www.` alone is not a link — the domain after it must
    // contain a `.` before any path/query. `www.foo` stays plain text while
    // `www.foo.com` and `www.foo.com/x` link.
    for (src, should_link) in [
        ("see www.foo now", false),
        ("see www.foo/bar now", false),
        ("see www.foo?x.y now", false),
        ("see www.foo.com now", true),
        ("see www.foo.com/x now", true),
    ] {
        let doc = parse(src);
        let p = project(&doc, src, Selection::caret(0));
        let linked = p
            .spans
            .iter()
            .any(|s| matches!(s.payload, Payload::Link { .. }));
        assert_eq!(linked, should_link, "{src}");
    }
}
