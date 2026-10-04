# memoria-gpui: agent instructions

## Scope and entry points

Experimental native GPUI clone of the Memoria notes/diary app (KOS-146).
Reads and writes notes through the local Mundus Engine API — the app never
opens the ARK database itself (write boundary: cortex `docs/write-boundary.md`).

- Root binary crate `memoria-gpui`: GPUI app, windows, views (`src/app/`),
  theme glue, a11y + UI autotests (`src/ui_tests/`), `src/bin/` helper bins
  (incl. `check-source-size`).
- `crates/memoria-model`: pure data layer — Engine transport, ARK object
  models, content codec. Split out (KOS-328) so a UI edit does not rebuild
  the model; it must stay free of gpui dependencies.
- `crates/memoria-editor-gpui`, `crates/memoria-editor-core`: editor widget
  and its platform-independent core.
- `hk.pkl`: the local gate (pre-commit + pre-push). `.github/workflows/`:
  CI + nightly release — keep them working; this is a public repo.

## Data access contract

- The app locates the Engine via `engine.lock.json` in the Mundus data dir
  (`MUNDUS_DATA_DIR` override; legacy `KOSMOS_DATA_DIR`/`Kosmos` paths are
  transitional fallbacks) and calls `http://127.0.0.1:<port>/v1/rpc` with the
  bearer token from the lock file.
- All persistence goes through Engine ops; never write SQLite directly.
- `MEMORIA_OFFSCREEN=1` parks the window off-screen for headless runs.

## Setup and verification

```powershell
cargo install hk --locked; hk install   # once per checkout
hk run pre-push                          # or: hk check --all
```

Gate = `cargo nextest run --workspace --all-features`, `cargo shear`,
`cargo clippy` (with the allow-list in `hk.pkl`), `cargo deny check
advisories bans sources`, `cargo run -q --bin check-source-size`.
Pre-commit runs `cargo check` + `cargo fmt --check` on touched Rust files.
Install `cargo-nextest`, `cargo-shear`, `cargo-deny` on demand.
Linux also needs `pkg-config` + `libfontconfig-dev`.

## Contracts to preserve

- `gpui` (aliased `gpui-kit`), `gpui-component`, `gpui-base` are exact-pinned
  and must stay identical to `agenda-gpui` and `cortex/manager-gpui` — two
  gpui versions in one build are type errors. Bump them together.
- `imago-gpui` is pinned by git rev to `makekosmos/imago`; the
  `[patch.crates-io]` `gpui-pre{,-windows}` pins follow the shared patched
  gpui revision (KOS-328). One imago rev across all consumers.
- Parity targets live in `PARITY.md`; test-port plan in `PORT_TESTS.md`;
  seeded ARK snapshot in `fixtures/`, reference screenshots in
  `reference/screens/`. Update `PARITY.md` when behavior lands.
- UI tests use `#[gpui::test]` + `TestAppContext` behind gpui's
  `test-support` feature (dev-dependency only).
- Preserve keyboard access, focus behavior, accessible names
  (`src/a11y.rs`), and theme tokens — no hardcoded hex/fonts.
- Clean up listeners, timers and subscriptions on disposal.

## Completion

- One logical change per commit; no drive-by refactors.
- `cargo fmt` before committing; run the targeted tests for crates you
  touched, then `hk run pre-push` before pushing.
- Report exact commands and PASS / FAIL / NOT_RUN with reasons; for UI
  changes state whether you verified visually.
- Do not bump `[package].version`, tag, or publish releases unless
  authorized (releases are automated via `scripts/release.py` /
  `publish-version.sh` + the Build workflow).
