//! `extractBookMetadata` port — DOMParser-free reimplementation of
//! `bookMetadata.ts` extraction on top of `scraper` (html5ever, the same
//! HTML5 parser family browsers use). JSON-LD/schema.org records win; meta
//! tags and labeled page text are the fallbacks.

use serde_json::Value;
use std::sync::LazyLock;

use crate::book_languages::normalize_book_language;
use crate::book_metadata::{clean_text, normalize_isbn, BookMetadata, BookMetadataPage};
use jsonld::{first_json_value, parse_json_ld, string_from_json_value};

mod jsonld;

static PAGE_LINE_SEL: LazyLock<scraper::Selector> = LazyLock::new(|| {
    scraper::Selector::parse("h1, h2, h3, p, li, dt, dd, th, td, div, section")
        .expect("line selector")
});

fn meta(document: &scraper::Html, names: &[&str]) -> String {
    for name in names {
        let selector =
            format!("meta[property=\"{name}\"], meta[name=\"{name}\"], meta[itemprop=\"{name}\"]");
        let Ok(selector) = scraper::Selector::parse(&selector) else {
            continue;
        };
        if let Some(element) = document.select(&selector).next() {
            let value = clean_text(&Value::from(
                element.value().attr("content").unwrap_or_default(),
            ));
            if !value.is_empty() {
                return value;
            }
        }
    }
    String::new()
}

fn element_value(document: &scraper::Html, selectors: &[&str]) -> String {
    for selector in selectors {
        let Ok(selector) = scraper::Selector::parse(selector) else {
            continue;
        };
        if let Some(element) = document.select(&selector).next() {
            let value = ["content", "href", "src"]
                .iter()
                .map(|attr| {
                    clean_text(&Value::from(element.value().attr(attr).unwrap_or_default()))
                })
                .chain(std::iter::once(clean_text(&Value::from(
                    element.text().collect::<String>(),
                ))))
                .find(|s| !s.is_empty());
            if let Some(value) = value {
                return value;
            }
        }
    }
    String::new()
}

fn page_lines(document: &scraper::Html) -> Vec<String> {
    static BODY: LazyLock<scraper::Selector> =
        LazyLock::new(|| scraper::Selector::parse("body").expect("body"));
    let Some(body) = document.select(&BODY).next() else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    let mut previous = String::new();
    for element in body.select(&PAGE_LINE_SEL) {
        let text = clean_text(&Value::from(element.text().collect::<String>()));
        if text.is_empty() || text == previous || text.chars().count() > 500 {
            continue;
        }
        previous = text.clone();
        lines.push(text);
    }
    lines
}

fn labeled_value(lines: &[String], labels: &regex::Regex) -> String {
    for (index, line) in lines.iter().enumerate() {
        let Some(capture) = labels.captures(line) else {
            continue;
        };
        let inline = capture
            .get(1)
            .map(|m| clean_text(&Value::from(m.as_str())))
            .unwrap_or_default();
        if !inline.is_empty() {
            return inline;
        }
        return lines
            .get(index + 1)
            .map(|s| clean_text(&Value::from(s.as_str())))
            .unwrap_or_default();
    }
    String::new()
}

fn absolute_http_url(value: &str, base_url: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    let Ok(base) = url::Url::parse(base_url) else {
        return String::new();
    };
    match base.join(value) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => url.to_string(),
        _ => String::new(),
    }
}

fn page_count(value: &str) -> Option<i64> {
    static PAGE_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"\d{1,6}").expect("pages"));
    let number: i64 = PAGE_RE.find(value)?.as_str().parse().ok()?;
    (number > 0 && number <= 100_000).then_some(number)
}

/// `extractBookMetadata` — JSON-LD → meta tags → labeled lines, ISBN
/// normalized, cover resolved against `finalUrl`, `source_url` preserved.
pub fn extract_book_metadata(page: &BookMetadataPage) -> BookMetadata {
    let document = scraper::Html::parse_document(&page.html);
    let records = parse_json_ld(&document);
    let lines = page_lines(&document);
    let json_text = |keys: &[&str]| {
        first_json_value(&records, keys)
            .map(string_from_json_value)
            .unwrap_or_default()
    };

    let title = [
        json_text(&["name", "headline"]),
        element_value(&document, &["main h1", "article h1", "h1"]),
        meta(&document, &["og:title", "twitter:title"]),
    ]
    .into_iter()
    .find(|s| !s.is_empty())
    .unwrap_or_default();
    let author = [
        json_text(&["author", "creator"]),
        element_value(
            &document,
            &[
                "[itemprop=\"author\"]",
                "a[rel=\"author\"]",
                "main h1 + a",
                "main h1 + div a",
                "article h1 + a",
            ],
        ),
        meta(&document, &["author", "book:author"]),
    ]
    .into_iter()
    .find(|s| !s.is_empty())
    .unwrap_or_default();
    let raw_cover = [
        json_text(&["image", "thumbnailUrl", "primaryImageOfPage"]),
        meta(
            &document,
            &["og:image", "twitter:image", "twitter:image:src"],
        ),
        element_value(&document, &["[itemprop=\"image\"]"]),
    ]
    .into_iter()
    .find(|s| !s.is_empty())
    .unwrap_or_default();
    let raw_isbn = [
        json_text(&["isbn"]),
        meta(&document, &["book:isbn", "isbn"]),
        labeled_value(&lines, &label(r"^ISBN\s*[:：]?\s*(.*)$")),
    ]
    .into_iter()
    .find(|s| !s.is_empty())
    .unwrap_or_default();
    let raw_pages = [
        json_text(&["numberOfPages", "pageCount"]),
        meta(&document, &["book:page_count", "numberOfPages"]),
        labeled_value(
            &lines,
            &label(r"^(?:Количество страниц|Страниц|Pages)\s*[:：]?\s*(.*)$"),
        ),
    ]
    .into_iter()
    .find(|s| !s.is_empty())
    .unwrap_or_default();
    let language = normalize_book_language(&Value::from(
        [
            json_text(&["inLanguage", "language"]),
            meta(&document, &["book:language", "language"]),
            labeled_value(&lines, &label(r"^(?:Язык|Language)\s*[:：]?\s*(.*)$")),
        ]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_default(),
    ));
    let publisher = [
        json_text(&["publisher"]),
        meta(&document, &["book:publisher", "publisher"]),
        labeled_value(
            &lines,
            &label(r"^(?:Издательство|Publisher)\s*[:：]?\s*(.*)$"),
        ),
    ]
    .into_iter()
    .find(|s| !s.is_empty())
    .unwrap_or_default();
    let published_date = [
        json_text(&["datePublished", "copyrightYear"]),
        meta(&document, &["book:release_date", "datePublished"]),
        labeled_value(
            &lines,
            &label(r"^(?:Год издания|Дата издания|Published)\s*[:：]?\s*(.*)$"),
        ),
    ]
    .into_iter()
    .find(|s| !s.is_empty())
    .unwrap_or_default();

    let isbn = normalize_isbn(&Value::from(raw_isbn));
    let pages = page_count(&raw_pages);
    let non_empty = |value: String| (!value.is_empty()).then_some(value);
    BookMetadata {
        title: non_empty(title),
        author: non_empty(author),
        cover_image: non_empty(absolute_http_url(&raw_cover, &page.final_url)),
        isbn: non_empty(isbn),
        page_count: pages,
        language: non_empty(language),
        publisher: non_empty(publisher),
        published_date: non_empty(published_date),
        source_url: Some(page.final_url.clone()),
    }
}

fn label(pattern: &str) -> regex::Regex {
    regex::RegexBuilder::new(pattern)
        .case_insensitive(true)
        .build()
        .expect("label regex")
}

#[cfg(test)]
mod tests;
