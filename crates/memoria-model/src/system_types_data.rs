//! `systemTypeDefinitions.ts` + `systemTypeVisualDefinitions.ts` +
//! `systemTypeGameDefinitions.ts` — the built-in `SYSTEM_TYPES` records.
//! Schema/ui-schema strings are literal transcriptions of the Vue
//! `JSON.stringify(...)` literals — byte-identical, key order included.

use std::sync::LazyLock;

use crate::model::NoteType;
use crate::note_type_schemas::parse_note_type_definition;
use crate::note_types::parse_note_type_ui_schema;

pub const SYSTEM_TYPE_NOTE_ID: &str = "note_obj";
pub const SYSTEM_TYPE_BOOK_ID: &str = "book_obj";
pub const SYSTEM_TYPE_WORKOUT_ID: &str = "system-type-workout";
pub const SYSTEM_TYPE_EXERCISE_ID: &str = "system-type-exercise";
pub const SYSTEM_TYPE_JOURNAL_ID: &str = "system-type-journal";
pub const SYSTEM_TYPE_COLLECTION_ID: &str = "collection_obj";
pub const SYSTEM_TYPE_IMAGE_ID: &str = "image_obj";
pub const SYSTEM_TYPE_PERSON_ID: &str = "person_obj";
pub const SYSTEM_TYPE_GAME_ID: &str = "game_obj";

const BOOK_LANGUAGE_OPTIONS_JSON: &str = r#"["Русский","Английский","Украинский","Белорусский","Немецкий","Французский","Испанский","Итальянский","Португальский","Польский","Чешский","Нидерландский","Шведский","Норвежский","Датский","Финский","Китайский","Японский","Корейский","Арабский","Турецкий","Греческий","Латинский","Другой"]"#;

pub const NOTE_SCHEMA_JSON: &str = r#"{"fields":[{"id":"description","label":"Описание","kind":"long_text","required":false,"visible":true,"read_only":false,"system":false},{"id":"related_notes","label":"Связанные заметки","kind":"relation","required":false,"visible":true,"read_only":false,"link_type":"related","system":false}]}"#;
pub const NOTE_HEADER_TEMPLATE_JSON: &str =
    r#"{"kind":"default","primaryFieldIds":[],"secondaryFieldIds":[],"imageFieldId":null}"#;
pub const NOTE_UI_SCHEMA_JSON: &str = r#"{"featured_fields":[],"visible_fields":[],"hidden_fields":["description","related_notes","created_at","updated_at","deleted_at"],"read_only_fields":[],"field_order":["description","related_notes"],"header_layout":"inline","default_layout":"page","default_template_id":null,"collection_name":"Заметки"}"#;
pub const JOURNAL_UI_SCHEMA_JSON: &str = r#"{"featured_fields":[],"visible_fields":[],"hidden_fields":["description","related_notes","created_at","updated_at","deleted_at"],"read_only_fields":[],"field_order":["description","related_notes"],"header_layout":"inline","default_layout":"page","default_template_id":null,"collection_name":"Дневники"}"#;

pub static BOOK_SCHEMA_JSON: LazyLock<String> = LazyLock::new(|| {
    format!(
        r#"{{"fields":[{{"id":"cover_image","label":"Обложка","kind":"image","required":false,"visible":true,"read_only":false,"system":false}},{{"id":"author","label":"Автор","kind":"text","required":false,"visible":true,"read_only":false,"system":false}},{{"id":"isbn","label":"ISBN","kind":"text","required":false,"visible":true,"read_only":false,"system":false}},{{"id":"page_count","label":"Страниц","kind":"number","required":false,"visible":true,"read_only":false,"system":false}},{{"id":"language","label":"Язык","kind":"select","options":{opts},"required":false,"visible":true,"read_only":false,"system":false}},{{"id":"publisher","label":"Издательство","kind":"text","required":false,"visible":true,"read_only":false,"system":false}},{{"id":"published_date","label":"Дата издания","kind":"text","required":false,"visible":true,"read_only":false,"system":false}},{{"id":"source_url","label":"Источник","kind":"url","required":false,"visible":true,"read_only":false,"system":false}}]}}"#,
        opts = BOOK_LANGUAGE_OPTIONS_JSON
    )
});
pub const BOOK_HEADER_TEMPLATE_JSON: &str = r#"{"kind":"default","primaryFieldIds":["author"],"secondaryFieldIds":[],"imageFieldId":"cover_image"}"#;
pub const BOOK_UI_SCHEMA_JSON: &str = r#"{"featured_fields":["author"],"visible_fields":["cover_image","author","isbn","page_count","language","publisher","published_date","source_url"],"hidden_fields":["created_at","updated_at","deleted_at"],"read_only_fields":[],"field_order":["cover_image","author","isbn","page_count","language","publisher","published_date","source_url"],"header_layout":"inline","default_layout":"page","default_template_id":null,"collection_name":"Книги"}"#;

pub const COLLECTION_SCHEMA_JSON: &str = r#"{"fields":[{"id":"object_type_id","label":"Тип объектов","kind":"text","required":true,"visible":true,"read_only":true,"system":true},{"id":"description","label":"Описание","kind":"long_text","required":false,"visible":true,"read_only":false,"system":false}]}"#;
pub const COLLECTION_HEADER_TEMPLATE_JSON: &str = r#"{"kind":"default","primaryFieldIds":["object_type_id"],"secondaryFieldIds":[],"imageFieldId":null}"#;
pub const COLLECTION_UI_SCHEMA_JSON: &str = r#"{"featured_fields":["object_type_id"],"visible_fields":["object_type_id"],"hidden_fields":["description","created_at","updated_at","deleted_at"],"read_only_fields":["object_type_id"],"field_order":["object_type_id","description"],"header_layout":"inline","default_layout":"page","default_template_id":null,"collection_name":"Коллекции"}"#;

pub const WORKOUT_SCHEMA_JSON: &str = r#"{"fields":[{"id":"date","label":"Дата","kind":"date","required":true},{"id":"duration_min","label":"Длительность (мин)","kind":"number","required":false},{"id":"volume_kg","label":"Объем (кг)","kind":"number","required":false},{"id":"exercise_count","label":"Упражнений","kind":"number","required":false},{"id":"source","label":"Источник","kind":"text","required":false},{"id":"external_id","label":"Внешний ID","kind":"text","required":false}]}"#;
pub const WORKOUT_HEADER_TEMPLATE_JSON: &str = r#"{"kind":"default","primaryFieldIds":["date","duration_min"],"secondaryFieldIds":["volume_kg","exercise_count"],"imageFieldId":null}"#;
pub const WORKOUT_UI_SCHEMA_JSON: &str = r#"{"featured_fields":["date","duration_min"],"visible_fields":["date","duration_min","volume_kg","exercise_count","source"],"hidden_fields":["created_at","updated_at","deleted_at","external_id"],"read_only_fields":[],"field_order":["date","duration_min","volume_kg","exercise_count","source","external_id"],"header_layout":"inline","default_layout":"page","default_template_id":null,"collection_name":"Тренировки"}"#;

pub const EXERCISE_SCHEMA_JSON: &str = r#"{"fields":[{"id":"exercise_name","label":"Упражнение","kind":"text","required":true},{"id":"exercise_type","label":"Тип","kind":"select","required":false,"options":["weight_reps","reps_only","distance_duration","duration"]},{"id":"equipment","label":"Оборудование","kind":"select","required":false,"options":["barbell","dumbbell","machine","cable","bodyweight","none","other"]},{"id":"muscle_group","label":"Группа мышц","kind":"text","required":false},{"id":"sets_summary","label":"Подходы","kind":"long_text","required":false},{"id":"best_set","label":"Лучший подход","kind":"text","required":false},{"id":"total_volume_kg","label":"Объем (кг)","kind":"number","required":false},{"id":"notes","label":"Заметки","kind":"long_text","required":false}]}"#;
pub const EXERCISE_HEADER_TEMPLATE_JSON: &str = r#"{"kind":"default","primaryFieldIds":["exercise_name","muscle_group"],"secondaryFieldIds":["sets_summary","best_set"],"imageFieldId":null}"#;
pub const EXERCISE_UI_SCHEMA_JSON: &str = r#"{"featured_fields":["exercise_name","muscle_group"],"visible_fields":["exercise_name","exercise_type","equipment","muscle_group","sets_summary","best_set","total_volume_kg","notes"],"hidden_fields":["created_at","updated_at","deleted_at"],"read_only_fields":[],"field_order":["exercise_name","exercise_type","equipment","muscle_group","sets_summary","best_set","total_volume_kg","notes"],"header_layout":"inline","default_layout":"page","default_template_id":null,"collection_name":"Упражнения"}"#;

pub const IMAGE_SCHEMA_JSON: &str = r#"{"fields":[{"id":"image","label":"Изображение","kind":"image","required":true,"visible":true,"read_only":false,"system":false},{"id":"file_name","label":"Имя файла","kind":"text","required":false,"visible":true,"read_only":false,"system":false},{"id":"mime_type","label":"MIME-тип","kind":"text","required":false,"visible":false,"read_only":true,"system":true},{"id":"size_bytes","label":"Размер (байт)","kind":"number","required":false,"visible":true,"read_only":true,"system":true},{"id":"width","label":"Ширина","kind":"number","required":false,"visible":true,"read_only":true,"system":true},{"id":"height","label":"Высота","kind":"number","required":false,"visible":true,"read_only":true,"system":true},{"id":"resolution","label":"Разрешение","kind":"text","required":false,"visible":true,"read_only":true,"system":true},{"id":"source_path","label":"Исходный путь","kind":"text","required":false,"visible":false,"read_only":true,"system":true},{"id":"alt_text","label":"Alt-текст","kind":"text","required":false,"visible":true,"read_only":false,"system":false}]}"#;
pub const IMAGE_HEADER_TEMPLATE_JSON: &str = r#"{"kind":"default","primaryFieldIds":["image"],"secondaryFieldIds":["file_name","resolution","size_bytes"],"imageFieldId":"image"}"#;
pub const IMAGE_UI_SCHEMA_JSON: &str = r#"{"featured_fields":["file_name","resolution"],"visible_fields":["image","file_name","size_bytes","width","height","resolution","alt_text"],"hidden_fields":["created_at","updated_at","deleted_at","mime_type","source_path"],"read_only_fields":["mime_type","source_path","size_bytes","width","height","resolution"],"field_order":["image","file_name","size_bytes","width","height","resolution","alt_text","mime_type","source_path"],"header_layout":"inline","default_layout":"page","default_template_id":null,"collection_name":"Изображения"}"#;

pub static PERSON_SCHEMA_JSON: LazyLock<String> = LazyLock::new(|| {
    format!(
        r#"{{"fields":[{{"id":"first_name","label":"Имя","kind":"text","required":false,"visible":true,"read_only":false,"system":false}},{{"id":"last_name","label":"Фамилия","kind":"text","required":false,"visible":true,"read_only":false,"system":false}},{{"id":"patronymic","label":"Отчество","kind":"text","required":false,"visible":true,"read_only":false,"system":false}},{{"id":"birth_date","label":"Дата рождения","kind":"date","required":false,"visible":true,"read_only":false,"system":false}},{{"id":"photo","label":"Фотография","kind":"relation","required":false,"visible":true,"read_only":false,"multiple":false,"allowed_object_types":["{img}"],"system":false}}]}}"#,
        img = SYSTEM_TYPE_IMAGE_ID
    )
});
pub const PERSON_HEADER_TEMPLATE_JSON: &str = r#"{"kind":"centered_profile","primaryFieldIds":["first_name","last_name","patronymic"],"secondaryFieldIds":["birth_date","photo"],"imageFieldId":"photo"}"#;
pub const PERSON_UI_SCHEMA_JSON: &str = r#"{"featured_fields":["first_name","last_name","patronymic"],"visible_fields":["first_name","last_name","patronymic","birth_date","photo"],"hidden_fields":["created_at","updated_at","deleted_at"],"read_only_fields":[],"field_order":["photo","first_name","last_name","patronymic","birth_date"],"header_layout":"column","default_layout":"page","default_template_id":null,"collection_name":"Люди"}"#;

pub const GAME_SCHEMA_JSON: &str = r#"{"fields":[{"id":"description","label":"Описание","kind":"long_text","required":false,"visible":true,"read_only":false,"system":false},{"id":"user_rating","label":"Оценка","kind":"number","required":false,"visible":true,"read_only":false,"system":false},{"id":"play_status","label":"Статус","kind":"select","required":false,"visible":true,"read_only":false,"options":["not_started","in_progress","completed","abandoned"],"system":false},{"id":"genres","label":"Жанры","kind":"multi_select","required":false,"visible":true,"read_only":false,"options":["Action","Adventure","RPG","Strategy","Simulation","Shooter","Puzzle","Platformer","Racing","Sports","Survival","Horror","Sandbox","Indie"],"system":false},{"id":"cover_image","label":"Обложка","kind":"image","required":false,"visible":true,"read_only":false,"system":false},{"id":"background_image","label":"Фон","kind":"image","required":false,"visible":true,"read_only":false,"system":false},{"id":"related_notes","label":"Связанные заметки","kind":"relation","required":false,"visible":true,"read_only":false,"link_type":"related","system":false},{"id":"exe_path","label":"Путь к игре","kind":"text","required":false,"visible":true,"read_only":false,"system":true},{"id":"save_path","label":"Путь к сейвам","kind":"text","required":false,"visible":true,"read_only":false,"system":true},{"id":"total_playtime_seconds","label":"Время игры","kind":"number","required":false,"visible":true,"read_only":true,"system":true},{"id":"last_played_at","label":"Последний запуск","kind":"date","required":false,"visible":true,"read_only":true,"system":true},{"id":"play_count","label":"Запусков","kind":"number","required":false,"visible":true,"read_only":true,"system":true},{"id":"save_exists","label":"Сейв найден","kind":"boolean","required":false,"visible":true,"read_only":true,"system":true},{"id":"rawg_id","label":"RAWG ID","kind":"text","required":false,"visible":false,"read_only":true,"system":true},{"id":"exe_name","label":"Имя exe","kind":"text","required":false,"visible":false,"read_only":true,"system":true}]}"#;
pub const GAME_HEADER_TEMPLATE_JSON: &str = r#"{"kind":"default","primaryFieldIds":[],"secondaryFieldIds":["play_status","genres","total_playtime_seconds","last_played_at"],"imageFieldId":null}"#;
pub const GAME_UI_SCHEMA_JSON: &str = r#"{"featured_fields":[],"visible_fields":["play_status","genres","total_playtime_seconds","last_played_at"],"hidden_fields":["created_at","updated_at","deleted_at","description","user_rating","cover_image","background_image","related_notes","exe_path","save_path","play_count","save_exists","rawg_id","exe_name","sync_source"],"read_only_fields":["total_playtime_seconds","last_played_at","play_count","save_exists","rawg_id","exe_name"],"field_order":["play_status","genres","total_playtime_seconds","last_played_at","description","user_rating","play_count","save_exists","cover_image","background_image","related_notes","exe_path","save_path","rawg_id","exe_name"],"header_layout":"inline","default_layout":"page","default_template_id":null,"collection_name":"Игры"}"#;

const LEGACY_GAME_FEATURED_FIELDS: &[&str] = &[
    "play_status",
    "genres",
    "user_rating",
    "total_playtime_seconds",
    "last_played_at",
    "play_count",
    "save_exists",
];
const LEGACY_GAME_VISIBLE_FIELDS: &[&str] = &[
    "description",
    "play_status",
    "genres",
    "user_rating",
    "cover_image",
    "background_image",
    "related_notes",
    "exe_path",
    "save_path",
    "total_playtime_seconds",
    "last_played_at",
    "play_count",
    "save_exists",
];
const GAME_READ_ONLY_FIELDS: &[&str] = &[
    "total_playtime_seconds",
    "last_played_at",
    "play_count",
    "save_exists",
    "rawg_id",
    "exe_name",
];

fn arrays_equal(left: Option<&[String]>, right: &[&str]) -> bool {
    let Some(left) = left else {
        return right.is_empty();
    };
    left.len() == right.len()
        && left
            .iter()
            .map(String::as_str)
            .zip(right)
            .all(|(a, b)| a == *b)
}

/// `shouldUpgradeLegacyGamePresentation`.
pub fn should_upgrade_legacy_game_presentation(note_type: &NoteType) -> bool {
    let Ok(ui) = parse_note_type_ui_schema(note_type.ui_schema_json.as_deref()) else {
        return false;
    };
    arrays_equal(ui.featured_fields.as_deref(), LEGACY_GAME_FEATURED_FIELDS)
        && arrays_equal(ui.visible_fields.as_deref(), LEGACY_GAME_VISIBLE_FIELDS)
        && arrays_equal(ui.read_only_fields.as_deref(), GAME_READ_ONLY_FIELDS)
        && ui.header_layout.as_deref().unwrap_or("inline") == "column"
}

/// `shouldUpgradeLegacyGameSchema` — bad JSON counts as legacy.
pub fn should_upgrade_legacy_game_schema(note_type: &NoteType) -> bool {
    let Ok(definition) = parse_note_type_definition(&note_type.schema_json) else {
        return true;
    };
    let genres = definition.fields.iter().find(|f| f.id == "genres");
    let play_status = definition.fields.iter().find(|f| f.id == "play_status");
    genres.map(|f| f.kind.as_str()) != Some("multi_select")
        || genres
            .and_then(|f| f.options.as_ref())
            .map(|o| o.is_empty())
            .unwrap_or(true)
        || play_status.map(|f| f.kind.as_str()) != Some("select")
}

#[allow(clippy::too_many_arguments)]
fn note_type(
    id: &str,
    name: &str,
    slug: &str,
    icon: &str,
    color: Option<&str>,
    schema_json: String,
    header_template_json: &str,
    ui_schema_json: String,
) -> NoteType {
    NoteType {
        id: id.into(),
        name: name.into(),
        slug: slug.into(),
        icon: Some(icon.into()),
        color: color.map(Into::into),
        schema_json,
        header_template_json: header_template_json.into(),
        ui_schema_json: Some(ui_schema_json),
        created_at: 0,
        updated_at: 0,
        extra: Default::default(),
    }
}

fn collection_type() -> NoteType {
    note_type(
        SYSTEM_TYPE_COLLECTION_ID,
        "Коллекция",
        "collection",
        "folder",
        Some("#a855f7"),
        COLLECTION_SCHEMA_JSON.into(),
        COLLECTION_HEADER_TEMPLATE_JSON,
        COLLECTION_UI_SCHEMA_JSON.into(),
    )
}

/// `SYSTEM_TYPE_COLLECTION` from `systemTypes.ts`.
pub static SYSTEM_TYPE_COLLECTION: LazyLock<NoteType> = LazyLock::new(collection_type);

macro_rules! system_type_static {
    ($name:ident, $id:ident) => {
        /// Named `SYSTEM_TYPE_*` export from `systemTypes.ts`.
        pub static $name: LazyLock<NoteType> = LazyLock::new(|| {
            system_types()
                .into_iter()
                .find(|t| t.id == $id)
                .expect("system type registered")
        });
    };
}

system_type_static!(SYSTEM_TYPE_NOTE, SYSTEM_TYPE_NOTE_ID);
system_type_static!(SYSTEM_TYPE_BOOK, SYSTEM_TYPE_BOOK_ID);
system_type_static!(SYSTEM_TYPE_JOURNAL, SYSTEM_TYPE_JOURNAL_ID);
system_type_static!(SYSTEM_TYPE_IMAGE, SYSTEM_TYPE_IMAGE_ID);
system_type_static!(SYSTEM_TYPE_PERSON, SYSTEM_TYPE_PERSON_ID);
system_type_static!(SYSTEM_TYPE_GAME, SYSTEM_TYPE_GAME_ID);
system_type_static!(SYSTEM_TYPE_WORKOUT, SYSTEM_TYPE_WORKOUT_ID);
system_type_static!(SYSTEM_TYPE_EXERCISE, SYSTEM_TYPE_EXERCISE_ID);

/// `SYSTEM_TYPES` — canonical order (note, book, collection, journal, image,
/// person, game, workout, exercise).
pub fn system_types() -> Vec<NoteType> {
    vec![
        note_type(
            SYSTEM_TYPE_NOTE_ID,
            "Заметка",
            "note_obj",
            "document-text",
            Some("#2aa7ee"),
            NOTE_SCHEMA_JSON.into(),
            NOTE_HEADER_TEMPLATE_JSON,
            NOTE_UI_SCHEMA_JSON.into(),
        ),
        note_type(
            SYSTEM_TYPE_BOOK_ID,
            "Книга",
            "book_obj",
            "book",
            None,
            BOOK_SCHEMA_JSON.clone(),
            BOOK_HEADER_TEMPLATE_JSON,
            BOOK_UI_SCHEMA_JSON.into(),
        ),
        collection_type(),
        note_type(
            SYSTEM_TYPE_JOURNAL_ID,
            "Дневник",
            "journal",
            "document-text",
            Some("#a855f7"),
            NOTE_SCHEMA_JSON.into(),
            NOTE_HEADER_TEMPLATE_JSON,
            JOURNAL_UI_SCHEMA_JSON.into(),
        ),
        note_type(
            SYSTEM_TYPE_IMAGE_ID,
            "Изображение",
            "image",
            "image",
            Some("#38bdf8"),
            IMAGE_SCHEMA_JSON.into(),
            IMAGE_HEADER_TEMPLATE_JSON,
            IMAGE_UI_SCHEMA_JSON.into(),
        ),
        note_type(
            SYSTEM_TYPE_PERSON_ID,
            "Человек",
            "person",
            "user",
            Some("#14b8a6"),
            PERSON_SCHEMA_JSON.clone(),
            PERSON_HEADER_TEMPLATE_JSON,
            PERSON_UI_SCHEMA_JSON.into(),
        ),
        note_type(
            SYSTEM_TYPE_GAME_ID,
            "Игра",
            "game_obj",
            "game-controller",
            Some("#ef4444"),
            GAME_SCHEMA_JSON.into(),
            GAME_HEADER_TEMPLATE_JSON,
            GAME_UI_SCHEMA_JSON.into(),
        ),
        note_type(
            SYSTEM_TYPE_WORKOUT_ID,
            "Тренировка",
            "workout",
            "barbell",
            Some("#f97316"),
            WORKOUT_SCHEMA_JSON.into(),
            WORKOUT_HEADER_TEMPLATE_JSON,
            WORKOUT_UI_SCHEMA_JSON.into(),
        ),
        note_type(
            SYSTEM_TYPE_EXERCISE_ID,
            "Упражнение",
            "exercise",
            "fitness",
            Some("#22c55e"),
            EXERCISE_SCHEMA_JSON.into(),
            EXERCISE_HEADER_TEMPLATE_JSON,
            EXERCISE_UI_SCHEMA_JSON.into(),
        ),
    ]
}
