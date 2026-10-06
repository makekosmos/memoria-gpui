# AGENTS.md — memoria-gpui

Нативное GPUI-приложение Memoria (заметки, дневник) для Mundus. Главная
платформа — Windows; Linux и macOS собираются и проверяются в CI. Приложение
ставится Engine из GitHub Releases этого репо.

## Карта

- Корневой крейт `memoria-gpui`: приложение, окна и экраны (`src/app/`), тема
  (`src/theme.rs`), доступность (`src/a11y.rs`), UI-тесты (`src/ui_tests/`),
  `src/bin/check-source-size.rs` — гейт размера файлов.
- `crates/memoria-model` — слой данных без gpui: транспорт Engine, модели
  объектов ARK, кодек содержимого. Не добавляй сюда зависимость от gpui.
- `crates/memoria-editor-core` — ядро редактора без GPUI (буфер, парсер
  Markdown, Live Preview, команды, undo); `crates/memoria-editor-gpui` —
  виджет редактора на нём. Дизайн: `DESIGN.md` в каждом крейте.
- `PARITY.md` — матрица паритета с Vue-версией Memoria; `PORT_TESTS.md` — план
  портирования тестов; `fixtures/` и `crates/memoria-model/fixtures/` —
  снимок ARK и эталоны, `reference/screens/` — эталонные скриншоты.

## Границы

- Все данные идут через операции Engine (`http://127.0.0.1:<port>/v1/rpc` с
  токеном из `engine.lock.json`). Приложение никогда не открывает SQLite само
  (`cortex/docs/write-boundary.md`).
- Engine ищется по `engine.lock.json` в `MUNDUS_DATA_DIR`; без неё — в папке
  конфигурации Mundus. Старые `KOSMOS_DATA_DIR` и `Kosmos` пока читаются как
  запасной путь: первый найденный lock-файл побеждает.
- Переменные окружения: `MEMORIA_DEMO=1` (встроенный набор данных, Engine не
  нужен, то же используют UI-тесты), `MEMORIA_OFFSCREEN=1` (окно за экраном
  для headless-запуска), `MEMORIA_IMAGES_DIR` (корень картинок редактора).

## Запуск

```text
cargo run                          # против живого Engine
MEMORIA_DEMO=1 cargo run           # без Engine
node scripts/seed-fixtures.mjs     # залить снимок ARK в запущенный Engine
```

Engine берётся из `makekosmos/cortex`: `pnpm run dev -- --engine-only` в корне
cortex (`--data-dir DIR`, тот же `DIR` отдай приложению через
`MUNDUS_DATA_DIR`). На Linux нужны `ld.lld` в `PATH` (`.cargo/config.toml`),
`pkg-config` и `libfontconfig-dev`; полный список пакетов — в шаге
`Linux dependencies` файла `.github/workflows/ci.yml`.

## Проверки

CI запускается на каждый PR и на push в `main`: `ci.yml` (Ubuntu: fmt, clippy
с `-D warnings`, nextest, `cargo deny`, `cargo shear`, размер файлов) и
`build.yml` (сборка и тесты на Windows, Linux и macOS, проверка правил версий
`python scripts/test_release.py`). Ночью (00:00 МСК) `build.yml` выпускает
релиз.

Локально те же проверки гонит `hk` (`hk.pkl`):

```text
cargo install hk --locked && hk install     # один раз на копию
hk run pre-push                             # или hk check --all
```

Гейт: `cargo nextest run --workspace --all-features`, `cargo shear`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`cargo deny check advisories bans sources`, `check-source-size`.
`cargo-nextest`, `cargo-shear`, `cargo-deny` ставь через `cargo install`.
Не обходи хуки через `--no-verify`. Для ядра редактора есть бенчмарки
(`cargo bench -p memoria-editor-core`) и фаззинг парсера
(`cargo +nightly fuzz run parse_project` в `crates/memoria-editor-core`).

## Правила кода

- Мёртвый код удаляй сразу, вместе с тестами только на него. Исключение —
  модули из `docs/unwired-modules.md`: они намеренно оставлены для будущего
  подключения, их не удаляй. Политика линтов
  одна — таблица `[workspace.lints]` в корневом `Cargo.toml` плюс
  `-D warnings`; сейчас исключений нет. Не добавляй `-A …` в командную строку
  и `#[allow(dead_code)]` в код: чини код.
- Размер файла — не больше 300 строк (`src/bin/check-source-size.rs`);
  список `GRANDFATHERED` только сокращается.
- `gpui` (псевдоним `gpui-kit`), `gpui-component`, `gpui-base` закреплены
  точными версиями и должны совпадать с cortex/manager-gpui, agenda-gpui и
  dictation: две версии gpui в одной сборке — ошибка типов. Поднимай вместе.
  `imago-gpui`, `mundus-gpui-kit` и `[patch.crates-io]` закреплены по rev
  репозитория `makekosmos/imago`.
- UI-тесты — `#[gpui::test]` с `TestAppContext` (фича gpui `test-support` только
  в dev-зависимостях). Меняя поведение, обнови `PARITY.md`.
- Цвета и шрифты — только через токены темы. Сохраняй клавиатурный доступ,
  фокус и доступные имена элементов (`src/a11y.rs`).
- Подписки, таймеры и слушатели снимай при уничтожении окна или view.

## Релиз

Выпускает `build.yml` по расписанию: версию в `Cargo.toml` вручную не меняй,
тег не ставь и релиз не публикуй без просьбы (`scripts/release.py`,
`scripts/publish-version.sh`).
