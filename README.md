# memoria-gpui

Experimental native GPUI clone of Memoria (KOS-146). Not a replacement for the
Vue/Mundus package Memoria.

Source of truth: `makekosmos/memoria` @ `7ccbb9f89841fc93c8bcb48aef67f3df2d3df84e` (0.6.9).
Feature parity matrix: [`PARITY.md`](PARITY.md). Test-port plan: [`PORT_TESTS.md`](PORT_TESTS.md).
Модули, которые есть, но пока не подключены к UI: [`docs/unwired-modules.md`](docs/unwired-modules.md).
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

Обычный запуск читает заметки через локальный Mundus Engine API v1.
Сначала запусти Mundus Engine с той же папкой данных, что использует
Vue Memoria. Приложение читает `engine.lock.json` из `%APPDATA%\Mundus`
(Windows), `~/Library/Application Support/Mundus` (macOS) или
`$XDG_CONFIG_HOME/Mundus` / `~/.config/Mundus` (Linux) и вызывает
`http://127.0.0.1:<port>/v1/rpc` с bearer-токеном из lock-файла.
`MUNDUS_DATA_DIR` переопределяет папку данных. В переходный период также
проверяются `KOSMOS_DATA_DIR` и `<config>/Kosmos` — первый путь с
`engine.lock.json` побеждает.

`MEMORIA_OFFSCREEN=1` паркует окно за пределами экрана (для headless-запусков).

Остальные переменные окружения:

- `MEMORIA_DEMO=1` — запуск без Engine на встроенном in-memory наборе данных (то же, что используют UI-тесты).
- `MEMORIA_IMAGES_DIR` — корень для относительных путей картинок в редакторе.

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
(tar.gz с `Memoria.app`, ad-hoc подпись без notarization). `publish`-job
(только cron/dispatch на main) коммитит версию, ставит тег `vX.Y.Z` и публикует
GitHub Release со всеми архивами и `SHA256SUMS.txt` — оттуда Memoria ставит
магазин Mundus (KOS-265).

## Упаковка в состав Mundus (KOS-156)

Transitional Windows-инсталлер Mundus зашивает этот бинарь как компонент
`resources/components/memoria/Mundus Memoria.exe` рядом с `components/manager`
(GPUI Manager) и `components/agenda` (KOS-137). Сборкой управляет
`cortex/desktop/scripts/build-package-components.mjs`: он берёт checkout этого
репозитория из `KOSMOS_MEMORIA_GPUI_SRC` (или sibling `../memoria-gpui`),
проверяет `git rev-parse HEAD` по пину `desktop/component-pins.json` и собирает
`cargo build --locked --release --target x86_64-pc-windows-msvc` с
`MUNDUS_MEMORIA_VERSION=<win-версия релиза>` — build.rs штампует VERSIONINFO
этой версией (legacy `KOSMOS_MEMORIA_VERSION` тоже читается) (без env — версия из Cargo.toml).

Запуск из установленного продукта: launcher-команда «Открыть Memoria (GPUI)»,
ярлык Start Menu «Mundus Memoria» и кнопка «Открыть Memoria» в GPUI Manager —
все три пути резолвят один exe и используют общий `MUNDUS_DATA_DIR`
(`%APPDATA%\Mundus` в prod), поэтому Memoria GPUI читает тот же
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

