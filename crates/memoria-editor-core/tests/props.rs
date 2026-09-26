//! Property tests: random edit sequences keep the buffer valid, the cursor
//! stays in range, projections don't panic, and full undo restores source.

use memoria_editor_core::{md::parse, project::project, Editor, Selection};
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    Insert(usize, String),
    Delete(usize, usize),
    Enter(usize),
    Backspace(usize),
    Select(usize, usize),
    Command(memoria_editor_core::cmd::Command),
}

fn arb_op() -> impl Strategy<Value = Op> {
    let text = prop::sample::select(vec![
        "x".to_string(),
        "**".into(),
        "- ".into(),
        "\n".into(),
        "ё".into(),
        "👨‍👩‍👧".into(),
        "```rust\n".into(),
        "> ".into(),
        "https://a.b".into(),
        "|".into(),
    ]);
    let cmds = prop::sample::select(vec![
        memoria_editor_core::cmd::Command::Bold,
        memoria_editor_core::cmd::Command::Italic,
        memoria_editor_core::cmd::Command::Heading(2),
        memoria_editor_core::cmd::Command::List(memoria_editor_core::cmd::ListKind::Bullet),
        memoria_editor_core::cmd::Command::List(memoria_editor_core::cmd::ListKind::Task),
        memoria_editor_core::cmd::Command::Quote,
        memoria_editor_core::cmd::Command::Rule,
        memoria_editor_core::cmd::Command::CodeBlock("rs".into()),
        memoria_editor_core::cmd::Command::TaskToggle,
    ]);
    (
        prop::collection::vec(text, 0..=2),
        any::<usize>(),
        any::<usize>(),
        prop::sample::select(vec![0u8, 1, 2, 3, 4, 5]),
        cmds,
    )
        .prop_map(|(texts, a, b, pick, cmd)| {
            let t = texts.concat();
            match pick {
                0 => Op::Insert(a, t),
                1 => Op::Delete(a, b),
                2 => Op::Enter(a),
                3 => Op::Backspace(a),
                5 => Op::Command(cmd),
                _ => Op::Select(a, b),
            }
        })
}

const SEEDS: &[&str] = &[
    "# T\n\npara **b** `c` [l](u).\n\n- [ ] a\n- i\n\n> q\n\n```rs\nx\n```\n",
    "plain",
    "",
    "а б 👋🏽\r\nв\r\n",
    "| A |\n|---|\n| 1 |\n",
    "![i](p.png) and https://x.y\n",
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn random_edits_preserve_invariants(
        seed_idx in 0..SEEDS.len(),
        ops in prop::collection::vec(arb_op(), 0..40),
    ) {
        let src = SEEDS[seed_idx];
        let original = src.to_string();
        let mut e = Editor::new(src);
        e.manual_clock();
        for op in &ops {
            let len = e.len();
            match op {
                Op::Insert(p, t) => {
                    let p = p % (len + 1);
                    let mut tx = memoria_editor_core::Tx::new();
                    tx.insert(p, t.clone());
                    tx.selection(Selection::caret((p + t.len()).min(len + t.len())));
                    e.apply(tx, memoria_editor_core::EditKind::Insert);
                }
                Op::Delete(a, b) => {
                    if len == 0 { continue; }
                    let (a, b) = (a % (len + 1), b % (len + 1));
                    let (a, b) = (a.min(b), a.max(b));
                    let mut tx = memoria_editor_core::Tx::new();
                    tx.replace(a, b, "");
                    tx.selection(Selection::caret(a));
                    e.apply(tx, memoria_editor_core::EditKind::Delete);
                }
                Op::Enter(p) => {
                    e.set_selection(Selection::caret(p % (len + 1)));
                    e.key_enter();
                }
                Op::Backspace(p) => {
                    e.set_selection(Selection::caret(p % (len + 1)));
                    e.key_backspace();
                }
                Op::Select(a, b) => {
                    e.set_selection(Selection::new(a % (len + 1), b % (len + 1)));
                }
                Op::Command(c) => {
                    e.command(c);
                }
            }
            // invariants after every op
            let t = e.text();
            prop_assert!(e.selection().start() <= t.len());
            prop_assert!(e.selection().end() <= t.len());
            prop_assert!(std::str::from_utf8(t.as_bytes()).is_ok());
            let doc = parse(&t);
            let p = project(&doc, &t, e.selection());
            for v in 0..=p.text.len() {
                prop_assert!(p.to_source(v) <= t.len());
            }
            for s in 0..=t.len() {
                prop_assert!(p.to_visible(s) <= p.text.len());
            }
        }
        // full undo restores the original source
        while e.undo() {}
        prop_assert_eq!(e.text(), original);
    }

    #[test]
    fn projection_never_panics(src in ".*", caret in any::<usize>()) {
        let doc = parse(&src);
        let p = project(&doc, &src, Selection::caret(caret % (src.len() + 1)));
        for v in 0..=p.text.len() {
            prop_assert!(p.to_source(v) <= src.len());
        }
    }
}
