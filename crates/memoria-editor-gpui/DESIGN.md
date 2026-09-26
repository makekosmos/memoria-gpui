# memoria-editor-gpui — design (KOS-150, M3)

GPUI rendering/input layer over `memoria-editor-core`. The core owns the
markdown source of truth, projection, selection and IME conversions; this
crate owns pixels, hit-testing, focus, blink, zoom, highlighting, images and
the `EditorEvent` surface the app shell (`src/app.rs`) consumes.

## Wiring

```
markdown source ──► memoria_editor_core::Editor
                        │ project_cached()  (M2: parse → Doc → Projection)
                        ▼
MemoriaEditor (entity)  rows::build  →  Vec<Row> (visible-text rows, block tags)
        │ Render        element.rs::EditorElement (custom canvas Element)
        ▼                prepaint: shape visible rows only (virtualization)
        EditorElement    paint:    code panels → rules/images → rows (selection
                                   underlay + text) → placeholder → caret
```

- `MemoriaEditor::new(src, window, cx)` — app creates it lazily once; markdown
  enters via `set_markdown` (load/refresh) and leaves via `markdown()` /
  `EditorEvent::Autosave(SharedString)`.
- `key_bindings()` is installed once in `main.rs`; all actions live under
  `key_context("MemoriaEditor")` on the focus-tracked root `div`.
- Events: `Edited`, `SelectionChanged`, `Autosave(md)`, `ZenToggled`,
  `ZoomChanged(f)`, `CtrlK` (reserved for M4 search wiring).

## IME / input

`EntityInputHandler for MemoriaEditor` (`input.rs`). GPUI's
`ElementInputHandler` is registered in `EditorElement::paint`, so the platform
IME machinery reaches the view every frame. Ranges at the API boundary are
UTF-16 units over **source markdown** (core contract); rendering maps
source → visible → screen:

```
utf16 range → buf.utf16_to_byte → proj.to_visible → row → shaped glyph x
```

`bounds_for_range` (IME candidate window) uses the caret/marker bounds of the
visible-mapped range; `character_index_for_point` reverses it for IME
hit-testing. Clipboard: `paste()` reads `ClipboardEntry::String` metadata —
HTML payloads go through the core's html→md path, plain text verbatim.

## Rows & virtualization

`rows.rs` turns `Projection::text` into `Row`s: split on `\n` (incl. the
synthetic separators the projection inserts between block siblings), clip
spans per row, tag rows by block (`Heading(n)`, `CodeBlock{lang}`, `Quote`,
`Item`, rule, standalone image). `layout.rs` keeps cumulative offsets/heights;
`EditorElement::prepaint` shapes **only the viewport rows**
(`layout.visible(scroll_y..scroll_y+h)`), re-measures once if heights changed,
then `DocLayout::relayout` fixes offsets. Large docs therefore pay shaping
cost per *visible* row, not per line.

Zoom (`EditorScale`) multiplies all font metrics; the wrap width and shaped
cache are invalidated on zoom/resize.

## Token ↔ Vue CSS map (`style.rs`)

| Vue (`src/editor-tiptap/tiptap.css` etc.) | Here |
|---|---|
| `--font-size:16px`, `line-height:1.7` | `BODY_SIZE=16`, `BODY_LH=1.7` |
| max-width 760px + `clamp(24px,4vw,40px)` gutter | `wrap_width()` = `min(w, 760·zoom)`, `text_origin_x()` centers the column with a `clamp`ed gutter |
| block margin ~`1.7em` (27.2px) | `BLOCK_GAP` per-row top pad |
| H1 2em / H2 1.6 / H3 1.3 / H4 1.15 / H5 1em+uppercase / H6 0.92em muted | `block_style()` sizes × zoom |
| code: 13px mono, padding 12/14, radius 8, dark surface bg | `CODE_*` consts; `paint::code_panel` quad |
| selection = accent @ 22% | `selection_bg()` = `accent.opacity(0.22)` |
| links accent+underline | `runs.rs` underlines `Marks::LINK`, `accent()` |
| marker/dim text (hidden markup reveal) | `marker_color()` |
| placeholder «Начните писать...» | `PLACEHOLDER`, drawn only when doc empty |
| caret 1px accent, ~530 ms blink | `CARET_W`, `BLINK_MS` |

## Highlighting (`highlight.rs`, `languages.rs`)

`gpui-component`'s `LanguageRegistry`/`SyntaxHighlighter` + `HighlightTheme`
cover 25 of Vue's 31 `SHIKI_LANGUAGES`; six grammars ship directly
(`tree-sitter-{ini,less,objc,perl,r,xml}`) registered into the registry.
Styles cache by `(language, code-hash)`; >24 KB blocks skip highlighting
(plain fallback). Missing grammar/query → plain text (documented GAP, never
an error). Mapping: theme scopes → gpui `HighlightStyle` (`editor` rows keep
marker color; only code content is re-tinted).

## Language picker (`picker.rs`)

`Ctrl+F4`-style trigger (`OpenLangPicker` action, wired in `view.rs`) opens a
searchable 31-language list; `picker_key` handles arrows/enter/esc/input in
`capture_key_down` before dispatch. Choosing a language calls
`Command::SetCodeLang{lang}` → `block::set_code_block_lang` rewrites the
opening fence info string; carets inside the info string are pinned to the
end of the new token.

## Images (`images.rs`)

`Payload::Image` rows resolve URLs like `localImages.ts`: absolute/`file://`
paths direct, `kosmos-local-image://file/<enc>` decoded, relative joined onto
`image_root` (app sets `MEMORIA_IMAGES_DIR`). Decode is lazy via
`window.use_asset` (gpui image cache); placeholders paint while loading, a
"broken image" stub paints on decode failure.

## Mouse (`mouse.rs`)

Window-space point → `vis_index_for_point` (row_at_y → shaped
`index_for_position`) → `proj.to_source` — so clicking text that rendered with
hidden markers lands at the real source offset. click=caret, drag=selection
(`on_mouse_move` extends), dbl=word (2-click `click_count`), tri=paragraph.

## Hotkeys (`view.rs`, `actions.rs`)

All bindings match **physical** keys — `Keystroke.key` is the ASCII-equivalent
keycap (gpui-pre-linux `guess_ascii`), so `Ctrl+Б` ≡ `Ctrl+B` on Russian
layouts — mirroring Vue's `e.code`-based `useKeyboard.ts`. Zen is the
`Ctrl+K, Z` chord (`CtrlK` arms a 2 s window; `capture_key_down` consumes a
bare `z` while armed) plus the `Ctrl+Alt+Z` alternative. Zoom: `Ctrl+=` /
`Ctrl+Numpad+`, `Ctrl+-` / `Ctrl+Numpad-`, `Ctrl+0` → `×1.1`/`÷1.1`/`1.0`.

## App integration (`src/app.rs`, `app/render.rs`, `app/refresh.rs`)

`Entity<MemoriaEditor>` replaces the M1 `TextareaState`. `Edited` sets
`dirty`; `Autosave(md)` (editor-internal 300 ms debounce) →
`send_save` → `Command::SaveEntry` with `write_entry_markdown(md)`; `Saved`
clears dirty + `mark_saved()`. `EngineEvent::Changed` → `LoadEntry` →
`refresh_decision` (`app/refresh.rs`): note-switch always applies; same note
goes through `should_apply_remote_entry` (self-echo fingerprint skip, dirty
skip) so remote changes never wipe in-progress input. Zen hides the sidebar;
the status bar shows `char_count()` (core `charcount`, Vue parity).

## Known limits / GAPs

- No incremental parse — whole doc reprojects per edit (fine at note scale;
  `perf_ten_thousand_lines` ignored test measures it).
- Selection is single-range (core contract); no block/multi-select.
- Table cells render as monospace grid text; cell-aware selection is a GAP.
- Language picker triggers via hotkey only — no hover toolbar yet
  (`EdenCodeBlockTools` parity is partial).
- Image resize/drag handles are not implemented (view-only).
- Zen/zoom are editor-local; app chrome (nav, titlebar) is M4+.
