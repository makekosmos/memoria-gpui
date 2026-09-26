//! Key behavior tests: Enter / Backspace / Tab / Shift+Tab (TipTap parity).

use memoria_editor_core::{Editor, Selection};

fn ed(src: &str, caret: usize) -> Editor {
    let mut e = Editor::new(src);
    e.set_selection(Selection::caret(caret));
    e
}

// --- Enter -------------------------------------------------------------------

#[test]
fn enter_in_paragraph() {
    let mut e = ed("ab", 1);
    e.key_enter();
    assert_eq!(e.text(), "a\nb");
}

#[test]
fn enter_continues_bullet() {
    let mut e = ed("- a", 3);
    e.key_enter();
    assert_eq!(e.text(), "- a\n- ");
    assert_eq!(e.selection().start(), 6);
}

#[test]
fn enter_continues_ordered_incrementing() {
    let mut e = ed("1. a", 4);
    e.key_enter();
    assert_eq!(e.text(), "1. a\n2. ");
}

#[test]
fn enter_continues_task() {
    let mut e = ed("- [ ] a", 7);
    e.key_enter();
    assert_eq!(e.text(), "- [ ] a\n- [ ] ");
}

#[test]
fn enter_empty_item_exits_list() {
    let mut e = ed("- a\n- ", 6);
    e.key_enter();
    assert_eq!(e.text(), "- a\n");
}

#[test]
fn enter_splits_list_item() {
    let mut e = ed("- ab", 3);
    e.key_enter();
    assert_eq!(e.text(), "- a\n- b");
}

#[test]
fn enter_nested_list_keeps_indent() {
    let mut e = ed("- a\n  - b", 9);
    e.key_enter();
    assert_eq!(e.text(), "- a\n  - b\n  - ");
}

#[test]
fn enter_continues_quote() {
    let mut e = ed("> a", 3);
    e.key_enter();
    assert_eq!(e.text(), "> a\n> ");
}

#[test]
fn enter_in_quote_midtext() {
    let mut e = ed("> ab", 3);
    e.key_enter();
    assert_eq!(e.text(), "> a\n> b");
}

#[test]
fn enter_splits_heading_keeping_level() {
    let mut e = ed("## ab", 4);
    e.key_enter();
    assert_eq!(e.text(), "## a\n## b");
}

#[test]
fn enter_at_heading_end_makes_paragraph() {
    let mut e = ed("## ab", 5);
    e.key_enter();
    assert_eq!(e.text(), "## ab\n");
}

#[test]
fn enter_in_code_block_indents() {
    let src = "```rust\nfn main() {\n}\n```\n";
    let caret = src.find("{\n").unwrap() + 1;
    let mut e = ed(src, caret);
    e.key_enter();
    assert_eq!(e.text(), "```rust\nfn main() {\n  \n}\n```\n");
}

#[test]
fn enter_on_fence_line_is_normal() {
    let src = "```rust\nx\n```\n";
    let mut e = ed(src, 4);
    e.key_enter();
    // splits the fence line like a normal paragraph
    assert_eq!(e.text(), "```r\nust\nx\n```\n");
}

#[test]
fn shift_enter_hard_break() {
    let mut e = ed("ab", 1);
    e.key_shift_enter();
    assert_eq!(e.text(), "a\\\nb");
}

// --- Backspace ---------------------------------------------------------------

#[test]
fn backspace_deletes_grapheme() {
    let mut e = ed("ab", 2);
    e.key_backspace();
    assert_eq!(e.text(), "a");
}

#[test]
fn backspace_deletes_emoji_cluster() {
    let mut e = ed("a👨‍👩‍👧", "a👨‍👩‍👧".len());
    e.key_backspace();
    assert_eq!(e.text(), "a");
}

#[test]
fn backspace_removes_list_marker() {
    let mut e = ed("- item", 2);
    e.key_backspace();
    assert_eq!(e.text(), "item");
}

#[test]
fn backspace_removes_task_marker() {
    let mut e = ed("- [ ] item", 6);
    e.key_backspace();
    assert_eq!(e.text(), "item");
}

#[test]
fn backspace_removes_heading_marker() {
    let mut e = ed("## t", 3);
    e.key_backspace();
    assert_eq!(e.text(), "t");
}

#[test]
fn backspace_lifts_innermost_marker() {
    let mut e = ed("> - item", 4);
    e.key_backspace();
    assert_eq!(e.text(), "> item");
}

#[test]
fn backspace_at_line_start_joins_lines() {
    let mut e = ed("a\nb", 2);
    e.key_backspace();
    assert_eq!(e.text(), "ab");
}

#[test]
fn backspace_at_doc_start_noop() {
    let mut e = ed("a", 0);
    e.key_backspace();
    assert_eq!(e.text(), "a");
}

#[test]
fn backspace_deletes_selection() {
    let mut e = ed("abc", 0);
    e.set_selection(Selection::new(0, 2));
    e.key_backspace();
    assert_eq!(e.text(), "c");
}

// --- Tab ---------------------------------------------------------------------

#[test]
fn tab_indents_list_item() {
    let mut e = ed("- a\n- b", 4);
    e.key_tab();
    assert_eq!(e.text(), "- a\n  - b");
}

#[test]
fn shift_tab_dedents_list_item() {
    let mut e = ed("- a\n  - b", 6);
    e.key_shift_tab();
    assert_eq!(e.text(), "- a\n- b");
}

#[test]
fn tab_in_code_inserts_spaces() {
    let src = "```\nlet x\n```\n";
    let mut e = ed(src, 7);
    e.key_tab();
    assert_eq!(e.text(), "```\nlet   x\n```\n");
}

#[test]
fn shift_tab_in_code_dedents() {
    let src = "```\n  let x\n```\n";
    let mut e = ed(src, 8);
    e.key_shift_tab();
    assert_eq!(e.text(), "```\nlet x\n```\n");
}

#[test]
fn tab_in_paragraph_inserts_tab() {
    let mut e = ed("ab", 1);
    e.key_tab();
    assert_eq!(e.text(), "a\tb");
}
