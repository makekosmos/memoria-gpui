//! `BookMetadataImportModal.vue` reply + apply flow — `lookupIsbn` /
//! `fetchPage` replies fill the modal preview (with the ISBN enrichment
//! pass), `apply` writes selected header props + title via `SaveEntry`.
use gpui::Context;
use serde_json::Value;

use memoria_model::book_metadata::{
    book_metadata_field_value, fill_missing_book_metadata, has_extracted_book_data,
    is_empty_book_value, normalize_isbn, BookMetadata, BookMetadataPage,
};
use memoria_model::object_views::parse_entry_header_props;
use memoria_model::store::Command;

use super::metadata_modal::FIELD_LABELS;
use super::Memoria;

/// Vue default selection — every non-empty preview field gets checked.
fn preview_selected(m: &BookMetadata) -> Vec<String> {
    FIELD_LABELS
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !is_empty_book_value(&book_metadata_field_value(m, k)))
        .map(str::to_string)
        .collect()
}

impl Memoria {
    /// `loadMetadata` — ISBN-shaped source → `lookupIsbn`, else a public
    /// HTTPS URL → `fetchPage` (Vue `isValidSource`).
    pub(crate) fn metadata_load(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = self.metadata_modal.as_mut() else {
            return;
        };
        if modal.loading {
            return;
        }
        let source = modal.source_input.read(cx).value().trim().to_string();
        let isbn_like = source
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, 'X' | 'x' | ' ' | '-'));
        let isbn = if isbn_like && !source.is_empty() {
            normalize_isbn(&Value::from(source.clone()))
        } else {
            String::new()
        };
        if isbn.is_empty() && !source.starts_with("https://") {
            modal.error = Some("Вставь публичную HTTPS-ссылку или корректный ISBN".into());
            return;
        }
        modal.loading = true;
        modal.error = None;
        modal.metadata = None;
        modal.selected.clear();
        modal.enrich_isbn = None;
        if !isbn.is_empty() {
            self.send(Command::LookupIsbn(isbn), cx);
        } else {
            self.send(Command::FetchBookPage(source), cx);
        }
        cx.notify();
    }

    /// `lookupIsbn` reply — direct ISBN answer, or the enrichment pass when
    /// `enrich_isbn` is set (Vue `fillMissingBookMetadata` merge).
    pub(crate) fn on_book_metadata(
        &mut self,
        result: Result<Option<BookMetadata>, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(modal) = self.metadata_modal.as_mut() else {
            return;
        };
        if modal.enrich_isbn.take().is_some() {
            if let (Some(base), Ok(Some(extra))) = (modal.metadata.clone(), result) {
                let merged = fill_missing_book_metadata(&base, &extra);
                modal.metadata = Some(merged.clone());
                modal.selected = preview_selected(&merged);
            }
            modal.loading = false;
            cx.notify();
            return;
        }
        match result {
            Ok(Some(metadata)) => {
                modal.metadata = Some(metadata.clone());
                modal.selected = preview_selected(&metadata);
            }
            Ok(None) => modal.error = Some("Книга с таким ISBN не найдена".into()),
            Err(e) => modal.error = Some(e),
        }
        modal.loading = false;
        cx.notify();
    }

    /// `fetchPage` reply — extract via the ported `extractBookMetadata`, then
    /// kick the ISBN enrichment lookup when the page carried one.
    pub(crate) fn on_book_metadata_page(
        &mut self,
        result: Result<Option<BookMetadataPage>, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(modal) = self.metadata_modal.as_mut() else {
            return;
        };
        match result {
            Ok(Some(page)) => {
                let extracted = memoria_model::book_metadata_extract::extract_book_metadata(&page);
                if let Some(isbn) = extracted.isbn.clone().filter(|s| !s.is_empty()) {
                    modal.metadata = Some(extracted);
                    modal.enrich_isbn = Some(isbn.clone());
                    self.send(Command::LookupIsbn(isbn), cx);
                    return; // stays loading until enrichment lands
                }
                if !has_extracted_book_data(&extracted) {
                    modal.error =
                        Some("На этой странице не удалось распознать данные книги".into());
                } else {
                    modal.metadata = Some(extracted.clone());
                    modal.selected = preview_selected(&extracted);
                }
                modal.loading = false;
            }
            Ok(None) => {
                modal.error = Some("Не удалось загрузить страницу. Проверь ссылку".into());
                modal.loading = false;
            }
            Err(e) => {
                modal.error = Some(e);
                modal.loading = false;
            }
        }
        cx.notify();
    }

    /// `apply` — write selected metadata into header props (+title) and save
    /// (Vue `handleBookMetadataApply`).
    pub(crate) fn metadata_apply(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = self.metadata_modal.take() else {
            return;
        };
        let Some(metadata) = modal.metadata else {
            return;
        };
        let Some(entry) = self.current.as_mut() else {
            return;
        };
        if entry.id != modal.entry_id {
            return;
        }
        let mut props = parse_entry_header_props(entry);
        let mut changed = false;
        for key in &modal.selected {
            let incoming = book_metadata_field_value(&metadata, key);
            if is_empty_book_value(&incoming) {
                continue;
            }
            if key == "title" {
                let clean = incoming.as_str().unwrap_or("").replace(['\r', '\n'], " ");
                if entry.title != clean {
                    entry.title = clean;
                    changed = true;
                }
            } else if props.get(key) != Some(&incoming) {
                props.insert(key.clone(), incoming);
                changed = true;
            }
        }
        if !changed {
            return;
        }
        entry.header_props_json = Some(serde_json::to_string(&props).unwrap_or_default());
        let updated = entry.clone();
        if let Some(row) = self.list.iter_mut().find(|e| e.id == updated.id) {
            *row = updated.clone();
        }
        self.send(Command::SaveEntry(Box::new(updated)), cx);
        cx.notify();
    }
}
