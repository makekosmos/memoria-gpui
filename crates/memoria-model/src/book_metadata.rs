//! Port of `src/lib/bookMetadata.ts` — the `BookMetadata` contract shared by
//! the Engine ops (`bookMetadata.lookupIsbn` → this shape, `fetchPage` →
//! `{finalUrl, html}` parsed by `book_metadata_extract`) plus the ISBN and
//! merge helpers the import modal uses.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::LazyLock;

/// `BookMetadata` — snake_case keys mirror the TS interface and the Engine
/// op payload verbatim.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BookMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isbn: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
}

/// `BookMetadataPage` — `bookMetadata.fetchPage` normalized result.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BookMetadataPage {
    pub final_url: String,
    pub html: String,
}

/// `cleanText` — collapse whitespace; numbers stringify.
pub(crate) fn clean_text(value: &Value) -> String {
    match value {
        Value::Number(n) if n.as_f64().map(|f| f.is_finite()).unwrap_or(false) => n.to_string(),
        Value::String(s) => s.split_whitespace().collect::<Vec<_>>().join(" "),
        _ => String::new(),
    }
}

fn valid_isbn10(value: &str) -> bool {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() != 10 || !chars[..9].iter().all(|c| c.is_ascii_digit()) {
        return false;
    }
    if !chars[9].is_ascii_digit() && chars[9] != 'X' {
        return false;
    }
    let sum: u32 = chars
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let digit = if *c == 'X' {
                10
            } else {
                c.to_digit(10).unwrap()
            };
            digit * (10 - i as u32)
        })
        .sum();
    sum.is_multiple_of(11)
}

fn valid_isbn13(value: &str) -> bool {
    let digits: Vec<u32> = value.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.len() != 13 || value.len() != 13 {
        return false;
    }
    let sum: u32 = digits[..12]
        .iter()
        .enumerate()
        .map(|(i, d)| d * if i % 2 == 0 { 1 } else { 3 })
        .sum();
    (10 - (sum % 10)) % 10 == digits[12]
}

fn isbn10_to_13(value: &str) -> String {
    let first_twelve = format!("978{}", &value[..9]);
    let sum: u32 = first_twelve
        .chars()
        .enumerate()
        .map(|(i, c)| c.to_digit(10).unwrap() * if i % 2 == 0 { 1 } else { 3 })
        .sum();
    format!("{}{}", first_twelve, (10 - (sum % 10)) % 10)
}

static ISBN_CANDIDATE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?:97[89][\d\s-]{10,20}|[\dX][\dX\s-]{8,18})").expect("isbn re")
});

/// `normalizeIsbn` — find candidate runs, strip separators, validate
/// checksum; ISBN-10 promotes to ISBN-13. "" when nothing is valid.
pub fn normalize_isbn(value: &Value) -> String {
    let cleaned = clean_text(value).to_uppercase();
    for candidate in ISBN_CANDIDATE_RE.find_iter(&cleaned) {
        let normalized: String = candidate
            .as_str()
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == 'X')
            .collect();
        if valid_isbn13(&normalized) {
            return normalized;
        }
        if valid_isbn10(&normalized) {
            return isbn10_to_13(&normalized);
        }
    }
    String::new()
}

/// Metadata field as a `Value` for prop merge/preview rows.
pub fn book_metadata_field_value(m: &BookMetadata, key: &str) -> Value {
    match key {
        "title" => m.title.clone().map_or(Value::Null, Value::from),
        "author" => m.author.clone().map_or(Value::Null, Value::from),
        "cover_image" => m.cover_image.clone().map_or(Value::Null, Value::from),
        "isbn" => m.isbn.clone().map_or(Value::Null, Value::from),
        "page_count" => m.page_count.map_or(Value::Null, Value::from),
        "language" => m.language.clone().map_or(Value::Null, Value::from),
        "publisher" => m.publisher.clone().map_or(Value::Null, Value::from),
        "published_date" => m.published_date.clone().map_or(Value::Null, Value::from),
        "source_url" => m.source_url.clone().map_or(Value::Null, Value::from),
        _ => Value::Null,
    }
}

/// `isEmptyBookValue` — number NaN, empty array, null/"" all count empty.
pub fn is_empty_book_value(value: &Value) -> bool {
    match value {
        Value::Number(n) => n.as_f64().map(|f| !f.is_finite()).unwrap_or(true),
        Value::Array(items) => items.is_empty(),
        _ => value.is_null() || clean_text(value).is_empty(),
    }
}

/// `hasExtractedBookData` — any populated field besides `source_url`.
pub fn has_extracted_book_data(metadata: &BookMetadata) -> bool {
    let filled = |value: &Option<String>| value.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
    filled(&metadata.title)
        || filled(&metadata.author)
        || filled(&metadata.cover_image)
        || filled(&metadata.isbn)
        || metadata.page_count.is_some()
        || filled(&metadata.language)
        || filled(&metadata.publisher)
        || filled(&metadata.published_date)
}

/// `fillMissingBookMetadata` — merge `fallback` into `primary`; primary's
/// non-empty values win.
pub fn fill_missing_book_metadata(primary: &BookMetadata, fallback: &BookMetadata) -> BookMetadata {
    let mut merged = fallback.clone();
    let Ok(Value::Object(primary_map)) = serde_json::to_value(primary) else {
        return merged;
    };
    for (key, value) in &primary_map {
        if is_empty_book_value(value) {
            continue;
        }
        match key.as_str() {
            "title" => merged.title = value.as_str().map(str::to_string),
            "author" => merged.author = value.as_str().map(str::to_string),
            "cover_image" => merged.cover_image = value.as_str().map(str::to_string),
            "isbn" => merged.isbn = value.as_str().map(str::to_string),
            "page_count" => merged.page_count = value.as_i64(),
            "language" => merged.language = value.as_str().map(str::to_string),
            "publisher" => merged.publisher = value.as_str().map(str::to_string),
            "published_date" => merged.published_date = value.as_str().map(str::to_string),
            "source_url" => merged.source_url = value.as_str().map(str::to_string),
            _ => {}
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn isbn10_promotes_to_13() {
        // Vue golden: BookMetadataImportModal.spec.ts normalizeIsbn cases.
        assert_eq!(normalize_isbn(&json!("0-306-40615-2")), "9780306406157");
        assert_eq!(normalize_isbn(&json!("9780140328721")), "9780140328721");
        assert_eq!(
            normalize_isbn(&json!("ISBN: 978-0-14-032872-1")),
            "9780140328721"
        );
    }

    #[test]
    fn isbn_rejects_invalid_checksums() {
        // Vue: malformed 978-… ISBN-10-checksum shape must not survive.
        assert_eq!(normalize_isbn(&json!("978-0-306-40615-8")), "");
        assert_eq!(normalize_isbn(&json!("not an isbn")), "");
        assert_eq!(normalize_isbn(&json!("0140328729")), "");
    }

    #[test]
    fn merge_prefers_primary_non_empty() {
        let primary = BookMetadata {
            title: Some("T".into()),
            ..Default::default()
        };
        let fallback = BookMetadata {
            title: Some("Old".into()),
            author: Some("A".into()),
            ..Default::default()
        };
        let merged = fill_missing_book_metadata(&primary, &fallback);
        assert_eq!(merged.title.as_deref(), Some("T"));
        assert_eq!(merged.author.as_deref(), Some("A"));
    }
}
