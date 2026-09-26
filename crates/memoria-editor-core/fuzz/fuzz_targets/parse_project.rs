#![no_main]

use libfuzzer_sys::fuzz_target;
use memoria_editor_core::cursor::Selection;
use memoria_editor_core::md::parse;
use memoria_editor_core::project::project;

// Fuzz parser + projection + source↔visible map over arbitrary bytes.
// Run: `cargo +nightly fuzz run parse_project -- -max_total_time=600`
fuzz_target!(|data: &[u8]| {
    let src = String::from_utf8_lossy(data);
    let doc = parse(&src);
    for sel in [
        Selection::caret(0),
        Selection::caret(src.len() / 2),
        Selection::caret(src.len()),
        Selection::new(0, src.len()),
    ] {
        let p = project(&doc, &src, sel);
        for v in 0..=p.text.len() {
            assert!(p.to_source(v) <= src.len());
        }
        for s in 0..=src.len() {
            assert!(p.to_visible(s) <= p.text.len());
        }
    }
});
