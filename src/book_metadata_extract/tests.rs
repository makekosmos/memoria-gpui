//! Golden extraction tests — the fixtures are verbatim ports of
//! `tests/fixtures/bookMetadataPages.ts` (LiveLib-like + Schema.org pages)
//! with the same expectations as `BookMetadataImportModal.spec.ts`.

use super::*;

fn livelib_page() -> BookMetadataPage {
    BookMetadataPage {
        final_url: "https://www.livelib.ru/book/1000000001-marafon-v-raj-artur-klark".to_string(),
        html: r#"<!doctype html>
<html lang="ru">
  <head>
    <meta property="og:image" content="/storage/covers/marafon-v-raj.jpg">
  </head>
  <body>
    <main>
      <h1>Марафон в рай</h1>
      <a rel="author">Артур Кларк</a>
      <dl>
        <dt>ISBN</dt><dd>978-0-306-40615-7</dd>
        <dt>Количество страниц</dt><dd>352</dd>
        <dt>Язык</dt><dd>Русский</dd>
        <dt>Издательство</dt><dd>АСТ</dd>
        <dt>Год издания</dt><dd>2024</dd>
      </dl>
    </main>
  </body>
</html>"#
            .to_string(),
    }
}

fn schema_org_page() -> BookMetadataPage {
    BookMetadataPage {
        final_url: "https://books.example/library/solaris".to_string(),
        html: r#"<!doctype html>
<html lang="pl">
  <head>
    <script type="application/ld+json">
      {
        "@context": "https://schema.org",
        "@type": "Book",
        "name": "Solaris",
        "author": { "@type": "Person", "name": "Stanisław Lem" },
        "image": { "url": "/covers/solaris.jpg" },
        "isbn": "978-0-306-40615-7",
        "numberOfPages": 224,
        "inLanguage": "pl",
        "publisher": { "@type": "Organization", "name": "Wydawnictwo Literackie" },
        "datePublished": "1961"
      }
    </script>
  </head>
  <body><h1>Fallback title that must not win</h1></body>
</html>"#
            .to_string(),
    }
}

#[test]
fn extracts_livelib_like_page_without_domain_adapter() {
    assert_eq!(
        extract_book_metadata(&livelib_page()),
        BookMetadata {
            title: Some("Марафон в рай".into()),
            author: Some("Артур Кларк".into()),
            cover_image: Some("https://www.livelib.ru/storage/covers/marafon-v-raj.jpg".into()),
            isbn: Some("9780306406157".into()),
            page_count: Some(352),
            language: Some("Русский".into()),
            publisher: Some("АСТ".into()),
            published_date: Some("2024".into()),
            source_url: Some(livelib_page().final_url),
        }
    );
}

#[test]
fn prefers_schema_org_book_metadata() {
    assert_eq!(
        extract_book_metadata(&schema_org_page()),
        BookMetadata {
            title: Some("Solaris".into()),
            author: Some("Stanisław Lem".into()),
            cover_image: Some("https://books.example/covers/solaris.jpg".into()),
            isbn: Some("9780306406157".into()),
            page_count: Some(224),
            language: Some("Польский".into()),
            publisher: Some("Wydawnictwo Literackie".into()),
            published_date: Some("1961".into()),
            source_url: Some(schema_org_page().final_url),
        }
    );
}

#[test]
fn empty_page_yields_only_source_url() {
    let page = BookMetadataPage {
        final_url: "https://x.test/".into(),
        html: "<html><body><p>nothing</p></body></html>".into(),
    };
    let meta = extract_book_metadata(&page);
    assert_eq!(meta.title, None);
    assert_eq!(meta.isbn, None);
    assert!(!crate::book_metadata::has_extracted_book_data(&meta));
    assert_eq!(meta.source_url.as_deref(), Some("https://x.test/"));
}
