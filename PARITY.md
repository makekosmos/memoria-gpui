# PARITY.md — Memoria GPUI parity matrix

Source of truth: `makekosmos/memoria` (Vue 3 / Tiptap) @
`7ccbb9f89841fc93c8bcb48aef67f3df2d3df84e` (`memoria-kos-147` worktree).
All paths below are relative to that commit's `src/` unless noted.

Milestone labels follow the KOS-146 epic as actually landed (M9 audit update):

| M | scope | Ticket / PR |
|---|-------|-------------|
| M1 | Engine data layer + app shell scaffold | KOS-148 (merged, PR #1) |
| M2 | Editor core — markdown Live Preview, чистый Rust (`crates/memoria-editor-core`) | KOS-149 (merged, PR #2) |
| M3 | GPUI editor surface on editor-core | KOS-150 (merged, PR #4) |
| M4 | Shell & navigation — sidebar, search, history, settings, trash | KOS-151 (merged, PR #3) |
| M5 | Typed objects — properties, books, games, collections | KOS-152 (merged, PR #7) |
| M6 | Diary bubbles — timeline, calendar, threads, tags | KOS-153 (merged, PR #8) |
| M7 | Stickers — floating note windows | KOS-154 (merged, PR #5) |
| M8 | Obsidian vault import/export + Agenda task sync | KOS-155 (merged, PR #6) |
| M9 | Packaging into Kosmos + final parity audit | KOS-156 (this change) |

Status values: `DONE` — ported, parity verified (Rust tests / `ui_tests` /
reference screenshots in `reference/screens/`); `PARTIAL` — ported with a
documented residual scope gap; `GAP` — not ported; every GAP carries an
**accept** (deliberate divergence, closed) or **follow-up** (needs a ticket)
recommendation; `NOT_RUN` — verification pending. The M9 audit removed all
bare `TODO` rows; every sibling milestone (M1–M8) has merged to main.

## Feature matrix

| Feature | Vue source @7ccbb9f | M | Status |
|---|---|---|---|
| Tiptap editor surface (all marks/blocks, tasks, code w/ shiki, images) | `src/editor-tiptap/TiptapEditor.vue`, `src/editor-tiptap/shikiHighlight.ts` | M2/M3 | DONE (M2 core commands/keys/paste; M3 GPUI render + tree-sitter, см. «Редактор — M3 GPUI») |
| `content_json` codecs (tiptap/markdown/legacy, byte-compat) | `src/editor-content/content.ts` | M1 | DONE (M1) |
| Block selection (classes, pointer, composable) | `src/lib/blockSelectionClasses.ts`, `src/lib/blockSelectionPointer.ts`, `src/composables/useBlockSelection.ts` | M3 | GAP — not ported (editor is markdown-source, not a block DOM; text selection covers the main flows). **Recommend follow-up ticket** if block-level drag/multi-select is still wanted. |
| Char counter | `src/lib/charCount.ts`, `src/composables/useCharCounter.ts` | M1 | DONE (M1) |
| Window chrome / titlebar | `src/App.vue`, `src/Titlebar.vue`, `src/Titlebar.css` | M4 | DONE (M4: `src/app/chrome.rs` — drag area, back/forward, search button, entry menu, platform controls; imago tokens) |
| Top navigation («Всё»/«Дневник») + screen state | `src/App.vue` (`eden-top-navigation`), `src/store/eden.ts` (`activeScreen`) | M4 | DONE (M4: sidebar nav «Всё»/«Дневник» per milestone spec — Vue removed its sidebar and uses titlebar-center nav). GAP: sliding-indicator animation — **accept** (cosmetic; native sidebar layout is the agreed divergence) |
| «Всё» grid + item cards | `src/components/everything/EverythingView.vue`, `src/components/everything/EverythingItemCard.vue` | M4 | DONE (M4: `src/app/everything.rs` — icon + title + preview + pin marker, context menu) |
| Navigation history (back/forward) | `src/composables/useNavigationHistory.ts`, `src/lib/kepler-navigation.ts` | M4 | DONE (M4: `src/nav_history.rs`, `src/app/nav.rs` — 30-cap, seed, suppression, deleted-entry fallback; `ui_tests::card_opens_note_and_history_works`) |
| Zen mode / layout state | `src/store/layout.ts` | M4 | DONE (M3/M4: `Ctrl+K Z` chord toggles `zen` per Vue `useKeyboard` `currentEntry` guard — `src/app/editor_host.rs`; sidebar collapse Ctrl+B; layout persists via `local_state.rs` `memoria-local-state.json`) |
| Keyboard shortcuts (physical keys) | `src/composables/useKeyboard.ts` | M3/M4 | DONE (M4: Ctrl+K, Ctrl+B, Alt+←/→, Esc — layout-independent `keystroke.key`; M3: editor commands + zen chord + zoom `Ctrl +/-/0`) |
| Theme (light/dark) | `src/composables/useTheme.ts`, `src/index.css`, `src/App.css` | M4 | PARTIAL — imago-gpui tokens throughout (single palette, no hex in src). GAP: runtime light/dark toggle — **follow-up ticket** (needs an imago-gpui light palette + pref wiring) |
| Platform detection | `src/composables/usePlatform.ts` | M4 | DONE — n/a as a module: Rust `cfg!(target_os)`/GPUI platform APIs replace the composable at call sites (**accept**: no port needed) |
| Engine store (Pinia `eden` store + actions) | `src/store/eden.ts`, `edenStore{Data,Draft,Save,NoteType,SystemType}Actions.ts`, `edenStoreHelpers.ts`, `edenEntryFactory.ts` | M1/M5 | DONE for shipped scope (M1: `EntryApi` + save coordinator + worker `Command`/`Reply`; M4: `create_entry`, trash, conflicts, live refresh wired). Draft/data/system-type action residues are covered by the M5 property/object flows (merged, PR #7) |
| Kepler/Engine API surface (`window.api` shim → `ark.request`) | `src/lib/kepler-api-shim.ts`, `src/lib/edenApi.ts`, `src/lib/kepler-ui-runtime.ts`, `src/lib/kepler-command-bus.ts`, `src/lib/kepler-folder-stubs.ts` | M1 | DONE for shipped scope (M1: `ark.request` transport + command bus + fake-Engine tests — `src/store/transport*`, `src/command_bus.rs`). `edenApi`/`kepler-ui-runtime`/`kepler-folder-stubs` are host-bridge shims with no GPUI equivalent — **accept** (Apps→Engine direct, no `window.api`) |
| Entry ↔ ARK object mapping | `src/lib/kepler-entry-api.ts`, `src/lib/kepler-entry-mappers.ts` | M1 | DONE (M1) |
| Live refresh / subscriptions / list filter | `src/store/liveRefresh.ts`, `src/store/edenLiveRefreshSubscription.ts`, `src/store/liveListFilter.ts` | M1/M4 | DONE (M1: pure fns + WS subscription; M4: per-entry wiring — `src/app/refresh.rs` `refresh_decision`, self-echo + dirty guards; `tests/live_refresh.rs`) |
| Entry conflicts & change tracking | `src/store/entryConflicts.ts`, `src/store/entryChanges.ts`, `src/lib/saveResult.ts` | M1 | DONE (M1) |
| Kepler task sync (`com.kosmos.task`) | `src/lib/kepler-task-sync.ts`, `src/lib/taskStatus.ts` | M8 | DONE (M8: `src/task_status.rs` + `src/task_sync.rs` — schema ensure/normalize, getTask/patchTask/createTask/softDelete, object-event classify; fake-`ArkBridge` tests; merged, PR #6) |
| Trash storage over ARK soft-delete | `src/lib/kepler-trash-storage.ts` | M1 | DONE (M1) |
| Search | `src/composables/useSearch.ts`, `src/components/SearchOverlay.vue` | M4 | DONE (M4: `src/search_model.rs` + `src/app/search.rs` — Engine `search_objects`, 300ms debounce, highlight ranges, ↑↓/Enter/Esc; `ui_tests::search_*`) |
| Note types & system types | `src/lib/typedNotes.ts`, `src/lib/typedNoteSchemas.ts`, `src/lib/systemTypes.ts`, `systemType{,Game,Visual}Definitions.ts`, `src/lib/kepler-note-type-api.ts` | M1/M5 | DONE (M1 data layer; M5 adds game/visual object UIs — merged, PR #7) |
| Typed note header & property editors | `src/components/typed-notes/{TypedHeader,ObjectPropertyField,ObjectPropertyPicker}.vue`, `src/lib/typedNoteHeaderProps.ts`, `src/lib/objectFieldFormatting.ts` | M5 | DONE (M5, merged PR #7: `src/app/{typed_header,prop_field,prop_edit,prop_picker}.rs` — hero/avatar/cover, featured+secondary table, all field kinds incl. select/multi-select/relation pickers, read-only fields, edits persist via `SaveEntry`→`upsert_object`; `ui_tests::typed::*`) |
| Type object lists / image objects | `src/components/objects/{TypeObjectsView,ImageObjectView}.vue`, `src/lib/objectImages.ts`, `src/lib/iconResolver.ts` | M4/M5 | DONE (M4: `src/app/objects.rs`, `src/app/image.rs`, `src/object_views.rs`, `src/image_src.rs` — summary columns, gallery, person names; M5 (merged, PR #7) extends `object_images.rs`) |
| Entry titles | `src/lib/entryTitles.ts` | M1 | DONE (M1) |
| Books: cover, dropzone, metadata import | `src/components/books/{BookCover,BookCoverFileDropzone,BookMetadataImportModal}.vue`, `src/lib/bookMetadata.ts`, `src/lib/bookLanguages.ts` | M5 | DONE (M5, merged PR #7: `src/app/{book,cover_modal,metadata_modal,metadata_apply}.rs`, `src/book_metadata{,_extract}.rs` — cover+spine via Engine `images.*`, URL & OS-file-drop covers via `images.storeCover`, ISBN/URL metadata lookup via `bookMetadata.*`, preview→apply; `ui_tests::typed_book::*`) |
| Diary bubbles view + timeline + calendar | `src/components/bubbles/{BubbleDiaryView,BubbleTimelineItem,BubbleTiptapRenderer,BubbleDiaryCalendarSidebar}.vue`, `bubbleDiaryModel.ts`, `src/lib/kepler-bubble-api.ts` | M6 | DONE (M6: `src/diary/*` model + `src/store/bubble_api.rs` + `src/app/diary{,_item,_forms,_blocks,_calendar}.rs` — см. «Дневник — M6 GPUI»; ARK writes still blocked by Engine ingress, see Byte-compat) |
| Stickers (floating note windows) | `src/views/StickerNoteView.vue`, `src/lib/sticker.ts`, `src/composables/useDockedWidget.ts` | M7 | DONE (`src/sticker_route.rs`, `src/app/doc.rs`, `src/app/sticker.rs` — см. «Стикеры» ниже. GAP: `useDockedWidget`/host docking и runtime always-on-top — нет API в GPUI → **accept**) |
| Settings page + sections | `src/components/settings/{SettingsPage,GeneralSettings,ExportSettings,TrashSettings}.vue(+css)`, `src/views/EdenSettingsView.vue`, `src/composables/usePreferences.ts` | M4/M8 | PARTIAL — M4: `src/app/settings.rs` General prefs persist via `local_state.rs`, Trash works. Export section wires to the vault ops that landed in M8 (merged, PR #6: `src/obsidian/*`, `src/vault_ops.rs`) |
| Conflict banner | `src/components/EntryConflictBanner.vue` | M4 | DONE (M4: `src/app/conflict*.rs` — recheck/accept-remote/keep-copy/copy-local/cancel; `ui_tests::extra::conflict_banner_accept_remote`) |
| FPS monitor (dev overlay) | `src/composables/useFpsMonitor.ts` | — | GAP — **accept** (dev-only overlay; GPUI has `gpui::perf`/`debug_bounds` for ad-hoc profiling; reopen only if a perf regression needs an in-app meter) |
| Obsidian vault import/export (+ images, frontmatter, journal transaction) | `src/lib/obsidianVault{,Export,ExportAssets,ExportAssetPaths,ImportFrontmatter,ImportImages,ImportTransaction}.ts`, `src/lib/markdownFrontmatter.ts` | M8 | DONE (M8, merged PR #6: `src/obsidian/*` + `src/vault_ops.rs` — import/export, images, journaled transaction + recovery over Engine `filesystem.vault.*`; ported Vue suites + golden `vault-plan.json` byte-equal vs Vue `importObsidianVault`; UI wiring via `memoria-gpui --export` CLI + M8 shell screens) |
| Eden legacy migration (read-only: storage keys, userData, command prefixes) | `src/lib/memoria-migration.ts`, `manifest.json` `legacy_*` fields | M1 | DONE for shipped scope (M1: storage/userData/command-prefix ports + tests — `src/migration.rs`). GAP: `manifest.json` `legacy_*` fields are Vue-package contract — **accept** (`.kspkg` manifest stays with the Vue package; GPUI is a component, not a package) |
| Local image resolution | `src/lib/localImages.ts` | M3 | DONE (M3: `kosmos-local-image://` → path, lazy decode, broken-stub — `images.rs`) |
| App bootstrap / env types | `src/main.ts`, `src/vite-env.d.ts` | — | n/a — replaced by Rust app shell |
| Sidebar leftovers | `src/components/sidebar/types.ts` | — | не нужен: sidebar was removed; only a dead `types.ts` (3 lines) remains |

### «Редактор» — M2 core (`crates/memoria-editor-core`, чистый Rust, без GPUI)

Статус ядра по сравнению с `TiptapEditor.vue` (StarterKit + TaskList + EdenImage):

| Фича | Vue-эталон | Status |
|---|---|---|
| Буфер: rope + UTF-8/grapheme позиции, один курсор | ProseMirror doc+selection | PASS |
| Markdown как source of truth, parse с byte-офсетами | `markdownToTiptapDoc` (line parser) | PASS (pulldown-cmark + offsets) |
| Live Preview: скрытие маркеров вне курсора (Obsidian-правило) | нет в Vue (WYSIWYG Tiptap) | PASS |
| Таблицы (GFM), таск-листы, strikethrough, autolinks | tiptap tables нет; strikethrough есть | PASS (таблицы — моноширинная сетка `│`; GAP: выделение ячеек — **follow-up** при портировании block selection) |
| Команды: bold/italic/strike/code, H1–H3, списки, quote, code+lang, hr, link, image | StarterKit + TaskList | PASS |
| Checkbox toggle | TaskItem | PASS |
| Enter/Backspace/Tab/Shift+Tab в списках/цитатах/заголовках/code | PM keymap + Eden обработчики | PASS |
| Undo/redo: группировка по паузе и типу, восстановление выделения | prosemirror-history | PASS |
| Paste: plain/markdown-as-is, HTML→markdown | Tiptap paste | PASS (минимальный HTML; GAP: Word mso-list — **follow-up** при жалобах на Office-paste) |
| IME contract (EntityInputHandler-shaped, UTF-16) | браузерный IME | PASS |
| Char count 1:1 `charCount.ts` | `src/lib/charCount.ts` | PASS |
| Отрисовка GPUI, syntax highlighting, пиксели изображений | — | DONE (M3 — см. таблицу ниже) |

### «Редактор» — M3 GPUI слой (`crates/memoria-editor-gpui`)

| Фича | Vue-эталон | Status | Evidence |
|---|---|---|---|
| Ввод текста + EntityInputHandler | Tiptap contenteditable | PASS | `tests::typing_updates_markdown_and_marks_dirty` |
| IME marked text (UTF-16 ranges, candidate bounds) | браузерный IME | PASS | `tests::ime_mark_then_commit`, `ime_replaces_selection_utf16` |
| Live Preview рендер (заголовки, цитаты, код, hr, ссылки) | Tiptap WYSIWYG | PASS | `rows.rs`/`runs.rs`/`style.rs`, токен↔CSS карта в `DESIGN.md` |
| Курсор+blink, выделение, автоскролл, виртуализация рядов | contenteditable | PASS | `element.rs`/`paint.rs` (shaping только видимых рядов) |
| Мышь: click/drag/dbl/tri, скрытые маркеры → source offset | PM posAtCoords | PASS | `tests::click_on_hidden_marker_word_maps_source`, `mouse.rs` |
| Горячие клавиши по физическим клавишам (RU layout) | `useKeyboard.ts` `e.code` | PASS | `tests::hotkeys_russian_layout` (key_char=«и/л/я», key=ASCII) |
| `Ctrl+K Z` zen chord (700ms, `currentEntry` guard) + zoom `Ctrl +/-/0` (±0.1, 0.5–2.0) | `useKeyboard.ts` | PASS | `view.rs` bindings, `lifecycle.rs`; test выше. GAP: zoom не персистится между запусками — **follow-up** (предпочтение в `local_state.rs`) |
| Подсветка кода — 31 язык | `shikiHighlight.ts` SHIKI_LANGUAGES | PARTIAL | 25 via `gpui-component` registry + 6 direct grammars; per-grammar availability см. `languages.rs`, missing → plain (**accept** — осознанный GAP) |
| Пикер языка код-блока с поиском | `EdenCodeBlockTools` «Поиск языка...» | PARTIAL | `picker.rs` + `set_code_block_lang`; alias-нормализация + «Plain text» (`tests_regressions::picker_*`); без hover-тулбара (только `ctrl-shift-l`) — **accept** |
| Картинки: local path, lazy decode, placeholder/broken | `localImages.ts`/`objectImages.ts` | PASS | `images.rs`, `images::tests::decodes_local_image_scheme` |
| Плейсхолдер «Начните писать...», char counter | `Placeholder`, `charCount.ts` | PASS | `paint.rs` placeholder; статус-бар `char_count()` |
| Autosave 300ms debounce → SaveEntry | `edenStoreSaveActions.ts` | PASS | `tests::autosave_debounce_emits_single_event` + `undo_after_autosave`; stale-save race → `tests_regressions::stale_save_reply_keeps_dirty`; flush при переключении → `flush_before_switch_emits_once` |
| Live refresh без затирания ввода | `liveRefresh.ts` | PASS | `app::refresh::tests` (5 кейсов) + `set_markdown_is_the_live_refresh_path` |
| Undo-изоляция между заметками | PM history per-editor | PASS | `tests_regressions::note_switch_resets_undo_history` (`set_markdown` → `reset_history`) |
| Perf: 10k строк, frame time | — | NOT_RUN default | `tests::perf_ten_thousand_lines` (`--ignored`) — **accept** (manual perf check, не гейт) |

### «Дневник» — M6 GPUI (`src/diary/`, `src/app/diary*.rs`, `src/store/bubble_api.rs`)

| Фича | Vue-эталон | Status | Evidence |
|---|---|---|---|
| Модель: kinds (`plain`/`idea`/`task`/`highlight`), draft tags, sortKey, date/time occurrence | `bubbleDiaryModel.ts` | PASS | `tests/bubble_diary_model.rs` |
| Threads: `reply_to` links, roots newest-first / replies oldest-first, invalid links stay roots | `normalizeBubbleThreads` | PASS | `tests/bubble_diary_model.rs` |
| Legacy journal → bubbles (`system-type-journal` detection, `journal-*-N` sortKeys, `listAllEntries` source) | `createJournalBubblesFromEntry` | PASS | `tests/bubble_diary_model.rs`, `tests/bubble_ark_migrate.rs::migrate_diary_imports_and_deletes_legacy_dated_journals` |
| Local blob `{version, journalImported, bubbles}` — `JSON.stringify` key order, byte-identical | `encodeLocalBubblesStorage` | PASS | `tests/bubble_diary_golden.rs` + `fixtures/diary-bubbles.json` |
| ARK API: create/update/delete, `reply_to` links, `writeEntryTiptapDoc`, migration idempotence | `kepler-bubble-api.ts` | PASS vs `FakeArk` | `tests/bubble_ark_api.rs`; live Engine writes still blocked — see Byte-compat |
| Unknown tiptap nodes: rendered as text, preserved on non-text save | `BubbleTiptapRenderer` | PASS | `render_model.rs` + `kind_only_update_preserves_unknown_tiptap_nodes` |
| Composer: compact editor, submit on button/Ctrl+Enter, `#tags`, kind `plain` | `addDraftBubble` (Tiptap) | PASS | `ui_tests::diary::composer_*`, `tags_extract_and_render`; markdown→tiptap rules in editor `DESIGN.md` |
| Item card: kind dot+menu (overlay, Esc/backdrop dismiss), time label opens edit (autofocus), 2-step «Удалить», Esc cancels | `BubbleTimelineItem.vue` + shared `Dropdown` | PASS | `ui_tests::diary::{edit_flow_updates_bubble,delete_flow_removes_bubble,bubble_kind_menu_changes_kind}` |
| Reply form under last thread row; `Отмена`/`Ответить` | `BubbleTimelineItem.vue` | PASS | `ui_tests::diary::reply_in_thread` |
| Calendar: Monday weeks, newest-first days, counts, today ring, 40-day pad | `BubbleDiaryCalendarSidebar.vue` | PASS | `src/diary/calendar.rs` + `ui_tests::diary::calendar_day_jump` |
| Дата-jump scroll | `scrollIntoView({block:"center"})` | PARTIAL | GPUI `scroll_to_item` aligns differently — visual GAP, **accept** |
| Timeline virtualization (`virtualRows` measured heights) | Vue virtual list | PARTIAL | GPUI renders the full list; fine at diary scale — **follow-up** если лента вырастет (perf GAP) |
| Journal migration re-run on `journalEntries` prop change mid-session | Vue `watch` → `migrateJournalEntries` | PARTIAL | GPUI migrates once per `start_diary` via `listAllEntries` (unfiltered; re-navigation retries after a failed run); a dated journal entry created mid-session migrates on the next diary open — behavioral GAP, **accept** (self-heals on next diary open) |
| Midnight/focus label re-resolution (`labelNow` timer + `visibilitychange`) | `midnightTimer` + listeners | PARTIAL | GPUI refreshes `labelNow` on every diary render and on `Bubbles` replies; an idle app left open across midnight keeps stale labels until the next interaction — residual GAP, **accept** |
| Populated-diary visual check vs `reference/screens/diary-*` | host screenshots | NOT_RUN | Vue shots are empty-state only (ARK ingress rejects bubble props — Byte-compat ниже); GPUI verified via `ui_tests::diary`. **Follow-up**: снять populated shots когда Engine ingress отпустит bubble props |

### «Стикеры» — M7 (`src/sticker_route.rs`, `src/app/doc.rs`, `src/app/sticker.rs`)

| Behavior | Vue source | Status | Notes |
|---|---|---|---|
| `canOpenInSticker` — только `note_obj`/`book_obj` | `sticker.ts` | PASS | `sticker_route::can_open_in_sticker`; тест `sticker_route::tests::can_open_in_sticker_types` |
| `stickerWindowKeyFor` — raw `sticker:<id>` при host-safe id, иначе FNV-1a→base36 | `sticker.ts` | PASS | fnv считает по UTF-16 code unit (`charCodeAt(0)`-паритет: для astral char — high surrogate); `sticker_roundtrip_and_window_keys`, `long_and_odd_ids_get_fnv_tags` |
| `stickerRouteFor`/`parseStickerRoute` — `/sticker/<encoded>` | `sticker.ts` + call sites | PASS | `parse_sticker_route` — rejects: bad `%`-encoding, encoded `? # \`, whitespace, control chars, id>200 (decoded `/` легален); `#/sticker` normalisation — на call sites как в Vue. Тест `sticker_route::tests::sticker_routes` |
| Окно 380×480, min 260×200 | `sticker.ts` STICKER_WINDOW_* | PASS | `STICKER_WINDOW_SIZE`/`STICKER_MIN_SIZE` в `open_sticker` |
| Одно окно на ключ; reopen фокусирует существующее | `useDockedWidget` keyed windows | PASS | `Memoria::stickers` map `key → WindowHandle`; `ui_tests::sticker::sticker_reopen_focuses_existing_window` |
| Shared document: edit в стикере виден в основном окне и наоборот | одна `entry` в store | PASS | `NoteDoc` (`app/doc.rs`) — один `MemoriaEditor`+entry snapshot на id; title — canonical string + per-window `InputState` (shared `InputState` ping-pong-ит `cx.notify` между окнами) |
| Сохранение через общий backend, 1 save на autosave-цикл | `edenStoreSaveActions` | PASS | `DocEvent::Save` → `Backend::send`; `Reply::Saved` несёт entry id → routing к нужному doc; `ui_tests::sticker::sticker_window_shares_document_and_dedupes_save` |
| Компактный вид: мини-титлбар, без сайдбара, title editable | `StickerNoteView.vue` | PASS | titlebar pin/close, `titlebar-open-sticker` button в основном окне (zen/route/`can_open_in_sticker`-gated) |
| Entry points: titlebar + context menu «Открыть стикером» | `NoteView`/`EntryListItem` | PASS | `chrome.rs` button + `menus.rs` ctx item |
| `useDockedWidget` — host `kepler.window.open` docking | `useDockedWidget.ts` | GAP — **accept** | kosmos host недоступен в GPUI shell; `WindowKind::Floating` — ближайший нативный аналог |
| Runtime always-on-top toggle (pin button) | `setAlwaysOnTop` host API | GAP — **accept** | нет runtime level API в GPUI; pin — индикатор. На Wayland always-on-top для обычных окон вообще не гарантируется композитором |
| In-sticker navigation (по ссылкам в редакторе) | `StickerNoteView` `handleNavigate` | GAP — **follow-up** | editor-level gap (нет `Navigate` event в `MemoriaEditor`), не sticker-specific — link navigation в редакторе в целом |
| Window position persistence | — | n/a | в Vue нет — out of scope |

### «Упаковка» — M9 (KOS-156)

Product contour mirrors KOS-137/agenda-gpui:

| Item | Status | Notes |
|---|---|---|
| `build.rs` Windows resources (icon + VERSIONINFO) | DONE | Icon/VERSIONINFO via `embed-resource`; **no** RT_MANIFEST id=1 (gpui-pre already embeds Common Controls v6 + PerMonitorV2 — duplicate → CVT1100). `windows/app.manifest` kept in-tree as docs. `KOSMOS_MEMORIA_VERSION` stamps FileVersion/ProductVersion (cortex sets desktop release version; fallback = `Cargo.toml`) |
| Release tooling (`scripts/release.py`, `publish-version.sh`, `test_release.py`) | DONE | same rules as agenda-gpui; `python scripts/test_release.py` green |
| Nightly workflow `.github/workflows/build.yml` | DONE | ubuntu/windows/macos matrix → `dist/` artifacts + `SHA256SUMS.txt` (`collect` job); **no GitHub Release** — publish held for Jack's default-launch decision |
| cortex component pin `memoria_gpui` | DONE | `desktop/component-pins.json`, verified by `build-package-components.mjs` |
| Packaged path `resources/components/memoria/Kosmos Memoria.exe` | DONE | staged next to `components/{manager,agenda}`; Start Menu shortcut + launcher command + Manager About entry; all paths pin `KOSMOS_DATA_DIR` → shared `engine.lock.json` |
| Vue Memoria `.kspkg` fallback | Kept | `com.kosmos.memoria` package remains installable; GPUI is an additional component, not a replacement |
| Default launch switch (Vue → GPUI) | NOT_DONE | Jack decision — GPUI is opt-in via the three launch paths above |
| Windows installed smoke (product) | NOT_RUN | env-blocked: no Windows host on this box — Jack smokes the epic once at the end |

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
         <(grep -oE 'src/[A-Za-z0-9_./-]+' /workspace/wt/memoria-gpui-kos-156/PARITY.md | sed 's|src/||;s|/$||' | sort -u)
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
| `src/App.css` | M4 | DONE | tokens ported via imago-gpui (`src/theme.rs` re-export), no hex in src |
| `src/App.vue` | M4 | DONE | root shell: `src/app/{chrome,render,nav}.rs`, screens via `Route` |
| `src/Titlebar.css` | M4 | DONE | imago tokens |
| `src/Titlebar.vue` | M4 | DONE | `src/app/chrome.rs` |
| `src/components/EntryConflictBanner.vue` | M4 | DONE | `src/app/conflict_banner.rs` |
| `src/components/SearchOverlay.vue` | M4 | DONE | `src/app/search.rs` |
| `src/components/books/BookCover.vue` | M5 | DONE | `src/app/book.rs`/`cover_modal.rs` |
| `src/components/books/BookCoverFileDropzone.vue` | M5 | DONE | `src/app/cover_modal.rs` |
| `src/components/books/BookMetadataImportModal.vue` | M5 | DONE | `src/app/metadata_modal.rs`/`metadata_apply.rs` |
| `src/components/bubbles/BubbleDiaryCalendarSidebar.vue` | M6 | DONE (M6) | `src/app/diary_calendar.rs` + `src/diary/calendar.rs` |
| `src/components/bubbles/BubbleDiaryView.vue` | M6 | DONE (M6) | `src/app/diary.rs` (view/composer/thread orchestration) |
| `src/components/bubbles/BubbleTimelineItem.vue` | M6 | DONE (M6) | `src/app/diary_item.rs` + `src/app/diary_forms.rs` |
| `src/components/bubbles/BubbleTiptapRenderer.vue` | M6 | DONE (M6) | `src/diary/render_model.rs` + `src/app/diary_blocks.rs` |
| `src/components/bubbles/bubbleDiaryModel.ts` | M6 | DONE (M6) | `src/diary/{text,timeline,calendar,journal,storage}.rs` |
| `src/components/everything/EverythingItemCard.vue` | M4 | DONE | `src/app/everything.rs` card (icon/title/preview/pin/ctx menu) |
| `src/components/everything/EverythingView.vue` | M4 | DONE | `src/app/everything.rs` grid + `create_entry` |
| `src/components/objects/ImageObjectView.vue` | M4/M5 | DONE | `src/app/image.rs`/`object_views.rs`; gallery extended in M5 |
| `src/components/objects/TypeObjectsView.vue` | M4/M5 | DONE | `src/app/objects.rs`/`object_views.rs`; columns extended in M5 |
| `src/components/settings/ExportSettings.vue` | M8 | GAP | UI stub — vault export itself shipped in M8 (`src/obsidian/*`, `memoria-gpui --export`); wiring the buttons to it is a **follow-up** |
| `src/components/settings/GeneralSettings.vue` | M4 | DONE | `src/app/settings.rs` + `local_state.rs` prefs |
| `src/components/settings/SettingsPage.css` | M4 | DONE | imago tokens |
| `src/components/settings/SettingsPage.vue` | M4 | DONE | `src/app/settings.rs` + `settings_widgets.rs` |
| `src/components/settings/TrashSettings.vue` | M4 | DONE | `src/app/trash.rs` |
| `src/components/sidebar/types.ts` | — | skip | не нужен — dead leftover after sidebar removal; no runtime user |
| `src/components/typed-notes/ObjectPropertyField.vue` | M5 | DONE | `src/app/prop_field.rs` + `prop_edit.rs` — all field kinds |
| `src/components/typed-notes/ObjectPropertyPicker.vue` | M5 | DONE | `src/app/prop_picker.rs` — anchored, viewport-clamped, single/multi |
| `src/components/typed-notes/TypedHeader.vue` | M5 | DONE | `src/app/typed_header.rs` — hero/cover, featured+secondary table |
| `src/composables/useBlockSelection.ts` | M3 | GAP | **follow-up** — see feature matrix |
| `src/composables/useCharCounter.ts` | M1 | DONE | `src/char_count.rs` |
| `src/composables/useDockedWidget.ts` | M7 | GAP | **accept** — host `kepler.window.open` docking n/a in GPUI (M7 notes) |
| `src/composables/useFpsMonitor.ts` | — | GAP | **accept** — dev overlay; use gpui perf tools instead |
| `src/composables/useKeyboard.ts` | M3/M4 | DONE | physical-key bindings in `editor_host.rs`/editor `view.rs` |
| `src/composables/useNavigationHistory.ts` | M4 | DONE | `src/nav_history.rs` |
| `src/composables/usePlatform.ts` | M4 | DONE | n/a — `cfg!(target_os)` at call sites (**accept**) |
| `src/composables/usePreferences.ts` | M4 | DONE | `src/local_state.rs` (`memoria-settings.json`) |
| `src/composables/useSearch.ts` | M4 | DONE | `src/search_model.rs` |
| `src/composables/useTheme.ts` | M4 | GAP | **follow-up** — runtime light/dark toggle needs imago light palette |
| `src/editor-content/content.ts` | M1 | DONE (M1) | `src/content/` codecs |
| `src/editor-tiptap/TiptapEditor.vue` | M2/M3 | DONE | `crates/memoria-editor-{core,gpui}` — см. «Редактор» таблицы |
| `src/editor-tiptap/shikiHighlight.ts` | M3 | PARTIAL | 25+6 grammars via `gpui-component` + direct; missing → plain (**accept**, осознанный GAP в «Редактор — M3») |
| `src/index.css` | M4 | DONE | theme tokens only |
| `src/lib/blockSelectionClasses.ts` | M3 | GAP | **follow-up** — with `useBlockSelection` |
| `src/lib/blockSelectionPointer.ts` | M3 | GAP | **follow-up** — with `useBlockSelection` |
| `src/lib/bookLanguages.ts` | M5 | DONE | `src/book_languages.rs` (M1) |
| `src/lib/bookMetadata.ts` | M5 | DONE | `src/book_metadata*.rs` — Engine `bookMetadata.*` |
| `src/lib/charCount.ts` | M1 | DONE (M1) | `src/char_count.rs` |
| `src/lib/edenApi.ts` | M1 | GAP | **accept** — `window.api` host shim; GPUI talks Engine directly |
| `src/lib/entryTitles.ts` | M1 | DONE (M1) | `src/entry_titles.rs` |
| `src/lib/iconResolver.ts` | M4 | DONE (M1) | `src/icon_resolver.rs` |
| `src/lib/kepler-api-shim.ts` | M1 | GAP | **accept** — `ark.request` transport = `src/store/transport*` |
| `src/lib/kepler-bubble-api.ts` | M6 | DONE (M6) | `src/store/bubble_api.rs` over `ArkBridge` |
| `src/lib/kepler-command-bus.ts` | M1 | DONE (M1) | `src/command_bus.rs` |
| `src/lib/kepler-entry-api.ts` | M1 | DONE (M1) | `src/store/entry_api.rs` |
| `src/lib/kepler-entry-mappers.ts` | M1 | DONE (M1) | `src/mapping*.rs` |
| `src/lib/kepler-folder-stubs.ts` | M1 | GAP | **accept** — stub-only surface in Vue; no folder API to port |
| `src/lib/kepler-navigation.ts` | M4 | DONE | `src/nav_history.rs`/`src/app/nav.rs` |
| `src/lib/kepler-note-type-api.ts` | M1 | DONE (M1) | `src/store/note_type_api.rs` |
| `src/lib/kepler-task-sync.ts` | M8 | DONE | `src/task_sync.rs` |
| `src/lib/kepler-trash-storage.ts` | M1 | DONE (M1) | `src/store/trash_api.rs` |
| `src/lib/kepler-ui-runtime.ts` | M1 | GAP | **accept** — host runtime shim, n/a in GPUI |
| `src/lib/localImages.ts` | M3 | DONE | `crates/memoria-editor-gpui/src/images.rs` |
| `src/lib/markdownFrontmatter.ts` | M8 | DONE (M1) | `src/frontmatter.rs` |
| `src/lib/memoria-migration.ts` | M1 | DONE (M1) | `src/migration.rs` |
| `src/lib/objectFieldFormatting.ts` | M5 | DONE | `src/object_fields.rs` |
| `src/lib/objectImages.ts` | M4/M5 | DONE | `src/object_views.rs`/`src/image_src.rs`/`object_images.rs` |
| `src/lib/obsidianVault.ts` | M8 | DONE | `src/obsidian/vault.rs` |
| `src/lib/obsidianVaultExport.ts` | M8 | DONE | `src/obsidian/run.rs` |
| `src/lib/obsidianVaultExportAssetPaths.ts` | M8 | DONE | `src/obsidian/export_paths.rs` |
| `src/lib/obsidianVaultExportAssets.ts` | M8 | DONE | `src/obsidian/export_assets.rs` |
| `src/lib/obsidianVaultImportFrontmatter.ts` | M8 | DONE | `src/obsidian/frontmatter.rs` |
| `src/lib/obsidianVaultImportImages.ts` | M8 | DONE | `src/obsidian/images.rs` + `image_index.rs` |
| `src/lib/obsidianVaultImportTransaction.ts` | M8 | DONE | `src/obsidian/transaction.rs` + `journal_recovery.rs` |
| `src/lib/saveResult.ts` | M1 | DONE (M1) | `src/save_result.rs` |
| `src/lib/sticker.ts` | M7 | DONE | `src/sticker_route.rs` |
| `src/lib/systemTypeDefinitions.ts` | M1 | DONE (M1) | `src/system_types*.rs` |
| `src/lib/systemTypeGameDefinitions.ts` | M5 | DONE | `src/system_types_data.rs` game schema + game UI |
| `src/lib/systemTypeVisualDefinitions.ts` | M5 | DONE | `src/system_types_data.rs` visual schemas + object UIs |
| `src/lib/systemTypes.ts` | M1 | DONE (M1) | `src/system_types.rs` |
| `src/lib/taskStatus.ts` | M8 | DONE | `src/task_status.rs` |
| `src/lib/typedNoteHeaderProps.ts` | M1/M5 | DONE | `src/header_props.rs` (M1); header UI — M5 |
| `src/lib/typedNoteSchemas.ts` | M1 | DONE (M1) | `src/note_type_schemas.rs` |
| `src/lib/typedNotes.ts` | M1 | DONE (M1) | `src/note_types.rs` |
| `src/main.ts` | — | DONE | replaced by Rust `src/main.rs` bootstrap |
| `src/store/eden.ts` | M1 | DONE | shipped scope: `src/store.rs` + `src/app.rs` state; M5 residues landed in M5 |
| `src/store/edenEntryFactory.ts` | M1 | DONE | `src/mapping.rs`/`entry_api.rs` entry construction |
| `src/store/edenLiveRefreshSubscription.ts` | M1 | DONE | `src/store/transport/events.rs` WS subscription |
| `src/store/edenStoreDataActions.ts` | M1 | DONE | `src/store/entry_api.rs` |
| `src/store/edenStoreDraftActions.ts` | M1 | DONE | dirty-tracking via `src/entry_changes.rs` + `app.rs` `dirty` flag |
| `src/store/edenStoreHelpers.ts` | M1 | DONE | subsumed by `entry_api`/`mapping` helpers |
| `src/store/edenStoreNoteTypeActions.ts` | M1/M5 | DONE | `src/store/note_type_api.rs`; object actions — M5 |
| `src/store/edenStoreSaveActions.ts` | M1 | DONE (M1) | `src/save_actions.rs` coordinator |
| `src/store/edenStoreSystemTypeActions.ts` | M1/M5 | DONE | `src/store/note_type_api.rs`; game/visual — M5 |
| `src/store/entryChanges.ts` | M1 | DONE (M1) | `src/entry_changes.rs` |
| `src/store/entryConflicts.ts` | M1 | DONE (M1) | `src/entry_conflicts.rs` + `conflict_store.rs` |
| `src/store/layout.ts` | M4 | DONE | zen flag + `sidebar_collapsed` in `local_state.rs` |
| `src/store/liveListFilter.ts` | M1 | DONE (M1) | `src/live_list_filter.rs` |
| `src/store/liveRefresh.ts` | M1 | DONE | `src/live_refresh.rs` + `src/app/refresh.rs` wiring |
| `src/views/EdenSettingsView.vue` | M4 | DONE | `src/app/settings.rs` |
| `src/views/StickerNoteView.vue` | M7 | DONE | `src/app/sticker.rs` |
| `src/vite-env.d.ts` | — | skip | vite ambient types — n/a for Rust |


## Known deliberate divergences (post-commit M1 parity review)

Reviewed against Vue `7ccbb9f`. Intentional, documented tolerances — revisit
when the corresponding flows get wired in M2+:

- **Malformed Engine data degrades instead of throwing.** Vue propagates
  `JSON.parse`/schema/zod throws out of API calls (whole op rejects); the
  Rust port drops malformed records per-item or applies documented defaults
  at tolerant sites (`listEntries` summaries, `related_notes` arrays,
  `SearchResult` decode, `getVaultStorageInfo` titles, `entry_header_layout`
  fallback). Wired-path throw sites that DO match Vue: save-path header-prop
  schema parse → `EngineError::Malformed`; malformed legacy note types →
  dropped/`None` (no phantom `NoteType`).
- **`Date.parse` residual edges.** Numbers/numeric strings now match JS
  (`NaN` → fallback); day/hour/minute/second/offset ranges and trailing junk
  are enforced. Zone-less date-times are rejected (JS treats them as *local*
  time — the port has no TZ source); `YYYY-MM` shorthand is not accepted.
- **Case handling.** `to_lowercase()` ≈ `toLocaleLowerCase("ru")` for
  Cyrillic; `ẞ`/`İ`-class edge cases diverge. `char::is_whitespace` ≠ JS
  `trim` for `\uFEFF`.
- **Command bus.** Per-handler panics are caught like Vue's per-handler
  try/catch; queue flush is synchronous inside `subscribe` (Vue uses
  `queueMicrotask`); the bus is an owned instance, not a process global.
- **Conflict persistence.** Only the `ConflictStore` bridge surface exists —
  Vue additionally mirrors into `localStorage["memoria.entry-conflicts.v1"]`
  as a crash-recovery checkpoint (`merge_entry_conflict_snapshots` is
  available for a future second surface). `resolve_with_state` covers the
  `merged` close.
- **Conflict snapshot numbers.** Integral floats (`1.0`) accepted for
  `version`/revisions/`detectedAt`/entry timestamps; non-integral floats are
  still rejected (JS would keep them — nonsense data either way).
- **Non-integral revision/timestamp fields** in `Entry` (`i64`) — JS would
  carry `1.5`; the port drops them at parse.
- **No-async shapes.** `Promise.all` call groups are sequential RPCs —
  semantics identical, latency differs.
