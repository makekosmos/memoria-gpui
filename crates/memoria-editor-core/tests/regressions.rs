//! Regression tests for bugs found in the KOS-190 / KOS-219 bug hunts.

use memoria_editor_core::cmd::{self, Command};
use memoria_editor_core::{Editor, Selection};

fn run(src: &str, a: usize, b: usize, cmd: &Command) -> String {
    let mut e = Editor::new(src);
    e.set_selection(Selection::new(a, b));
    e.command(cmd);
    e.text()
}

#[test]
fn rule_at_line_start_does_not_setext_the_line_above() {
    // Caret at the start of "def" — `abc\n---` parses as a setext H2, not a
    // rule: the marker must land behind a blank line.
    assert_eq!(run("abc\ndef", 4, 4, &Command::Rule), "abc\n\n---\n\ndef");
}

#[test]
fn display_image_src_authority_keeps_path_absolute() {
    // `file://host/path` → `new URL().pathname` drops the authority and keeps
    // the absolute path. Stripping `//` alone yields `host/path` — relative.
    assert_eq!(
        cmd::display_image_src("file://host/tmp/p.png"),
        "kosmos-local-image://file/%2Ftmp%2Fp.png"
    );
    // `file:relative` normalizes to `/relative` under URL semantics.
    assert_eq!(
        cmd::display_image_src("file:relative/p.png"),
        "kosmos-local-image://file/%2Frelative%2Fp.png"
    );
}

#[test]
fn link_caret_lands_after_closing_paren() {
    // KOS-219: caret used to stop two bytes early — inside the destination —
    // so typing right after "wrap in link" corrupted the URL.
    let mut e = Editor::new("click");
    e.set_selection(Selection::new(0, 5));
    e.command(&Command::Link {
        dest: "https://x".into(),
        title: None,
    });
    assert_eq!(e.selection().start(), "[click](https://x)".len());
    e.insert_text("TAIL");
    assert_eq!(e.text(), "[click](https://x)TAIL");

    let mut e = Editor::new("t");
    e.set_selection(Selection::new(0, 1));
    e.command(&Command::Link {
        dest: "u".into(),
        title: Some("T".into()),
    });
    assert_eq!(e.selection().start(), "[t](u \"T\")".len());
}
