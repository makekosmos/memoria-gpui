# PARITY.md — Memoria GPUI parity matrix

Source of truth: `makekosmos/memoria` (Vue 3 / Tiptap) @
`7ccbb9f89841fc93c8bcb48aef67f3df2d3df84e` (`memoria-kos-147` worktree).
All paths below are relative to that commit's `src/` unless noted.

Milestone labels follow the KOS-146 epic plan (M1–M9). Grouping was derived
from the KOS-147 AGENT_SECTION feature list (editor, shell/nav, data/store,
types, books, diary bubbles, stickers, settings, Obsidian, Kepler tasks, Eden
migration read-only); if the epic's exact M-numbering differs, adjust the
`M` column labels — the `feature → file` mapping stays valid.

| M | scope |
|---|-------|
| M1 | Editor & content (tiptap document model, `content_json` byte-compat, block selection, code highlight, images) |
| M2 | Shell & navigation (window chrome, top nav, screens incl. «Всё» grid, history, zen mode, shortcuts, theme) |
| M3 | Engine data layer (`/v1/rpc` store, entry CRUD, drafts, conflicts, live refresh, Kepler task sync) |
| M4 | Typed notes & object types (note types, system types, typed headers, property fields, object lists) |
| M5 | Books (covers, cover dropzone, metadata import) |
| M6 | Diary bubbles (timeline, calendar sidebar, renderer, model) |
| M7 | Stickers (floating sticker note windows, docked widget) |
| M8 | Settings & search (settings page, general/export/trash sections, preferences, search overlay, conflict banner) |
| M9 | Obsidian vault import/export + Eden legacy migration (read-only consumption) |

Status values: `TODO` — not started; `DONE` — parity verified against reference
screenshots in `reference/screens/`; `PARTIAL` — data layer ported, scope
noted in the status cell; `DONE (M1)` — ported + Rust tests green (M1 has no
screenshots to compare — unit/golden tests are the parity evidence).

## Feature matrix

| Feature | Vue source @7ccbb9f | M | Status |
|---|---|---|---|
| Tiptap editor surface (all marks/blocks, tasks, code w/ shiki, images) | `src/editor-tiptap/TiptapEditor.vue`, `src/editor-tiptap/shikiHighlight.ts` | M1 | TODO |
| `content_json` codecs (tiptap/markdown/legacy, byte-compat) | `src/editor-content/content.ts` | M1 | DONE (M1) |
| Block selection (classes, pointer, composable) | `src/lib/blockSelectionClasses.ts`, `src/lib/blockSelectionPointer.ts`, `src/composables/useBlockSelection.ts` | M1 | TODO |
| Char counter | `src/lib/charCount.ts`, `src/composables/useCharCounter.ts` | M1 | DONE (M1) |
| Window chrome / titlebar | `src/App.vue`, `src/Titlebar.vue`, `src/Titlebar.css` | M2 | TODO |
| Top navigation («Всё»/«Дневник») + screen state | `src/App.vue` (`eden-top-navigation`), `src/store/eden.ts` (`activeScreen`) | M2 | TODO |
| «Всё» grid + item cards | `src/components/everything/EverythingView.vue`, `src/components/everything/EverythingItemCard.vue` | M2 | TODO |
| Navigation history (back/forward) | `src/composables/useNavigationHistory.ts`, `src/lib/kepler-navigation.ts` | M2 | TODO |
| Zen mode / layout state | `src/store/layout.ts` | M2 | TODO |
| Keyboard shortcuts (physical keys) | `src/composables/useKeyboard.ts` | M2 | TODO |
| Theme (light/dark) | `src/composables/useTheme.ts`, `src/index.css`, `src/App.css` | M2 | TODO |
| Platform detection | `src/composables/usePlatform.ts` | M2 | TODO |
| Engine store (Pinia `eden` store + actions) | `src/store/eden.ts`, `edenStore{Data,Draft,Save,NoteType,SystemType}Actions.ts`, `edenStoreHelpers.ts`, `edenEntryFactory.ts` | M3 | PARTIAL (M1: `edenStoreSaveActions` coordinator + worker `Command`/`Reply` ported; draft/data/system-type actions TODO) |
| Kepler/Engine API surface (`window.api` shim → `ark.request`) | `src/lib/kepler-api-shim.ts`, `src/lib/edenApi.ts`, `src/lib/kepler-ui-runtime.ts`, `src/lib/kepler-command-bus.ts`, `src/lib/kepler-folder-stubs.ts` | M3 | PARTIAL (M1: `ark.request` transport + command bus ported + fake-Engine tests; `edenApi`/`kepler-ui-runtime`/`kepler-folder-stubs` TODO) |
| Entry ↔ ARK object mapping | `src/lib/kepler-entry-api.ts`, `src/lib/kepler-entry-mappers.ts` | M3 | DONE (M1) |
| Live refresh / subscriptions / list filter | `src/store/liveRefresh.ts`, `src/store/edenLiveRefreshSubscription.ts`, `src/store/liveListFilter.ts` | M3 | PARTIAL (M1: pure fns + WS subscription ported; per-entry live-merge wiring TODO) |
| Entry conflicts & change tracking | `src/store/entryConflicts.ts`, `src/store/entryChanges.ts`, `src/lib/saveResult.ts` | M3 | DONE (M1) |
| Kepler task sync (`com.kosmos.task`) | `src/lib/kepler-task-sync.ts`, `src/lib/taskStatus.ts` | M3 | TODO |
| Trash storage over ARK soft-delete | `src/lib/kepler-trash-storage.ts` | M3 | DONE (M1) |
| Search | `src/composables/useSearch.ts`, `src/components/SearchOverlay.vue` | M3/M8 | TODO |
| Note types & system types | `src/lib/typedNotes.ts`, `src/lib/typedNoteSchemas.ts`, `src/lib/systemTypes.ts`, `systemType{,Game,Visual}Definitions.ts`, `src/lib/kepler-note-type-api.ts` | M4 | DONE (M1) |
| Typed note header & property editors | `src/components/typed-notes/{TypedHeader,ObjectPropertyField,ObjectPropertyPicker}.vue`, `src/lib/typedNoteHeaderProps.ts`, `src/lib/objectFieldFormatting.ts` | M4 | PARTIAL (M1: `typedNoteHeaderProps` ported; UI TODO) |
| Type object lists / image objects | `src/components/objects/{TypeObjectsView,ImageObjectView}.vue`, `src/lib/objectImages.ts`, `src/lib/iconResolver.ts` | M4 | PARTIAL (M1: `iconResolver` ported; UI TODO) |
| Entry titles | `src/lib/entryTitles.ts` | M4 | DONE (M1) |
| Books: cover, dropzone, metadata import | `src/components/books/{BookCover,BookCoverFileDropzone,BookMetadataImportModal}.vue`, `src/lib/bookMetadata.ts`, `src/lib/bookLanguages.ts` | M5 | TODO |
| Diary bubbles view + timeline + calendar | `src/components/bubbles/{BubbleDiaryView,BubbleTimelineItem,BubbleTiptapRenderer,BubbleDiaryCalendarSidebar}.vue`, `bubbleDiaryModel.ts`, `src/lib/kepler-bubble-api.ts` | M6 | TODO |
| Stickers (floating note windows) | `src/views/StickerNoteView.vue`, `src/lib/sticker.ts`, `src/composables/useDockedWidget.ts` | M7 | TODO |
| Settings page + sections | `src/components/settings/{SettingsPage,GeneralSettings,ExportSettings,TrashSettings}.vue(+css)`, `src/views/EdenSettingsView.vue`, `src/composables/usePreferences.ts` | M8 | TODO |
| Conflict banner | `src/components/EntryConflictBanner.vue` | M8 | TODO |
| FPS monitor (dev overlay) | `src/composables/useFpsMonitor.ts` | M8 | TODO |
| Obsidian vault import/export (+ images, frontmatter, journal transaction) | `src/lib/obsidianVault{,Export,ExportAssets,ExportAssetPaths,ImportFrontmatter,ImportImages,ImportTransaction}.ts`, `src/lib/markdownFrontmatter.ts` | M9 | PARTIAL (M1: `markdownFrontmatter` ported; import/export TODO) |
| Eden legacy migration (read-only: storage keys, userData, command prefixes) | `src/lib/memoria-migration.ts`, `manifest.json` `legacy_*` fields | M9 | PARTIAL (M1: storage/userData/command-prefix ports + tests; `manifest.json` contract is Vue-only) |
| Local image resolution | `src/lib/localImages.ts` | M1 | TODO |
| App bootstrap / env types | `src/main.ts`, `src/vite-env.d.ts` | M2 | n/a — replaced by Rust app shell |
| Sidebar leftovers | `src/components/sidebar/types.ts` | — | не нужен: sidebar was removed; only a dead `types.ts` (3 lines) remains |

## Screens for reference (`reference/screens/`)

Captured on the real Electron Host + Engine (cortex `host/e2e` harness,
visible Xvfb windows, fixture snapshot seeded). Spec source:
`scripts/e2e/kos147-memoria-screens.spec.ts`.

| Screen | Route/state | Shots |
|---|---|---|
| Всё (home grid, populated) | default screen | `everything-{light,dark}.png` |
| Всё empty state | clean Engine, no fixtures | `everything-empty-{light,dark}.png` |
| Note editor (kitchen-sink) | `/note/note-markup-kitchen-sink` | `editor-{light,dark}.png` |
| Note editor, empty title | `/note/note-empty-title` | `editor-empty-dark.png` |
| Diary (empty; ARK bubbles unwritable — see Byte-compat) | «Дневник» top-nav | `diary-{light,dark}.png` |
| Typed object list (books) | `/note/collection:book_obj` | `types-books-{light,dark}.png` |
| Book detail + metadata fields | `/note/book-1` | `book-light.png` |
| Settings | `#/settings` | `settings-{light,dark}.png` |
| Conflict banner («Изменено удалённо») | seeded collection re-asserted by hydration | `conflict-banner-dark.png` |
| Aux window via `kepler.window.open` | `key: sticker:kos147` (renders main surface — M7 risk, see below) | `aux-window-dark.png` |
| Engine down | Engine terminated, page reloaded | `engine-down-dark.png` |

NOT_RUN:
- `sticker-*` surface — `#/sticker/<id>` hash and the aux-window route both
  render the main app, not `StickerNoteView`, on the built 0.6.9 bundle
  (aux URL carries the hash but `hashStickerEntryId` doesn't take effect;
  investigate in M7 — may be a build-vs-source drift or a host route issue).
- Populated diary — `propsJson.entry_kind`/`bubble_kind`/`tags` writes are
  rejected by the launch grant + canonical ingress, so no ARK bubble can be
  seeded or created through the shipped write path; diary shows
  «Пока нет записей». Flagged in the Byte-compat section.
- Hover/selected/error card states — Electron pointer hit-testing on the
  titlebar/grid was unreliable under Xvfb; nav used DOM clicks. Re-run the
  spec and add pointer shots on a session with working hit-testing.

## Coverage check

Every file under Vue `src/` is either mapped to a milestone above or marked
«не нужен». Verify with:

```bash
cd /workspace/wt/memoria-kos-147   # Vue SoT worktree @7ccbb9f
comm -23 <(cd src && find . -type f | sort) \
         <(grep -oE 'src/[A-Za-z0-9_./-]+' /workspace/wt/memoria-gpui-kos-147/PARITY.md | sed 's|src/||;s|/$||' | sort -u)
```

(Expected output: only files whose names appear inside directory rows —
see "не нужен" row above — everything else must be empty.)

## Byte-compat contract

- ARK `object.contentJson` on the wire is the **bare ProseMirror doc**
  `{ "type": "doc", "content": [...] }` — the Engine's `canonical_ingress`
  validates `rootType: "doc"` plus the `com.kosmos.note` contentContract
  (allowed nodes/marks) and **rejects the `TiptapContent` wrapper**.
- Vue `Entry.content_json` (string column inside the app) is the wrapped form:
  `TiptapContent { type:"tiptap", version:1, doc }` |
  `MarkdownContent { type:"markdown", version:1, text }` | raw doc (legacy).
  Codecs in `src/editor-content/content.ts`.
- ARK objects: `typeId "com.kosmos.note"`/`"com.kosmos.task"`;
  `propsJson` is closed — only `description` and `extensions` for notes.
  Memoria fields (`memoria_type_id`, `memoria_record_kind`, typed fields) live
  inside `extensions`; `readMemoriaProps` merges extensions over flat props.
- ⚠ Findings vs this Engine build (verified while seeding, KOS-147 run):
  - Flat custom `propsJson` keys (e.g. Vue's `entry_kind`/`bubble_kind`/`tags`,
    legacy `eden:*`/`memoria_type_id` top-level) are rejected twice: the
    launch grant allows only `title|content|props.description|props.extensions`
    writes, and canonical ingress enforces `additionalProperties:false`.
    Consequence: `createBubble`/`migrateBubble` writes are refused today —
    diary ARK bubbles cannot be created through the shipped path. The diary
    still renders legacy dated journal entries, which the app tries to
    migrate into bubble objects (also rejected). Flag for the epic — either
    the manifest grant must grow the bubble fields or the write must move
    under `props.extensions` in Vue.
  - `list_object_links`/`upsert_object_link` `reply_to` links seed fine.
- `id` is reserved as the legacy JSON-RPC request id on `/v1/rpc`; every
  request must carry `_req_id` so object `id` params survive the wire.

## File coverage (every `src/` file)

| File | M | Status | Note |
|---|---|---|---|
| `src/App.css` | M2 | TODO | visual source for tokens; port via imago-gpui tokens, no hex |
| `src/App.vue` | M2 | TODO | root shell: titlebar, top-nav, screens, conflict banner mount |
| `src/Titlebar.css` | M2 | TODO |  |
| `src/Titlebar.vue` | M2 | TODO |  |
| `src/components/EntryConflictBanner.vue` | M8 | TODO |  |
| `src/components/SearchOverlay.vue` | M8 | TODO |  |
| `src/components/books/BookCover.vue` | M5 | TODO |  |
| `src/components/books/BookCoverFileDropzone.vue` | M5 | TODO |  |
| `src/components/books/BookMetadataImportModal.vue` | M5 | TODO |  |
| `src/components/bubbles/BubbleDiaryCalendarSidebar.vue` | M6 | TODO |  |
| `src/components/bubbles/BubbleDiaryView.vue` | M6 | TODO |  |
| `src/components/bubbles/BubbleTimelineItem.vue` | M6 | TODO |  |
| `src/components/bubbles/BubbleTiptapRenderer.vue` | M6 | TODO |  |
| `src/components/bubbles/bubbleDiaryModel.ts` | M6 | TODO |  |
| `src/components/everything/EverythingItemCard.vue` | M2 | TODO |  |
| `src/components/everything/EverythingView.vue` | M2 | TODO |  |
| `src/components/objects/ImageObjectView.vue` | M4 | TODO |  |
| `src/components/objects/TypeObjectsView.vue` | M4 | TODO |  |
| `src/components/settings/ExportSettings.vue` | M8 | TODO |  |
| `src/components/settings/GeneralSettings.vue` | M8 | TODO |  |
| `src/components/settings/SettingsPage.css` | M8 | TODO |  |
| `src/components/settings/SettingsPage.vue` | M8 | TODO |  |
| `src/components/settings/TrashSettings.vue` | M8 | TODO |  |
| `src/components/sidebar/types.ts` | — | skip | не нужен — dead leftover after sidebar removal; no runtime user |
| `src/components/typed-notes/ObjectPropertyField.vue` | M4 | TODO |  |
| `src/components/typed-notes/ObjectPropertyPicker.vue` | M4 | TODO |  |
| `src/components/typed-notes/TypedHeader.vue` | M4 | TODO |  |
| `src/composables/useBlockSelection.ts` | M1 | TODO |  |
| `src/composables/useCharCounter.ts` | M1 | TODO |  |
| `src/composables/useDockedWidget.ts` | M7 | TODO |  |
| `src/composables/useFpsMonitor.ts` | M8 | TODO |  |
| `src/composables/useKeyboard.ts` | M2 | TODO |  |
| `src/composables/useNavigationHistory.ts` | M2 | TODO |  |
| `src/composables/usePlatform.ts` | M2 | TODO |  |
| `src/composables/usePreferences.ts` | M8 | TODO |  |
| `src/composables/useSearch.ts` | M3 | TODO |  |
| `src/composables/useTheme.ts` | M2 | TODO |  |
| `src/editor-content/content.ts` | M1 | DONE (M1) |  |
| `src/editor-tiptap/TiptapEditor.vue` | M1 | TODO |  |
| `src/editor-tiptap/shikiHighlight.ts` | M1 | TODO |  |
| `src/index.css` | M2 | TODO | same — theme tokens only |
| `src/lib/blockSelectionClasses.ts` | M1 | TODO |  |
| `src/lib/blockSelectionPointer.ts` | M1 | TODO |  |
| `src/lib/bookLanguages.ts` | M5 | TODO |  |
| `src/lib/bookMetadata.ts` | M5 | TODO |  |
| `src/lib/charCount.ts` | M1 | DONE (M1) |  |
| `src/lib/edenApi.ts` | M3 | TODO |  |
| `src/lib/entryTitles.ts` | M4 | DONE (M1) |  |
| `src/lib/iconResolver.ts` | M4 | DONE (M1) |  |
| `src/lib/kepler-api-shim.ts` | M3 | TODO |  |
| `src/lib/kepler-bubble-api.ts` | M6 | TODO |  |
| `src/lib/kepler-command-bus.ts` | M3 | DONE (M1) |  |
| `src/lib/kepler-entry-api.ts` | M3 | DONE (M1) |  |
| `src/lib/kepler-entry-mappers.ts` | M3 | DONE (M1) |  |
| `src/lib/kepler-folder-stubs.ts` | M3 | TODO |  |
| `src/lib/kepler-navigation.ts` | M3 | TODO |  |
| `src/lib/kepler-note-type-api.ts` | M4 | DONE (M1) |  |
| `src/lib/kepler-task-sync.ts` | M3 | TODO |  |
| `src/lib/kepler-trash-storage.ts` | M3 | DONE (M1) |  |
| `src/lib/kepler-ui-runtime.ts` | M3 | TODO |  |
| `src/lib/localImages.ts` | M1 | TODO |  |
| `src/lib/markdownFrontmatter.ts` | M9 | DONE (M1) |  |
| `src/lib/memoria-migration.ts` | M9 | DONE (M1) |  |
| `src/lib/objectFieldFormatting.ts` | M4 | TODO |  |
| `src/lib/objectImages.ts` | M4 | TODO |  |
| `src/lib/obsidianVault.ts` | M9 | TODO |  |
| `src/lib/obsidianVaultExport.ts` | M9 | TODO |  |
| `src/lib/obsidianVaultExportAssetPaths.ts` | M9 | TODO |  |
| `src/lib/obsidianVaultExportAssets.ts` | M9 | TODO |  |
| `src/lib/obsidianVaultImportFrontmatter.ts` | M9 | TODO |  |
| `src/lib/obsidianVaultImportImages.ts` | M9 | TODO |  |
| `src/lib/obsidianVaultImportTransaction.ts` | M9 | TODO |  |
| `src/lib/saveResult.ts` | M3 | DONE (M1) |  |
| `src/lib/sticker.ts` | M7 | TODO |  |
| `src/lib/systemTypeDefinitions.ts` | M4 | DONE (M1) |  |
| `src/lib/systemTypeGameDefinitions.ts` | M4 | TODO |  |
| `src/lib/systemTypeVisualDefinitions.ts` | M4 | TODO |  |
| `src/lib/systemTypes.ts` | M4 | DONE (M1) |  |
| `src/lib/taskStatus.ts` | M3 | TODO |  |
| `src/lib/typedNoteHeaderProps.ts` | M4 | DONE (M1) |  |
| `src/lib/typedNoteSchemas.ts` | M4 | DONE (M1) |  |
| `src/lib/typedNotes.ts` | M4 | DONE (M1) |  |
| `src/main.ts` | M2 | TODO | replaced by Rust `src/main.rs` bootstrap |
| `src/store/eden.ts` | M3 | TODO |  |
| `src/store/edenEntryFactory.ts` | M3 | TODO |  |
| `src/store/edenLiveRefreshSubscription.ts` | M3 | TODO |  |
| `src/store/edenStoreDataActions.ts` | M3 | TODO |  |
| `src/store/edenStoreDraftActions.ts` | M3 | TODO |  |
| `src/store/edenStoreHelpers.ts` | M3 | TODO |  |
| `src/store/edenStoreNoteTypeActions.ts` | M3 | TODO |  |
| `src/store/edenStoreSaveActions.ts` | M3 | DONE (M1) |  |
| `src/store/edenStoreSystemTypeActions.ts` | M3 | TODO |  |
| `src/store/entryChanges.ts` | M3 | DONE (M1) |  |
| `src/store/entryConflicts.ts` | M3 | DONE (M1) |  |
| `src/store/layout.ts` | M2 | TODO |  |
| `src/store/liveListFilter.ts` | M3 | DONE (M1) |  |
| `src/store/liveRefresh.ts` | M3 | DONE (M1) |  |
| `src/views/EdenSettingsView.vue` | M8 | TODO |  |
| `src/views/StickerNoteView.vue` | M7 | TODO |  |
| `src/vite-env.d.ts` | M2 | TODO | vite ambient types — n/a for Rust |

