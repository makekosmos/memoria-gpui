# memoria-gpui

Native GPUI-приложение Memoria для Mundus. Правила для агентов: [AGENTS.md](AGENTS.md).

Source of truth: `makekosmos/memoria` @ `7ccbb9f89841fc93c8bcb48aef67f3df2d3df84e` (0.6.9).
Feature parity matrix: [`PARITY.md`](PARITY.md). Test-port plan: [`PORT_TESTS.md`](PORT_TESTS.md).
Модули, которые есть, но пока не подключены к UI: [`docs/unwired-modules.md`](docs/unwired-modules.md).
Seeded ARK snapshot + reference screenshots: [`fixtures/`](fixtures/),
[`reference/screens/`](reference/screens/).

## Требования

- Rust stable; на Linux ещё `ld.lld` в `PATH`
- `hk` для Git hooks: `cargo install hk --locked`, затем `hk install` в корне репозитория
- Гейты pre-push: `cargo nextest`, `cargo shear`, `cargo clippy`, `cargo deny` (ставятся через `cargo install` по необходимости)
- `pkg-config` + `libfontconfig-dev` (Linux; для gpui text stack)

Весь гейт можно прогнать вручную: `hk check --all` или `hk run pre-push`.

## Сборка

Обычный запуск читает заметки через локальный Mundus Engine API v1.
Сначала запусти Mundus Engine (`pnpm run dev -- --engine-only` в корне cortex). Приложение читает `engine.lock.json` из `%APPDATA%\Mundus`
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

## Установка в составе Mundus

Memoria не входит в установщик Mundus: Engine ставит её из GitHub Releases этого
репозитория (KOS-265) в `%LOCALAPPDATA%\Mundus\Apps`. Приложение читает тот же
`engine.lock.json` в `MUNDUS_DATA_DIR`, что и Manager.

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

