//! Port of `src/lib/objectFieldFormatting.ts` — display formatting for typed
//! object fields (summary columns on the object table, image detail fields).

use serde_json::Value;

use crate::book_languages::{is_book_language_field, normalize_book_language};
use crate::dates::format_readable_russian_date;
use crate::model::ResolvedNoteTypeField;

fn play_status_label(value: &str) -> &str {
    match value {
        "not_started" => "Не начата",
        "in_progress" => "В процессе",
        "completed" => "Пройдена",
        "abandoned" => "Заброшена",
        _ => value,
    }
}

fn read_finite_number(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) if !s.trim().is_empty() => s.trim().parse::<f64>().ok().map(|f| f as i64),
        _ => None,
    }
}

fn has_meaningful_scalar(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(s) => !s.trim().is_empty(),
        _ => true,
    }
}

/// `formatPlaytimeSeconds` — seconds → «1 ч 30 мин» style labels.
fn format_playtime_seconds(value: &Value) -> String {
    let Some(total_seconds) = read_finite_number(value) else {
        return String::new();
    };
    let total_seconds = total_seconds.max(0);
    if total_seconds < 60 {
        return "Меньше минуты".into();
    }
    let total_minutes = total_seconds / 60;
    let days = total_minutes / (60 * 24);
    let hours = (total_minutes % (60 * 24)) / 60;
    let minutes = total_minutes % 60;
    if days > 0 {
        return if hours > 0 {
            format!("{days} д {hours} ч")
        } else {
            format!("{days} д")
        };
    }
    if hours > 0 {
        return if minutes > 0 {
            format!("{hours} ч {minutes} мин")
        } else {
            format!("{hours} ч")
        };
    }
    format!("{minutes} мин")
}

/// `formatObjectFieldValue` — the display string for a resolved field value.
/// Empty string means "no value" (the Vue table renders `—` for it).
pub fn format_object_field_value(field: &ResolvedNoteTypeField, value: &Value) -> String {
    if let Value::Array(items) = value {
        if items.is_empty() {
            return String::new();
        }
        return items
            .iter()
            .map(|item| match item {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .collect::<Vec<_>>()
            .join(", ");
    }

    if !has_meaningful_scalar(value) {
        return String::new();
    }

    let kind = field.field.kind.as_str();
    let id = field.field.id.as_str();

    if kind == "boolean" {
        return if value == &Value::Bool(true) {
            "Да".into()
        } else {
            "Нет".into()
        };
    }
    if id == "play_status" {
        if let Value::String(s) = value {
            return play_status_label(s).to_string();
        }
    }
    if is_book_language_field(&field.field) {
        return normalize_book_language(value);
    }
    if id == "total_playtime_seconds" {
        return format_playtime_seconds(value);
    }
    if id == "last_played_at" || kind == "date" {
        return format_readable_russian_date(value);
    }

    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NoteTypeField;
    use serde_json::json;

    fn fld(id: &str, kind: &str) -> ResolvedNoteTypeField {
        ResolvedNoteTypeField {
            field: NoteTypeField {
                id: id.into(),
                label: id.into(),
                kind: kind.into(),
                ..Default::default()
            },
            visible: true,
            read_only: false,
        }
    }

    #[test]
    fn boolean_and_arrays() {
        assert_eq!(
            format_object_field_value(&fld("x", "boolean"), &json!(true)),
            "Да"
        );
        assert_eq!(
            format_object_field_value(&fld("x", "boolean"), &json!(0)),
            "Нет"
        );
        assert_eq!(
            format_object_field_value(&fld("genres", "multi_select"), &json!(["RPG", "Инди"])),
            "RPG, Инди"
        );
        assert_eq!(
            format_object_field_value(&fld("g", "multi_select"), &json!([])),
            ""
        );
    }

    #[test]
    fn play_status() {
        assert_eq!(
            format_object_field_value(&fld("play_status", "select"), &json!("in_progress")),
            "В процессе"
        );
        assert_eq!(
            format_object_field_value(&fld("play_status", "select"), &json!("weird")),
            "weird"
        );
    }

    #[test]
    fn playtime() {
        assert_eq!(
            format_object_field_value(&fld("total_playtime_seconds", "number"), &json!(30)),
            "Меньше минуты"
        );
        assert_eq!(
            format_object_field_value(&fld("total_playtime_seconds", "number"), &json!(5400)),
            "1 ч 30 мин"
        );
        assert_eq!(
            format_object_field_value(&fld("total_playtime_seconds", "number"), &json!(180000)),
            "2 д 2 ч"
        );
    }

    #[test]
    fn book_language() {
        // normalize_book_language resolves ISO codes to Russian names when the
        // value is a book-language field.
        let f = fld("language", "select");
        let out = format_object_field_value(&f, &json!("en"));
        assert!(!out.is_empty());
    }
}
