use pulldown_cmark::{Options, Parser};

fn main() {
    let samples = [
        "# Head **bold** tail\n",
        "para **bold _it_** and `code` ~~gone~~ [l](https://x \"t\") ![alt](img.png)\n",
        "- [ ] todo\n- [x] done\n- plain\n  - nested\n",
        "1. one\n2. two\n",
        "> quote\n> line2\n\nnext\n",
        "```rust\nfn main() {}\n```\n",
        "auto https://example.com/x and <a@b.c> end\n",
        "| a | b |\n|---|---|\n| 1 | 2 |\n",
        "esc \\* ent &amp; end\n",
        "line  
break and\\\nhard\n",
        "***both*** ~~*both2*~~\n",
        "text\r\ncrlf para\r\n\r\nsecond\r\n",
        "- **bold item** tail\n",
        "term [ref][1] here\n\n[1]: https://r\n",
    ];
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_GFM);
    for s in samples {
        println!("=== {:?}", s);
        for (ev, range) in Parser::new_ext(s, opts).into_offset_iter() {
            println!(
                "{:>3}..{:<3} {:?} | {:?}",
                range.start,
                range.end,
                ev,
                &s[range.clone()]
            );
        }
    }
}
