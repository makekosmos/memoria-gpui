# KOS-154 — M7 stickers: notes in floating windows

Linear: KOS-154 (epic KOS-146). PR: makekosmos/memoria-gpui.
Vue source of truth: `src/lib/sticker.ts`, `src/views/StickerNoteView.vue`,
`src/composables/useDockedWidget.ts` (worktree `memoria-kos-154`).

## What landed

- **Routes/keys** — `src/sticker_route.rs`: `sticker_route_for` /
  `parse_sticker_route` (`/sticker/<encodeURIComponent id>`, Vue validation:
  bad `%`, encoded `?`/`#`/`\`, whitespace, control chars, empty, >200 →
  reject; decoded `/` stays legal), `can_open_in_sticker` (note/book only),
  `sticker_window_key_for` (host-safe `sticker:<id>` else FNV-1a→base36 —
  fixed M4's `ch as u32` to Vue's `charCodeAt(0)` = UTF-16 code unit, so
  astral chars hash the high surrogate). Window size 380×480, min 260×200.
- **Shared document** — `src/app/doc.rs` `NoteDoc`: one `MemoriaEditor` +
  entry snapshot per entry id, shared by the main window and any sticker.
  Canonical `title_text` with **per-window** `InputState` bindings — a shared
  `InputState` caches per-window layout on the entity and `cx.notify`s on
  every paint, which deadlocked two windows in an infinite redraw loop.
- **Saves** — `NoteDoc` emits `DocEvent::Save`; `Memoria` forwards
  `Command::SaveEntry` through the existing `Backend` (Engine worker or
  `DemoStore`). `Reply::Saved` now carries the entry id so replies route to
  the owning doc when several are open. `mark_pending_save` keeps the
  autosave-race guard (stale replies don't clear newer dirty edits).
- **Editor** — `MemoriaEditor::attach_window` (per-window focus/blur subs on
  the shared editor) + `mark_pending_save`, both in `lifecycle.rs`.
- **Sticker view** — `src/app/sticker.rs`: mini titlebar (pin indicator +
  close), editable title, shared editor body, loading/failed states, no
  sidebar. `WindowKind::Floating`; reopening an open key activates the
  existing `WindowHandle` instead of spawning a second window.
- **Entry points** — context menu «Открыть стикером» (unchanged) + new
  `titlebar-open-sticker` button in `chrome.rs`, gated on
  `!zen && route is entry && can_open_in_sticker(type)`.

## GAPs (documented in PARITY.md «Стикеры»)

- `useDockedWidget`/host `kepler.window.open` docking needs the Mundus host —
  unavailable in the GPUI shell; `WindowKind::Floating` + keyed reuse instead.
- Runtime always-on-top toggle (Vue pin button calls host `setAlwaysOnTop`) —
  no runtime window-level API in GPUI; pin renders as an indicator. On
  Wayland always-on-top isn't compositor-guaranteed at all.
- In-sticker navigation — editor-level gap (no `Navigate` event in
  `MemoriaEditor`), not sticker-specific.

## Tests

- `sticker_route::tests` — route parse/format rejects+roundtrip, window-key
  host-safe/deterministic-FNV incl. astral-char UTF-16 unit, stickerable
  types.
- `ui_tests::sticker` (`TestAppContext`, `DemoStore` backend):
  `sticker_window_shares_document_and_dedupes_save` — opens via the real
  titlebar button, types in the sticker, shared markdown updates, exactly one
  autosave upsert; `sticker_reopen_focuses_existing_window` — reopen
  activates the same window (no second spawn);
  `sticker_for_other_entry_opens_its_own_doc` — separate docs per entry.
  Fake-clock caveat: `TestAppContext::run_until_parked` doesn't advance time;
  tests call `executor().advance_clock(AUTOSAVE_DEBOUNCE + 50ms)` to fire the
  debounce.

## Gates

`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings` (+CI `-A` set), `cargo nextest run --workspace
--all-features` (298 pass), `cargo deny check`, `cargo shear`,
`check-source-size` — all green.
