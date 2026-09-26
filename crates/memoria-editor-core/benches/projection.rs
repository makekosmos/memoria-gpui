//! Criterion benches: open 10k lines, type one char, reproject.

use criterion::{criterion_group, criterion_main, Criterion};
use memoria_editor_core::{md::parse, project::project, Buffer, Editor, Selection};
use std::hint::black_box;

fn big_doc() -> String {
    let mut s = String::new();
    for i in 0..10_000 {
        s.push_str(&format!(
            "## Heading {i}\n\nParagraph **bold {i}** and `code` with [link](https://e.x/{i}).\n\n- [ ] task {i}\n- item {i}\n\n"
        ));
    }
    s
}

fn open_10k(c: &mut Criterion) {
    let src = big_doc();
    c.bench_function("open_10k_lines", |b| {
        b.iter(|| {
            let buf = Buffer::from_text(black_box(&src));
            black_box(buf.len_bytes())
        })
    });
    c.bench_function("parse_10k_lines", |b| {
        b.iter(|| black_box(parse(black_box(&src))))
    });
}

fn type_char(c: &mut Criterion) {
    let src = big_doc();
    c.bench_function("type_one_char", |b| {
        let mut ed = Editor::new(&src);
        ed.set_selection(Selection::caret(50_000));
        b.iter(|| {
            ed.insert_text(black_box("x"));
        })
    });
}

fn reproject(c: &mut Criterion) {
    let src = big_doc();
    c.bench_function("reproject_10k_lines", |b| {
        b.iter(|| {
            let doc = parse(black_box(&src));
            black_box(project(&doc, &src, Selection::caret(0)))
        })
    });
    // Realistic single-line doc reprojection (the hot path per keystroke).
    let small = "# T\n\nPara **b** `c` [l](u).\n";
    c.bench_function("reproject_small", |b| {
        b.iter(|| {
            let doc = parse(black_box(small));
            black_box(project(&doc, small, Selection::caret(5)))
        })
    });
}

criterion_group!(benches, open_10k, type_char, reproject);
criterion_main!(benches);
