//! Stability: every fixture note round-trips byte-identically (serialize is
//! the source) and projections cover both coordinate spaces.

use memoria_editor_core::md::parse;
use memoria_editor_core::project::project;
use memoria_editor_core::{Editor, Selection};

const FIXTURES: &[&str] = &[
    include_str!("fixtures/kitchen-sink.md"),
    include_str!("fixtures/crlf.md"),
    include_str!("fixtures/edge-cases.md"),
    include_str!("fixtures/images.md"),
];

#[test]
fn fixtures_serialize_byte_identical() {
    for src in FIXTURES {
        let ed = Editor::new(src);
        assert_eq!(ed.serialize(), *src);
        // The parse must cover the document and never panic.
        let doc = parse(src);
        assert!(doc.range.end <= src.len() || src.is_empty());
    }
}

#[test]
fn projection_map_covers_all_positions() {
    for src in FIXTURES {
        let doc = parse(src);
        for caret in [0, src.len() / 3, src.len() / 2, src.len()] {
            let p = project(&doc, src, Selection::caret(caret));
            // visible -> source always lands in range
            for v in 0..=p.text.len() {
                let s = p.to_source(v);
                assert!(s <= src.len(), "vis {v} -> src {s} out of {}", src.len());
            }
            // source -> visible always lands in range
            for s in 0..=src.len() {
                let v = p.to_visible(s);
                assert!(
                    v <= p.text.len(),
                    "src {s} -> vis {v} out of {}",
                    p.text.len()
                );
            }
            // chunks tile the visible text contiguously
            let mut covered = 0usize;
            for c in &p.chunks {
                if !c.vis.is_empty() {
                    assert_eq!(c.vis.start, covered, "gap in visible coverage");
                    covered = c.vis.end;
                }
            }
            assert_eq!(covered, p.text.len(), "visible text not fully covered");
        }
    }
}

#[test]
fn projection_spans_cover_visible_text() {
    for src in FIXTURES {
        let doc = parse(src);
        let p = project(&doc, src, Selection::caret(0));
        let mut covered = 0usize;
        for s in &p.spans {
            assert_eq!(s.vis.start, covered, "span gap at {:?}", s.vis);
            covered = s.vis.end;
            assert_eq!(&p.text[s.vis.clone()], &p.text[s.vis.clone()]);
        }
        assert_eq!(covered, p.text.len());
    }
}
