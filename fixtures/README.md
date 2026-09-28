# fixtures/

Deterministic ARK data snapshot used for reference screenshots and later for
GPUI parity tests.

## Contents

- `ark-snapshot.json` — objects + object links as stored by Engine:
  - `com.kosmos.note` notes: kitchen-sink markup (headings, marks, link,
    nested bullet/ordered/task lists, blockquote, `codeBlock` in
    rust/typescript/python, `horizontalRule`, `hardBreak`, `image`),
    a 10k-line note, empty notes, unknown tiptap nodes/marks, and legacy
    content shapes (`markdown` payload, raw ProseMirror doc, flat
    pre-0.6.3 `propsJson` with `eden:` keys).
  - typed objects covering every system type: `book_obj`,
    `system-type-person`, `game_obj`, `system-type-image`,
    `system-type-workout`, `system-type-exercise`, plus a custom
    `project_obj` note-type record and a typed note using it.
  - collection records (`collection_obj`, both canonical and legacy
    `typeId` forms), a trashed note, `com.kosmos.task` tasks
    (todo/done/canceled).
  - diary bubbles (`entry_kind: "bubble"` under `extensions`, kinds
    plain/idea/task/highlight, tags) including a `reply_to` thread and a
    legacy `system-type-journal` bubble (reader-only), plus two legacy dated
    journal entries (`journal-2026-09-24` / `journal-2026-09-25`) that feed
    the diary migration path. NOTE: ARK bubble objects seed into the Engine
    but Vue's `listBubbles()` reads flat `propsJson` keys, so they don't
    appear in the diary UI — see the Byte-compat section of `PARITY.md`.
- `large-note.json` — the 10k-paragraph `contentJson` (also inlined into the
  snapshot object `note-large-10k`).
- `obsidian-vault/` — small Obsidian vault (frontmatter, daily notes,
  attachments, wiki-links) for the Obsidian import flow.

## Regenerate

    node fixtures/build-snapshot.mjs   # rewrites both JSON files deterministically

## Seed into a clean Engine

    MUNDUS_DATA_DIR=<engine data dir> node scripts/seed-fixtures.mjs

Reads `<data-dir>/engine.lock.json`, replays `upsert_object` /
`upsert_object_link` over `http://127.0.0.1:<port>/v1/rpc` as a
`desktop-host` client (same transport the Host E2E helpers use).
Records marked `"$seed": false` are reader-path fixtures (unknown nodes,
legacy payloads, retired typeIds) and are skipped — a current Engine rejects
them by design. A clean run prints `failures: 0`.

Wire note: requests carrying an `id` param must also set `_req_id`; otherwise
the Engine consumes top-level `id` as the request id.
