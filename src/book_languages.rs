//! Port of `src/lib/bookLanguages.ts` — book language options + alias table.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::model::NoteTypeField;

/// `BOOK_LANGUAGE_OPTIONS` — order is part of the field-schema contract.
pub const BOOK_LANGUAGE_OPTIONS: &[&str] = &[
    "Русский",
    "Английский",
    "Украинский",
    "Белорусский",
    "Немецкий",
    "Французский",
    "Испанский",
    "Итальянский",
    "Португальский",
    "Польский",
    "Чешский",
    "Нидерландский",
    "Шведский",
    "Норвежский",
    "Датский",
    "Финский",
    "Китайский",
    "Японский",
    "Корейский",
    "Арабский",
    "Турецкий",
    "Греческий",
    "Латинский",
    "Другой",
];

fn alias_group(
    language: &'static str,
    aliases: &[&'static str],
) -> Vec<(&'static str, &'static str)> {
    aliases.iter().map(|a| (*a, language)).collect()
}

static LANGUAGE_ALIASES: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    [
        alias_group("Русский", &["ru", "rus", "russian", "русский"]),
        alias_group("Английский", &["en", "eng", "english", "английский"]),
        alias_group("Украинский", &["uk", "ukr", "ukrainian", "украинский"]),
        alias_group("Белорусский", &["be", "bel", "belarusian", "белорусский"]),
        alias_group("Немецкий", &["de", "deu", "ger", "german", "немецкий"]),
        alias_group(
            "Французский",
            &["fr", "fra", "fre", "french", "французский"],
        ),
        alias_group("Испанский", &["es", "spa", "spanish", "испанский"]),
        alias_group("Итальянский", &["it", "ita", "italian", "итальянский"]),
        alias_group(
            "Португальский",
            &["pt", "por", "portuguese", "португальский"],
        ),
        alias_group("Польский", &["pl", "pol", "polish", "польский"]),
        alias_group("Чешский", &["cs", "ces", "cze", "czech", "чешский"]),
        alias_group(
            "Нидерландский",
            &["nl", "nld", "dut", "dutch", "нидерландский"],
        ),
        alias_group("Шведский", &["sv", "swe", "swedish", "шведский"]),
        alias_group("Норвежский", &["no", "nor", "norwegian", "норвежский"]),
        alias_group("Датский", &["da", "dan", "danish", "датский"]),
        alias_group("Финский", &["fi", "fin", "finnish", "финский"]),
        alias_group("Китайский", &["zh", "zho", "chi", "chinese", "китайский"]),
        alias_group("Японский", &["ja", "jpn", "japanese", "японский"]),
        alias_group("Корейский", &["ko", "kor", "korean", "корейский"]),
        alias_group("Арабский", &["ar", "ara", "arabic", "арабский"]),
        alias_group("Турецкий", &["tr", "tur", "turkish", "турецкий"]),
        alias_group("Греческий", &["el", "ell", "gre", "greek", "греческий"]),
        alias_group("Латинский", &["la", "lat", "latin", "латинский"]),
    ]
    .into_iter()
    .flatten()
    .collect()
});

/// `normalizeBookLanguage` — alias lookup on a lowercased, `_`→`-` key;
/// unrecognized input returns the trimmed original.
pub fn normalize_book_language(value: &serde_json::Value) -> String {
    // `String(value ?? "")` — objects render as "[object Object]", arrays
    // join elements on commas.
    let original = match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        serde_json::Value::Object(_) => "[object Object]".to_string(),
        serde_json::Value::Array(items) => items
            .iter()
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .collect::<Vec<_>>()
            .join(","),
        other => other.to_string(),
    };
    let original = original.trim().to_string();
    // `toLocaleLowerCase("ru-RU")` ≈ Unicode lowercase; `_` → `-`.
    let key = original.to_lowercase().replace('_', "-");
    if key.is_empty() {
        return String::new();
    }
    if let Some(mapped) = LANGUAGE_ALIASES.get(key.as_str()) {
        return (*mapped).to_string();
    }
    let head = key.split('-').next().unwrap_or("");
    if let Some(mapped) = LANGUAGE_ALIASES.get(head) {
        return (*mapped).to_string();
    }
    original
}

/// `isBookLanguageField` — id `language` + exact option list.
pub fn is_book_language_field(field: &NoteTypeField) -> bool {
    if field.id != "language" {
        return false;
    }
    match &field.options {
        Some(options) => {
            options.len() == BOOK_LANGUAGE_OPTIONS.len()
                && options
                    .iter()
                    .map(String::as_str)
                    .zip(BOOK_LANGUAGE_OPTIONS.iter().copied())
                    .all(|(a, b)| a == b)
        }
        None => false,
    }
}
