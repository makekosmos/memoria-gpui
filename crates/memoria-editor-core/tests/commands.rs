//! Unit tests per command (TipTap StarterKit + TaskList parity).

use memoria_editor_core::cmd::{self, Command, ListKind};
use memoria_editor_core::{Editor, Selection};

fn ed(src: &str, a: usize, b: usize) -> Editor {
    let mut e = Editor::new(src);
    e.set_selection(Selection::new(a, b));
    e
}

fn run(src: &str, a: usize, b: usize, cmd: &Command) -> String {
    let mut e = ed(src, a, b);
    e.command(cmd);
    e.text()
}

// --- inline marks -----------------------------------------------------------

#[test]
fn bold_wraps_selection() {
    assert_eq!(run("abc", 0, 3, &Command::Bold), "**abc**");
}

#[test]
fn bold_caret_inserts_empty_pair() {
    // TipTap stores a pending mark; a source editor inserts an empty pair and
    // lands the caret inside, so typing produces `**x**`.
    let mut e = ed("a b", 2, 2);
    e.command(&Command::Bold);
    assert_eq!(e.text(), "a ****b");
    let v = e.selection().start();
    assert_eq!(&e.text()[..v], "a **");
}

#[test]
fn bold_unwraps_when_fully_marked() {
    assert_eq!(run("**abc**", 2, 5, &Command::Bold), "abc");
    // selection covering the delimiters also strips
    assert_eq!(run("**abc**", 0, 7, &Command::Bold), "abc");
}

#[test]
fn italic_wraps_and_unwraps() {
    assert_eq!(run("abc", 0, 3, &Command::Italic), "*abc*");
    assert_eq!(run("*abc*", 1, 4, &Command::Italic), "abc");
}

#[test]
fn italic_does_not_strip_strong() {
    // `**abc**` is a strong run; italic toggle must wrap, not strip one `*`.
    assert_eq!(run("**abc**", 2, 5, &Command::Italic), "***abc***");
    // Same when the selection covers the `**` delimiters (select-all): the
    // strong marks must survive — result is bold+italic, not `*abc*`.
    assert_eq!(run("**abc**", 0, 7, &Command::Italic), "***abc***");
    assert_eq!(run("__abc__", 0, 7, &Command::Italic), "*__abc__*");
    // `***abc***` carries an italic layer — toggling removes exactly that
    // layer, keeping the strong pair.
    assert_eq!(run("***abc***", 0, 9, &Command::Italic), "**abc**");
    // Plain `*abc*` still unwraps.
    assert_eq!(run("*abc*", 0, 5, &Command::Italic), "abc");
}

#[test]
fn strike_toggle() {
    assert_eq!(run("abc", 0, 3, &Command::Strike), "~~abc~~");
    assert_eq!(run("~~abc~~", 2, 5, &Command::Strike), "abc");
}

#[test]
fn inline_code_toggle() {
    assert_eq!(run("abc", 0, 3, &Command::InlineCode), "`abc`");
    assert_eq!(run("`abc`", 1, 4, &Command::InlineCode), "abc");
}

#[test]
fn inline_code_longer_fence_for_backticks() {
    // Content containing a backtick needs a longer fence.
    assert_eq!(run("a`b", 0, 3, &Command::InlineCode), "``a`b``");
}

#[test]
fn bold_inside_run_splits() {
    // selecting inside `**abcd**` wraps the subrange — splitting the run.
    assert_eq!(run("**abcd**", 3, 5, &Command::Bold), "**a**bc**d**");
}

// --- headings ----------------------------------------------------------------

#[test]
fn heading_set_and_toggle_off() {
    assert_eq!(run("text", 0, 0, &Command::Heading(1)), "# text");
    assert_eq!(run("# text", 2, 2, &Command::Heading(1)), "text");
    // switching level
    assert_eq!(run("# text", 2, 2, &Command::Heading(3)), "### text");
}

#[test]
fn heading_multiline_selection() {
    assert_eq!(run("a\nb", 0, 3, &Command::Heading(2)), "## a\n## b");
}

// --- lists -------------------------------------------------------------------

#[test]
fn bullet_list_toggle() {
    assert_eq!(
        run("a\nb", 0, 3, &Command::List(ListKind::Bullet)),
        "- a\n- b"
    );
    assert_eq!(
        run("- a\n- b", 0, 6, &Command::List(ListKind::Bullet)),
        "a\nb"
    );
}

#[test]
fn ordered_list_numbers() {
    assert_eq!(
        run("a\nb\nc", 0, 5, &Command::List(ListKind::Ordered)),
        "1. a\n2. b\n3. c"
    );
}

#[test]
fn task_list_toggle() {
    assert_eq!(run("a", 0, 1, &Command::List(ListKind::Task)), "- [ ] a");
    assert_eq!(run("- [ ] a", 0, 6, &Command::List(ListKind::Task)), "a");
}

#[test]
fn task_to_bullet_keeps_marker_shape() {
    assert_eq!(
        run("- [ ] a", 0, 6, &Command::List(ListKind::Bullet)),
        "- a"
    );
}

#[test]
fn checkbox_toggle() {
    assert_eq!(run("- [ ] a", 4, 4, &Command::TaskToggle), "- [x] a");
    assert_eq!(run("- [x] a", 4, 4, &Command::TaskToggle), "- [ ] a");
    // outside a task line: no-op
    assert_eq!(run("plain", 2, 2, &Command::TaskToggle), "plain");
}

// --- quote / code / rule -----------------------------------------------------

#[test]
fn quote_wrap_unwrap() {
    assert_eq!(run("a\nb", 0, 3, &Command::Quote), "> a\n> b");
    assert_eq!(run("> a\n> b", 0, 6, &Command::Quote), "a\nb");
}

#[test]
fn quote_empty_lines() {
    assert_eq!(run("a\n\nb", 0, 4, &Command::Quote), "> a\n>\n> b");
}

#[test]
fn code_block_wrap() {
    assert_eq!(
        run("let x = 1;", 0, 10, &Command::CodeBlock("rust".into())),
        "```rust\nlet x = 1;\n```"
    );
}

#[test]
fn code_block_unwrap() {
    let src = "```rust\nlet x = 1;\n```\n";
    let mut e = ed(src, 10, 10);
    e.command(&Command::CodeBlock("rust".into()));
    assert_eq!(e.text(), "let x = 1;\n");
}

#[test]
fn rule_insert_midline_splits() {
    // `ab|cd` — rule goes on its own line, never a setext heading
    assert_eq!(run("abcd", 2, 2, &Command::Rule), "ab\n\n---\n\ncd");
}

#[test]
fn rule_insert_empty_line() {
    assert_eq!(run("a\n\nb", 2, 2, &Command::Rule), "a\n---\nb");
}

// --- link / image ------------------------------------------------------------

#[test]
fn link_wraps_selection() {
    assert_eq!(
        run(
            "click",
            0,
            5,
            &Command::Link {
                dest: "https://x".into(),
                title: None
            }
        ),
        "[click](https://x)"
    );
}

#[test]
fn link_with_title() {
    assert_eq!(
        run(
            "t",
            0,
            1,
            &Command::Link {
                dest: "u".into(),
                title: Some("T".into())
            }
        ),
        "[t](u \"T\")"
    );
}

#[test]
fn image_insert() {
    assert_eq!(
        run(
            "",
            0,
            0,
            &Command::Image {
                src: "p.png".into(),
                alt: "a".into(),
                title: None
            }
        ),
        "![a](p.png)"
    );
}

#[test]
fn local_image_insertion_uses_file_path() {
    assert_eq!(
        run(
            "",
            0,
            0,
            &Command::LocalImage {
                path: "/tmp/p.png".into(),
                alt: "p".into()
            }
        ),
        "![p](/tmp/p.png)"
    );
    // display conversion mirrors localImages.ts
    assert_eq!(
        cmd::display_image_src("/tmp/p.png"),
        "/tmp/p.png" // non-file src passes through
    );
    assert_eq!(
        cmd::display_image_src("file:///tmp/p.png"),
        "kosmos-local-image://file/%2Ftmp%2Fp.png"
    );
    assert_eq!(
        cmd::display_image_src("C:\\img\\p.png"),
        "kosmos-local-image://file/C%3A%5Cimg%5Cp.png"
    );
}

#[test]
fn local_image_url_matches_vue_encoding() {
    assert_eq!(
        cmd::local_image_url("/a b/c.png"),
        "kosmos-local-image://file/%2Fa%20b%2Fc.png"
    );
}

#[test]
fn display_image_src_non_ascii_after_percent_does_not_panic() {
    // `%` followed by a multi-byte UTF-8 char must not slice mid-char.
    assert_eq!(
        cmd::display_image_src("file:///%€x"),
        "kosmos-local-image://file/%2F%25%E2%82%ACx"
    );
    // And a real `%HH` escape still decodes.
    assert_eq!(
        cmd::display_image_src("file:///%E2%82%AC/a.png"),
        "kosmos-local-image://file/%2F%E2%82%AC%2Fa.png"
    );
}
