#!/usr/bin/env node
// Generates crates/memoria-model/fixtures/ark-snapshot.json and fixtures/large-note.json.
// Deterministic: fixed ids and ISO timestamps. `node fixtures/build-snapshot.mjs`.
//
// Canonical-write contract (ark-core canonical_ingress, enforced by Engine):
//   - contentJson is a bare ProseMirror doc {type:"doc",content:[...]} whose
//     node/mark types must be in the type's contentContract (note:
//     doc/paragraph/text/heading/bulletList/orderedList/listItem/blockquote/
//     codeBlock/hardBreak/horizontalRule/taskList/taskItem/image; marks:
//     bold/italic/strike/code/link/underline).
//   - propsJson is closed: only `description` and `extensions` for
//     com.kosmos.note; custom data (memoria_type_id, entry_kind, bubble_kind,
//     tags, typed-note fields) lives inside `extensions` — Vue Memoria reads
//     them identically via readMemoriaProps().
//   - com.kosmos.task propsJson requires all canonical fields.
// Objects that a current Engine would reject on write (unknown nodes/marks,
// legacy content wrappers, flat pre-0.6.3 props, retired typeIds) carry
// "$seed": false — they exist for reader-path unit tests, not for seeding.
import { writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { doc, kitchenSinkDoc, para, text } from "./content.mjs";
import { fillRecords } from "./objects.mjs";

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));

const ts = (d, h = 12, m = 0) =>
  `2026-09-${String(d).padStart(2, "0")}T${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:00.000Z`;

// Vue Memoria's write path stores the entry's memoria type in
// props.extensions.memoria_type_id (MEMORIA_TYPE_ID_PROP). Without it the
// entry's type_id falls back to the raw ARK typeId ("com.kosmos.note") and
// Everything filters it out (it renders only note_obj/book_obj), so plain
// notes carry the default here; bubbles/typed/collection objects pass a full
// extensions bag that overrides it.
const note = (id, title, contentJson, propsJson = {}, extra = {}) => ({
  id,
  typeId: "com.kosmos.note",
  typeVersion: "1.0.0",
  title,
  contentJson,
  propsJson: {
    description: null,
    extensions: { memoria_type_id: "note_obj" },
    ...propsJson,
  },
  createdAt: ts(20),
  updatedAt: ts(24),
  deletedAt: null,
  ...extra,
});

const objects = [];
const links = [];

// --- Notes -----------------------------------------------------------------
objects.push(note("note-markup-kitchen-sink", "Полигон разметки", kitchenSinkDoc()));

objects.push(
  note("note-empty", "", doc([])),
  note("note-empty-title", "Без тела", doc([para()])),
);

// Reader-path edge cases — rejected by canonical ingress, so they exist only
// for unit tests replaying the snapshot file directly.
objects.push(
  note(
    "note-unknown-node",
    "Неизвестные узлы",
    doc([
      para(text("Перед неизвестным узлом.")),
      { type: "sticker_board", attrs: { zoom: 2 }, content: [para(text("hidden?"))] },
      para(text("Текст с ", { type: "text", text: "неизвестной меткой", marks: [{ type: "glitter" }] }, text("."))),
    ]),
    {},
    { $seed: false },
  ),
  note(
    "note-legacy-markdown",
    "Легаси markdown",
    { type: "markdown", version: 1, text: "# Заголовок\n\nСтарый формат контента." },
    { description: "pre-tiptap payload" },
    { $seed: false },
  ),
  note(
    "note-legacy-eden-props",
    "Легаси eden-поля",
    doc([para(text("Плоские props без extensions-обёртки."))]),
    // pre-0.6.3: fields lived at propsJson top level, no `extensions` bag.
    { memoria_type_id: "note_obj", pinned: true, eden_flag: "eden:legacy" },
    { $seed: false },
  ),
  note(
    "note-legacy-raw-doc",
    "Легаси doc",
    doc([para(text("contentJson без обёртки tiptap"))]),
  ),
);

objects.push(
  note("note-trash", "Удалённая заметка", doc([para(text("В корзине"))]), {}, { deletedAt: ts(25) }),
);

// The 10k-line note body lives in large-note.json (generated below).
objects.push(note("note-large-10k", "Длинная заметка (10k строк)", { $ref: "large-note.json" }));

fillRecords(objects, links, note, ts);

// --- large note --------------------------------------------------------------
const large = doc(
  Array.from({ length: 10_000 }, (_, i) =>
    para(text(`Строка ${i + 1}: проверка прокрутки и отрисовки длинного документа.`)),
  ),
);

const inline = (v) => JSON.parse(JSON.stringify(v));
const snapshot = {
  $schema: "memoria-gpui/fixture-snapshot@1",
  generated_by: "fixtures/build-snapshot.mjs",
  objects: objects.map((o) => inline(o)),
  links: links.map((l) => inline(l)),
};

for (const o of snapshot.objects) {
  if (o.contentJson && o.contentJson.$ref === "large-note.json") o.contentJson = large;
}
writeFileSync(
  path.join(root, "crates", "memoria-model", "fixtures", "ark-snapshot.json"),
  JSON.stringify(snapshot, null, 2) + "\n",
);
writeFileSync(
  path.join(root, "fixtures", "large-note.json"),
  JSON.stringify({ contentJson: large }, null, 2) + "\n",
);
const skipped = snapshot.objects.filter((o) => o.$seed === false).length;
console.log(
  `objects=${snapshot.objects.length} links=${snapshot.links.length} seedable=${snapshot.objects.length - skipped} reader_only=${skipped}`,
);
