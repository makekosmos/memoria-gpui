# PORT_TESTS.md — Vue test inventory → GPUI port plan

Source: `makekosmos/memoria` @ `7ccbb9f89841fc93c8bcb48aef67f3df2d3df84e`,
`tests/`. Runners: `vitest` unit (`*.test.ts`, `vitest.unit.config.ts`) and
browser component specs (`tests/components/*.spec.ts`, playwright pool).

For the GPUI port these split into:
- **port** — logic moves to Rust (`store/`, `editor-content/`, `lib/*`):
  re-express as `cargo nextest` cases over `fixtures/ark-snapshot.json`.
- **reference** — DOM/Vue-specific; keep the Vue spec as behavioral
  documentation and cover the equivalent state transitions in Rust model
  tests or e2e screenshots.

## Unit tests (`tests/*.test.ts`, `scripts/*.test.mjs`)

| Test file | Subject | Port to | Plan |
|---|---|---|---|
| `tests/content.test.ts` | `editor-content/content.ts` tiptap/markdown/legacy codecs | M1 | DONE → `tests/content_codec.rs`, `tests/codec_props.rs` (proptest), `tests/golden_roundtrip.rs` |
| `tests/components/ContentAdapter.spec.ts` | content adapter | M1 | port (logic) |
| `tests/charCount.test.ts` | `lib/charCount.ts` | M1 | DONE → `tests/char_count.rs` |
| `tests/components/CharCounter.spec.ts` | char counter UI | M1 | reference |
| `tests/components/BlockSelectionClasses.spec.ts` | block selection classes | M1 | port |
| `tests/components/BlockSelectionPointer.spec.ts` | pointer hit-testing | M1 | reference (GPUI hit model differs) |
| `tests/components/BlockSelectionAutoScroll.spec.ts` | autoscroll during selection | M1 | reference |
| `tests/components/TiptapEditor.spec.ts` | editor component | M1 | reference — port key editing behaviors to editor tests |
| `tests/bubbleArkApi.test.ts` | `kepler-bubble-api` CRUD/thread links | M6 | port against fixture snapshot |
| `tests/bubbleDiaryModel.test.ts` | `bubbleDiaryModel` timeline/tags/kind | M6 | port |
| `tests/components/BubbleDiaryView.spec.ts` | diary view | M6 | reference |
| `tests/bookLanguages.test.ts` | `lib/bookLanguages` | M5 | port (static table) |
| `tests/components/BookMetadataImportModal.spec.ts` | book metadata import modal | M5 | reference + port `bookMetadata` parsing |
| `tests/systemTypes.test.ts` | `systemTypes`, `iconResolver` | M4 | DONE → `tests/system_types.rs` |
| `tests/components/EntryTitle.spec.ts` | `entryTitles` | M4 | port |
| `tests/components/JournalTitleReadonly.spec.ts` | journal title rules | M4 | port |
| `tests/components/EverythingView.spec.ts` | home grid filtering | M2/M4 | reference + store-level port |
| `tests/entryChanges.test.ts` | `store/entryChanges` | M3 | DONE → `tests/entry_changes.rs` |
| `tests/entryConflicts.test.ts` | `store/entryConflicts` | M3 | DONE → `tests/entry_conflicts.rs` |
| `tests/saveConflict.test.ts` | `edenStoreSaveActions` conflict path | M3 | DONE → `tests/save_actions.rs` (synchronous coordinator port; the Promise-queue ordering cases are covered by sequenced-call tests) |
| `tests/liveRefresh.test.ts` | `store/liveRefresh` | M3 | DONE → `tests/live_refresh.rs` |
| `tests/liveListFilter.test.ts` | `store/liveListFilter` | M3 | DONE → `tests/live_list_filter.rs` |
| `tests/components/EdenLiveRefreshSubscription.spec.ts` | ARK subscription wiring | M3 | port (engine subscribe → store) |
| `tests/components/EdenStoreNavigation.spec.ts` | store navigation state | M2/M3 | port |
| `tests/components/EdenStoreRefreshRace.spec.ts` | refresh race handling | M3 | port |
| `tests/components/ConflictFlow.spec.ts` | conflict flow end-to-end | M3/M8 | reference + store-level port |
| `tests/components/EntryConflictBanner.spec.ts` | conflict banner UI | M8 | reference |
| `tests/components/keplerApiShim.spec.ts` | `kepler-api-shim` behavior | M3 | port — engine transport contract tests |
| `tests/components/KeplerNavigation.spec.ts` | `kepler-navigation` hash routes | M2/M3 | port — route table |
| `tests/stickerRoutes.test.ts` | sticker routes | M7 | port |
| `tests/components/StickerNoteView.spec.ts` | sticker window view | M7 | reference |
| `tests/preferences.test.ts` | `usePreferences` persistence | M8 | port |
| `tests/components/GeneralSettings.spec.ts` | general settings | M8 | reference |
| `tests/components/ExportSettings.spec.ts` | export settings | M8/M9 | reference + port export params |
| `tests/fpsMonitor.test.ts` | `useFpsMonitor` | M8 | port or drop — dev overlay only |
| `tests/obsidianVault.test.ts` | vault scan/import core | M9 | port |
| `tests/obsidianVault.export.test.ts` | vault export | M9 | port |
| `tests/obsidianVaultImportTransaction.test.ts` | import journal/rollback | M9 | port |
| `tests/obsidianVault.helpers.ts` | (helper, not a test) | M9 | — |
| `tests/memoriaMigration.test.ts` | `memoria-migration` legacy keys | M9 | DONE → `tests/migration.rs` + `tests/command_bus.rs` + `tests/ark_api.rs` (fake `ArkBridge`); manifest.json/compatibility.json assertions are Vue-package-only — N/A |
| `tests/packageContract.test.ts` | manifest/package contract | repo | keep — re-point at GPUI packaging metadata |
| `tests/workspaceDependencies.test.ts` | workspace dep audit | repo | n/a — repo-specific |
| `scripts/package-manager.test.mjs` | package manager pin | repo | n/a — repo-specific |
| `scripts/package-prepared.test.mjs` | kspkg packaging | repo | n/a for GPUI (native packaging TBD) |

## Cross-cutting

- `tests/components/editor-test-helpers.ts` — test fixture builders; port to a
  Rust `test-support` module backed by `fixtures/ark-snapshot.json`.
- Host-level e2e in cortex (`host/e2e/first-party-memoria-*.spec.ts`) is out of
  scope for this repo but remains the upstream contract to keep green while
  the Vue package stays the shipping app.
