//! Unicode, CRLF, IME contract, and charCount parity with Vue
//! `src/lib/charCount.ts`.

use memoria_editor_core::{count_chars, Editor, Selection};

// --- unicode buffers ---------------------------------------------------------

#[test]
fn grapheme_deletion_russian_emoji_combining() {
    // combining marks: `щ` + `̈`
    let mut e = Editor::new("щ̈x");
    e.set_selection(Selection::caret("щ̈".len()));
    e.key_backspace();
    assert_eq!(e.text(), "x");

    // emoji ZWJ sequence is one cluster
    let mut e = Editor::new("👨‍👩‍👧x");
    e.set_selection(Selection::caret("👨‍👩‍👧".len()));
    e.key_backspace();
    assert_eq!(e.text(), "x");

    // flag (two regional indicators = one cluster)
    let mut e = Editor::new("🇷🇺x");
    e.set_selection(Selection::caret("🇷🇺".len()));
    e.key_backspace();
    assert_eq!(e.text(), "x");
}

#[test]
fn editing_multibyte_keeps_boundaries() {
    let mut e = Editor::new("за");
    e.set_selection(Selection::caret(2)); // inside 'а'? byte2 = end of з
    e.insert_text("!");
    assert_eq!(e.text(), "з!а");
}

#[test]
fn crlf_lines_and_parse() {
    let src = "# T\r\n\r\n- a\r\n- b\r\n";
    let mut e = Editor::new(src);
    // parse works on CRLF
    let p = e.project();
    assert!(p.text.contains('T'));
    assert!(p.text.contains('•'));
    // enter inside a CRLF line
    e.set_selection(Selection::caret(2));
    e.key_enter();
    assert_eq!(e.text(), "# \n# T\r\n\r\n- a\r\n- b\r\n");
}

#[test]
fn crlf_serialize_identity() {
    let src = "a\r\nb\r\n\r\n- c\r\n";
    let e = Editor::new(src);
    assert_eq!(e.serialize(), src);
}

// --- IME contract ------------------------------------------------------------

#[test]
fn ime_set_marked_unmark() {
    let mut e = Editor::new("ab");
    e.set_selection(Selection::caret(1));
    e.set_marked_text("に");
    assert_eq!(e.text(), "aにb");
    let r = e.marked_source_range().unwrap();
    assert_eq!(&e.text()[r], "に");
    // composition continues
    e.replace_and_mark_text_in_range(None, "にち", Some(0.."にち".encode_utf16().count()));
    assert_eq!(e.text(), "aにちb");
    e.unmark_text();
    assert!(e.marked_source_range().is_none());
}

#[test]
fn ime_utf16_ranges() {
    // emoji = 2 UTF-16 units
    let mut e = Editor::new("a😀b");
    assert_eq!(e.selected_text_range_utf16(), 4..4); // caret at end: 1+2+1
    let (t, r) = e.text_for_range(1..3).unwrap();
    assert_eq!(t, "😀");
    assert_eq!(r, 1..3);
    e.set_selection(Selection::caret(1));
    e.replace_text_in_range(Some(0..1), "X");
    assert_eq!(e.text(), "X😀b");
}

#[test]
fn ime_marked_range_utf16() {
    let mut e = Editor::new("");
    e.set_marked_text("かな😀");
    // 3 kana + 2 units emoji = 5 utf16 units
    assert_eq!(e.marked_text_range_utf16(), Some(0..4));
    e.unmark_text();
    assert_eq!(e.marked_text_range_utf16(), None);
}

// --- charCount parity --------------------------------------------------------
// Vue: code points in text nodes; +1 per leaf block after the first;
// +1 per hard break; containers add nothing.

fn count(src: &str) -> usize {
    let mut e = Editor::new(src);
    let doc = e.doc().clone();
    count_chars(&doc, src)
}

#[test]
fn charcount_plain() {
    assert_eq!(count("abc"), 3);
    assert_eq!(count(""), 0);
}

#[test]
fn charcount_codepoints_not_utf16() {
    assert_eq!(count("😀"), 1); // surrogate pair = 1
    assert_eq!(count("日本語"), 3);
}

#[test]
fn charcount_block_separator() {
    // two paragraphs: 3 + 1 + 3
    assert_eq!(count("abc\n\ndef"), 7);
    // heading + para
    assert_eq!(count("# h\n\nabc"), 1 + 1 + 3);
}

#[test]
fn charcount_hard_break() {
    // Vue reads the in-paragraph newline as a hardBreak: "a\nb" = 3
    assert_eq!(count("a\nb"), 3);
    // explicit hard break `a\` newline `b` same count
    assert_eq!(count("a\\\nb"), 3);
}

#[test]
fn charcount_list_containers_free() {
    // `- a` + `- b`: leaf items 1 + 1 + 1 = 3 (marker, list itself free)
    assert_eq!(count("- a\n- b"), 3);
    assert_eq!(count("> a\n> b"), 3);
    // nested: items are the leaf paras
    assert_eq!(count("- a\n  - b"), 3);
}

#[test]
fn charcount_code_block() {
    // leaf + text incl. interior newline
    assert_eq!(count("```\nx\ny\n```"), 1 + 3);
}

#[test]
fn charcount_images_split_paragraphs() {
    // `a ![x](u) b`: para "a " (2) + image (0) + para " b" (2) + separator (1)
    assert_eq!(count("a ![x](u) b"), 5);
    // image-only paragraph contributes nothing
    assert_eq!(count("![x](u)"), 0);
    assert_eq!(count("t\n\n![x](u)\n\ns"), 3); // 1 + sep + 1
}

#[test]
fn charcount_marks_dont_count() {
    assert_eq!(count("**abc**"), 3);
    assert_eq!(count("`abc`"), 3);
    assert_eq!(count("[abc](u)"), 3);
    assert_eq!(count("# abc"), 3);
    assert_eq!(count("- [ ] abc"), 3); // checkbox marker free
}

#[test]
fn charcount_table_counts_as_raw_lines() {
    // Vue's markdown reader keeps table lines as paragraph text.
    let src = "| A |\n|---|\n| 1 |";
    let expected = src.chars().count(); // text + 2 hard breaks
    assert_eq!(count(src), expected);
}

// --- undo / redo ---------------------------------------------------------------

#[test]
fn undo_restores_source_and_selection() {
    let mut e = Editor::new("ab");
    e.set_selection(Selection::caret(1));
    e.insert_text("X");
    assert_eq!(e.text(), "aXb");
    assert!(e.undo());
    assert_eq!(e.text(), "ab");
    assert_eq!(e.selection().start(), 1);
    assert!(e.redo());
    assert_eq!(e.text(), "aXb");
    assert_eq!(e.selection().start(), 2);
}

#[test]
fn undo_groups_by_pause_and_kind() {
    use std::time::Duration;
    let mut e = Editor::new("");
    e.manual_clock();
    e.insert_text("a");
    e.advance_time(Duration::from_millis(100));
    e.insert_text("b");
    // two inserts in <500ms -> one group
    assert!(e.undo());
    assert_eq!(e.text(), "");
    assert!(!e.can_undo());

    // pause splits groups
    e.manual_clock();
    e.insert_text("a");
    e.advance_time(Duration::from_secs(1));
    e.insert_text("b");
    e.undo();
    assert_eq!(e.text(), "a");
    e.undo();
    assert_eq!(e.text(), "");
}

#[test]
fn undo_separates_insert_and_delete() {
    let mut e = Editor::new("x");
    e.manual_clock();
    e.insert_text("a");
    e.key_backspace();
    e.undo();
    assert_eq!(e.text(), "xa");
    e.undo();
    assert_eq!(e.text(), "x");
}

#[test]
fn commands_never_merge_into_typing() {
    let mut e = Editor::new("x");
    e.manual_clock();
    e.insert_text("a");
    e.command(&memoria_editor_core::cmd::Command::Quote);
    e.insert_text("b");
    assert_eq!(e.text(), "> bxa");
    e.undo(); // removes b
    assert_eq!(e.text(), "> xa");
    e.undo(); // removes quote marker
    assert_eq!(e.text(), "xa");
    e.undo(); // removes a
    assert_eq!(e.text(), "x");
}
