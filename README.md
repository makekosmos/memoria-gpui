# memoria-gpui

Experimental native GPUI clone of Memoria (KOS-146). Not a replacement for the
Vue/Kosmos package Memoria.

Source of truth: `makekosmos/memoria` @ `7ccbb9f89841fc93c8bcb48aef67f3df2d3df84e` (0.6.9).
Feature parity matrix: [`PARITY.md`](PARITY.md). Test-port plan: [`PORT_TESTS.md`](PORT_TESTS.md).
Seeded ARK snapshot + reference screenshots: [`fixtures/`](fixtures/),
[`reference/screens/`](reference/screens/).

## Требования

- Rust stable
- `hk` для Git hooks: `cargo install hk --locked`, затем `hk install` в корне репозитория
- Гейты pre-push: `cargo nextest`, `cargo shear`, `cargo clippy`, `cargo deny` (ставятся через `cargo install` по необходимости)
- Linux: обычный `cargo run`
- `pkg-config` + `libfontconfig-dev` (Linux; для gpui text stack)

Весь гейт можно прогнать вручную: `hk check --all` или `hk run pre-push`.

## Сборка

Обычный запуск читает заметки через локальный Kosmos Engine API v1.
Сначала запусти Kosmos Engine с той же папкой данных, что использует
Vue Memoria. Приложение читает `engine.lock.json` из `%APPDATA%\Kosmos`
(Windows), `~/Library/Application Support/Kosmos` (macOS) или
`$XDG_CONFIG_HOME/Kosmos` / `~/.config/Kosmos` (Linux) и вызывает
`http://127.0.0.1:<port>/v1/rpc` с bearer-токеном из lock-файла.
`KOSMOS_DATA_DIR` переопределяет папку данных.

`MEMORIA_OFFSCREEN=1` паркует окно за пределами экрана (для headless-запусков).

## Статус

M0: scaffold + parity matrix + fixtures + reference screenshots. UI-реализация
начинается с M1 — см. PARITY.md.

M2 (ветка `kos-149`): `crates/memoria-editor-core` — ядро редактора без GPUI:
rope-буфер, pulldown-cmark парсер с офсетами, Live Preview проекция
(скрытие маркеров по Obsidian-правилу + маппинг visible↔source), команды
StarterKit/TaskList, undo/redo с группировкой, paste (HTML→markdown),
IME-контракт формы `EntityInputHandler`, char count 1:1 с `charCount.ts`.
См. `crates/memoria-editor-core/DESIGN.md`.

### Editor-core gates

    cargo nextest run -p memoria-editor-core
    cargo bench -p memoria-editor-core   # criterion: open 10k lines, type, reproject

Фаззинг парсера и проекции (нужен `cargo fuzz`; в этом окружении —
`cargo fuzz` недоступен, статус NOT_RUN):

    cd crates/memoria-editor-core
    cargo +nightly fuzz run parse_project -- -max_total_time=600

