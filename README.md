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

## CI и ночные сборки

GitHub Actions проверяет форматирование, правила версий, Clippy и размер файлов
(Windows), тесты и release-сборки при push в `main` и в pull request
(`ci.yml` + `build.yml`). Архивы сборок доступны в Artifacts каждого успешного
запуска в течение 7 дней.

Каждый день в **00:00 МСК** (`21:00 UTC`) workflow `Build and package` собирает
артефакты для всех платформ. Ручной запуск: Actions → Build and package →
Run workflow на основной ветке. `scripts/release.py` берёт версию из
`Cargo.toml` (правила `X.Y.Z` — те же, что у agenda-gpui; проверка локально:
`python scripts/test_release.py`, Python 3.11+).

Платформы: **Windows x86_64** (ZIP с EXE + VERSIONINFO/иконка через `build.rs`),
**Linux x86_64** (tar.gz, сборка на Ubuntu 24.04) и **macOS Apple Silicon**
(tar.gz с `Memoria.app`, ad-hoc подпись без notarization). `collect`-job
складывает все архивы вместе с `SHA256SUMS.txt`. GitHub Release не создаётся —
публикация отложена до решения по умолчанию (см. KOS-156).

## Упаковка в состав Kosmos (KOS-156)

Transitional Windows-инсталлер Kosmos зашивает этот бинарь как компонент
`resources/components/memoria/Kosmos Memoria.exe` рядом с `components/manager`
(GPUI Manager) и `components/agenda` (KOS-137). Сборкой управляет
`cortex/desktop/scripts/build-package-components.mjs`: он берёт checkout этого
репозитория из `KOSMOS_MEMORIA_GPUI_SRC` (или sibling `../memoria-gpui`),
проверяет `git rev-parse HEAD` по пину `desktop/component-pins.json` и собирает
`cargo build --locked --release --target x86_64-pc-windows-msvc` с
`KOSMOS_MEMORIA_VERSION=<win-версия релиза>` — build.rs штампует VERSIONINFO
этой версией (без env — версия из Cargo.toml).

Запуск из установленного продукта: launcher-команда «Открыть Memoria (GPUI)»,
ярлык Start Menu «Kosmos Memoria» и кнопка «Открыть Memoria» в GPUI Manager —
все три пути резолвят один exe и используют общий `KOSMOS_DATA_DIR`
(`%APPDATA%\Kosmos` в prod), поэтому Memoria GPUI читает тот же
`engine.lock.json`, что и Manager. Vue Memoria (`com.kosmos.memoria` .kspkg)
остаётся fallback и не удаляется.

Linux → MSVC evidence-build (не для публикации):

```bash
cargo xwin build --release --locked --target x86_64-pc-windows-msvc
GPUI_FXC_PATH=/path/to/fxc scripts/compile-shaders-xwin.sh
cargo xwin build --release --locked --target x86_64-pc-windows-msvc
```

## Статус

M1–M8 merged в `main` (shell, editor, typed objects, diary, stickers,
Obsidian import/export + task sync); M9 добавляет упаковку в продукт —
см. PARITY.md (финальный аудит) и раздел «Поставка» выше.

`crates/memoria-editor-core` — ядро редактора без GPUI:
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

