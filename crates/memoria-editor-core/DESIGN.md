# memoria-editor-core — design (KOS-149, M2)

Pure-Rust markdown editor core for Memoria GPUI. **Markdown is the source of
truth**; every edit mutates source text. No `gpui` dependency — the UI layer
(M3) consumes `Projection`, `Tx`, and the IME-shaped methods on `Editor`.

## Data model

```
markdown source (ropey::Rope, UTF-8 byte offsets, grapheme-aware)
        │  pulldown-cmark::into_offset_iter
        ▼
Doc { children: [Node] }                     // md/ast.rs, md/parse.rs
        │  + markers::collect()              // md/markers.rs
        ▼
project(doc, src, sel) -> Projection         // project/
   text:  projected ("visible") text
   spans: [Span { vis, src, marks, block, payload }]
   chunks:[Chunk { vis, src, atomic }]        // bidirectional map
```

- `Buffer` (ropey): byte/grapheme/UTF-16 conversions, `replace() -> removed`
  gives history its inverse patches.
- `Selection { anchor, head }` — single cursor, byte offsets. `touches()`
  implements the Obsidian reveal rule.
- `Tx` — a set of `replace(start,end,text)` ops applied right-to-left plus an
  optional resulting selection. Commands are pure `fn(src, sel) -> Tx`;
  `Editor::apply` executes and records undo.
- `History` — `EditKind`-grouped entries; Insert/Delete/Ime merge while the
  pause between them < 500 ms (ProseMirror `newGroupDelay`). Undo restores
  document **and** selection.

## Parser choice: `pulldown-cmark` 0.13 + `into_offset_iter`

| option | verdict |
|---|---|
| tree-sitter-markdown | incremental parsing we don't need yet (notes are KB-scale, full reparse is ~µs/line); its dual-tree block/inline structure needs extra bookkeeping to map to one node model; heavier native dep. |
| **pulldown-cmark** | exact source ranges for every event/tag; `ENABLE_TABLES`, `ENABLE_TASKLISTS`, `ENABLE_STRIKETHROUGH` cover the GFM surface; pure Rust. Chosen. |

Verified empirically (`examples/dump_events.rs`): tag ranges cover the whole
construct incl. markers; text/code leaves carry inner ranges — so marker
ranges are `parent.range − children.range` fragments, computed in
`md/markers.rs` with bounded source scans (never wider than the construct).

Known gap: pulldown-cmark 0.13 `ENABLE_GFM` does **not** implement bare-URL
autolinking — `project/autolink.rs` adds a small scanner over `Text` leaves
(`http(s)://`/`www.`/`mailto:` plus trailing-punctuation trimming) so bare
URLs get `LINK` marks.

## Live Preview projection rules

`project()` walks the tree; gaps between siblings/children are carved into
literal pieces vs. marker pieces.

- **Hidden unless touched**: heading `#`, emphasis `** _ * ~~`, backticks,
  link `[]()` syntax, image `![]()` syntax, quote `> `, fenced code fences,
  list bullet markers, task `[ ]`, table `|` and the delimiter row.
- **Reveal** when `sel.touches(owner)`: the raw marker text renders with
  `Marks::MARKER` — caret or any selection overlap counts (Obsidian rule).
- **Substitutions** (hidden markers that still show): bullet `- ` → `• `,
  task `[ ]`/`[x]` → `☐`/`☑`, table `|` → `│`. Ordered `N.` markers carry
  information and stay literal text.
- **Joiners**: between sibling blocks the source newline(s) collapse to one
  `\n` (+ trailing indent from literal whitespace only — a hidden `> ` must
  not leak). Container *edge* gaps drop newlines entirely; a line's real
  separation comes from the joiner.
- **Tables**: cursor outside → cell text with `│` separators (delimiter row
  hidden); cursor inside → raw source lines (`Payload::TableRaw`).
- **Fenced code**: content keeps `Marks::CODE`; when hidden, the `\n` after
  the opening fence and before the closing fence are structural drops.
- **Map**: `chunks` tile the source; `to_visible`/`to_source` binary-search
  and snap inside `atomic` chunks (images, widgets, joiners) to the chunk's
  edges.

## Commands (`cmd/`)

TipTap StarterKit + TaskList parity: inline marks wrap/strip/split
(`**a|bc|**` unwraps; mid-run selection splits the run), headings H1–H3+,
bullet/ordered/task lists, checkbox toggle, quote, fenced code with language,
horizontal rule (split so `---` can never become a setext heading), link and
image insertion, local-image helpers mirroring `localImages.ts` (bare path in
markdown, `kosmos-local-image://file/<encodeURIComponent>` for display).

`keys.rs`: Enter continues/splits list markers (re-numbering ordered lists),
empty item lifts out; quote continues `> ` (PM `splitBlock`); headings split
keeping level; code blocks keep indent +2 after `{[(:` — plus Shift+Enter
(`\`-break, Vue serialization), marker-aware Backspace (innermost-first lift),
Tab/Shift+Tab (2-space indent for lists/code, `\t` elsewhere).

## Paste

`paste(plain, html)`: HTML (if it looks like markup) → `html2md` minimal
converter (headings, p/br/hr, strong/em/del/code, pre, blockquote,
ul/ol/li + checkboxes, a/img, tables, entities; script/style dropped);
otherwise text inserts verbatim — pasted plain text *is* markdown.

## IME

`EntityInputHandler`-shaped text subset, UTF-16 units at the API boundary:
`text_for_range`, `selected_text_range_utf16`, `marked_text_range_utf16`,
`replace_text_in_range`, `replace_and_mark_text_in_range`, `unmark_text`,
`set_marked_text`. A live preedit is the implicit replacement target.
Bounds/point queries intentionally absent (they belong to the renderer).

## Char count

`count_chars` replicates `charCount.ts` over our tree: code points in text;
`+1` per leaf block after the first; `+1` per hard break (in-paragraph
newlines count — Vue's markdown reader maps them to hardBreak); containers
free; top-level images split their paragraph but count 0; items' inline
content counts as one leaf (tiptap `listItem > paragraph`); **tables count as
raw text** (Vue's line reader has no table node).

## Known limits

- No incremental reparse — whole doc reparsed per change (rope keeps edits
  O(log n); parse is linear and fast enough for notes; revisit in M3).
- Inline HTML renders as raw text (`Payload::Html`).
- `Tx` ops are source offsets — commands don't compose inside a single Tx
  (they compose via sequential `apply`).
- Word/Outlook list detection in pasted HTML is minimal (`mso-list` styles
  not interpreted → items degrade to paragraphs).
- Bare-URL autolink scanner is heuristic (common TLD-agnostic; trailing
  `).,;:!?` trimmed) — not full GFM spec.
- Setext headings/`#nospace` etc. follow CommonMark via pulldown-cmark.
