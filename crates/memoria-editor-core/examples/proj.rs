//! Debug: project a fixture at a caret, print visible text + spans.
//! Usage: proj <file> [caret]

use memoria_editor_core::project::{Marks, Payload};
use memoria_editor_core::{Editor, Selection};

fn main() {
    let path = std::env::args().nth(1).expect("usage: proj <file> [caret]");
    let caret: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let src = std::fs::read_to_string(path).unwrap();
    let mut ed = Editor::new(&src);
    ed.set_selection(Selection::caret(caret));
    let p = ed.project();
    println!("=== source ({}B) ===\n{src}", src.len());
    println!("=== visible ===\n{}", p.text);
    println!("=== spans ===");
    for s in &p.spans {
        let mut m = Vec::new();
        for (f, n) in [
            (Marks::BOLD, "b"),
            (Marks::ITALIC, "i"),
            (Marks::STRIKE, "s"),
            (Marks::CODE, "c"),
            (Marks::LINK, "l"),
            (Marks::MARKER, "m"),
            (Marks::WIDGET, "w"),
        ] {
            if s.marks.has(f) {
                m.push(n);
            }
        }
        let pay = match &s.payload {
            Payload::Link { dest, .. } => format!(" link->{dest}"),
            Payload::Image { src } => format!(" img:{src}"),
            Payload::TaskBox { checked } => format!(" box:{checked}"),
            Payload::Marker => " marker".into(),
            Payload::Rule => " rule".into(),
            _ => String::new(),
        };
        let vis_txt: String = p.text[s.vis.clone()].chars().take(24).collect();
        println!(
            "  v{:>3?} s{:>3?} {:?} {:8} {}{}",
            s.vis,
            s.src,
            m,
            format!("{:?}", s.block),
            vis_txt.escape_debug(),
            pay
        );
    }
    // round-trip check: every visible byte maps back into source
    for v in 0..=p.text.len() {
        let s = p.to_source(v);
        assert!(s <= src.len(), "vis {v} -> src {s} out of range");
    }
    for s in 0..=src.len() {
        let v = p.to_visible(s);
        assert!(v <= p.text.len(), "src {s} -> vis {v} out of range");
    }
    println!("map check ok");
}
