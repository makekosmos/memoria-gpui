# Модули, которые есть, но пока не подключены

Эти модули лежат в `crates/memoria-model/src/` и экспортируются из `lib.rs`.
Они портированы из Vue Memoria, покрыты тестами, но **приложение
(`src/app/`) их пока не вызывает**: используются только из тестов. Код
намеренно оставлен, чтобы подключить его позже, когда понадобится
соответствующая функциональность. Не удаляйте их как «мёртвый код».

Строки соответствий Vue → Rust смотрите в [`PARITY.md`](../PARITY.md).

| Модуль | Что делает | Статус |
| --- | --- | --- |
| [`command_bus`](#command_bus) | очередь команд хоста | не подключён |
| [`task_status`](#task_status--task_sync) + [`task_sync`](#task_status--task_sync) | задачи Agenda (`com.kosmos.task`) | не подключены |
| [`save_actions`](#save_actions) | сериализация сохранений одной записи | не подключён, частично заменён |
| [`entry_changes`](#entry_changes) | «изменил ли пользователь запись» | не подключён |
| [`char_count`](#char_count) | счётчик символов по ProseMirror JSON | заменён, см. ниже |
| [`icon_resolver`](#icon_resolver) | имя иконки Anytype → `data:` URI | заменён, см. ниже |
| [`live_list_filter`](#live_list_filter) | попадает ли изменённый объект в список | не подключён |

## command_bus

`command_bus.rs` (порт `kepler-command-bus.ts`).

**Что делает.** Внутренняя шина команд: команды хоста приходят на каналы
`memoria:…`, нормализуются в `eden:…` (`eden_channel`) и доставляются
подписчикам. Если подписчика ещё нет, команда ставится в очередь и уходит
первому, кто подпишется. Падение одного обработчика не мешает остальным.
API: `CommandBus::{dispatch, on_command, unsubscribe}`.

**Где подключать.** Нужно, когда Memoria должна принимать команды извне
(из Mundus Manager, глобальные горячие клавиши, deep link). Создать
`CommandBus` в структуре `Memoria` (`src/app.rs`), диспетчеризовать из
места, где приходит внешняя команда, а подписчиком сделать
`Memoria::send(Command::…)` (`src/app/dispatch.rs`). Внутренние команды
UI → Engine идут через `store::Command` и шину не используют.

## task_status + task_sync

`task_status.rs` (порт `taskStatus.ts`), `task_sync.rs` (порт
`kepler-task-sync.ts`).

**Что делают.** Работа с задачами Agenda как с ARK-объектами
`com.kosmos.task`. `task_status`: статусы `triage → backlog/todo →
done/canceled`, нормализация старых значений, производные флаги
`is_completed` / `is_cancelled`. `task_sync`: регистрация типа объекта
(`ensure_task_object_type_registered`), `get_task`, `create_task`,
`patch_task`, `soft_delete_task` и `classify_object_change` для событий
Engine. Всё идёт через трейт `ArkBridge`, без прямого доступа к сети.
Совместимо с Agenda и Delphi.

**Где подключать.** В Memoria пока нет UI задач. Когда понадобится
(например, превращать строку заметки в задачу): вызывать функции с
`ArkBridge`, который реализует `store::Engine`, из обработчика
`Command` в `memoria_model::store`, а `classify_object_change`
подключить в `Memoria::on_engine_event` (`src/app/replies.rs`, ветка
`EngineEvent::Changed`).

## save_actions

`save_actions.rs` (порт `edenStoreSaveActions.ts`).

**Что делает.** `SaveActions<B: SaveBridge>` сериализует сохранения одной
записи: пока предыдущее сохранение идёт, новый черновик ждёт в очереди;
обновляет `updated_at`, конфликтные состояния (`record_conflict`,
`recheck_conflicts` подставляет вызывающий). Метод `handle_save`.

**Где подключать.** Сейчас сохранение живёт в `src/app/doc.rs` (`DocEvent::Save`,
`title_dirty`, `pending_save_title`) и уходит в Engine через
`Command::SaveEntry`. Модуль нужен, если сохранение переедет в отдельный
рабочий поток/слой, где важны очерёдность и обнаружение конфликтов при
гонках. Тогда `SaveBridge` реализуется поверх `Engine`, а
`record_conflict` связывается с `ConflictRepository` (`src/app.rs`).

## entry_changes

`entry_changes.rs` (порт `entryChanges.ts`).

**Что делает.** `has_user_visible_entry_changes` сравнивает черновик с
сохранённой записью, предварительно нормализовав свойства шапки и layout
по схеме типа заметки, чтобы значения по умолчанию из схемы не считались
правками пользователя (постмортем 2026-06-17).

**Где подключать.** В `Doc::is_dirty` (`src/app/doc.rs`) и там, где
решается, отправлять ли `SaveEntry` (`src/app/editor_host.rs`,
`src/app/prop_edit.rs`), чтобы не писать запись и не менять `updated_at`
без реальных правок.

## char_count

`char_count.rs` (порт `charCount.ts`).

**Что делает.** `count_chars_in_prose_mirror_doc` считает символы по
ProseMirror/TipTap JSON (кодпоинты, плюс по одному символу на границу
блока и жёсткий перенос).

**Статус.** Счётчик уже работает: `Editor::char_count`
(`crates/memoria-editor-gpui/src/editor.rs`) использует
`memoria_editor_core::charcount`, значение выводится в `src/app/note.rs`.
Этот модуль нужен только если счётчик понадобится для записи в
формате TipTap JSON без открытого редактора (например, в списке).

## icon_resolver

`icon_resolver.rs` (порт `iconResolver.ts`).

**Что делает.** `object_icon_uri` превращает имя иконки Anytype в
`data:` URI с монохромным SVG (`stroke="currentColor"` для mask-image).

**Статус.** UI рисует иконки через `IconId`
(`sidebar_model::icon_for_name`, `src/app/sidebar.rs`), так что модуль
дублирует это для случаев, где нужен именно URI: если иконку надо
отдать в виджет, принимающий только изображение. Если такой потребности
не появится, нужно решить, держать ли модуль.

## live_list_filter

`live_list_filter.rs` (порт `liveListFilter.ts`).

**Что делает.** `should_include_type_in_eden_list_for_live_update`
решает, должен ли изменённый «вживую» объект попасть в список: исключает
пузыри дневника, скрытые типы коллекций и коллекции с невидимым
`object_type_id`.

**Где подключать.** Сейчас `Memoria::on_engine_event`
(`src/app/replies.rs`, `EngineEvent::Changed`) просто заново загружает
весь список. Если перейти на точечные обновления, этот фильтр вместе с
`task_sync::classify_object_change` определяет, что вставлять в список
без полной перезагрузки.
