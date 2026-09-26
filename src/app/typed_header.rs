//! `TypedHeader.vue` port — hero row (cover/avatar + title + description),
//! type badge, secondary property table, and the book metadata button.
//! Field controls live in `prop_field.rs`, the picker overlay in
//! `prop_picker.rs`, book cover/modals in `book.rs`.
use std::collections::{HashMap, HashSet};

use gpui::{div, img, prelude::*, px, Context, SharedString, Window};
use gpui_component::input::Input;
use serde_json::{Map, Value};

use memoria_gpui::model::{Entry, NoteType, ResolvedNoteTypeField};
use memoria_gpui::note_type_fields::get_note_type_presentation;
use memoria_gpui::object_views::parse_entry_header_props;
use memoria_gpui::store::Command;
use memoria_gpui::system_types_data::{
    SYSTEM_TYPE_BOOK_ID, SYSTEM_TYPE_IMAGE_ID, SYSTEM_TYPE_JOURNAL_ID, SYSTEM_TYPE_NOTE_ID,
    SYSTEM_TYPE_PERSON_ID,
};

use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;

/// `hasMeaningfulValue` — a field stays in the header only when it holds data
/// (unless the type always shows empty fields, i.e. non-plain-note types).
pub(crate) fn meaningful(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Array(items)) => !items.is_empty(),
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(f64::is_finite),
        Some(v) => !v.as_str().map(str::trim).unwrap_or("").is_empty(),
        None => false,
    }
}

/// `personDisplayName` — first/last/patronymic joined with spaces.
fn person_display_name(props: &Map<String, Value>) -> String {
    ["first_name", "last_name", "patronymic"]
        .iter()
        .filter_map(|k| props.get(*k).and_then(Value::as_str).map(str::trim))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

impl Memoria {
    /// `displayImageSrc` — resolve a header-prop image value, routing remote
    /// `http(s)` refs through `images.fetch` (Engine) into `image_cache`.
    /// `kosmos-local-image://file/<enc>` decodes back to a plain fs path.
    pub(crate) fn display_image_src(
        &mut self,
        props: &Map<String, Value>,
        field_id: &str,
        cx: &mut Context<Self>,
    ) -> String {
        let entries: HashMap<String, Entry> = self
            .list
            .iter()
            .map(|e| (e.id.clone(), e.clone()))
            .collect();
        let raw = props
            .get(field_id)
            .map(|v| memoria_gpui::object_images::resolve_object_image_src(v, &entries))
            .unwrap_or_default();
        if raw.is_empty() {
            return String::new();
        }
        if let Some(path) = raw.strip_prefix("kosmos-local-image://file/") {
            return memoria_gpui::object_images::percent_decode_path(path);
        }
        if raw.starts_with("http://") || raw.starts_with("https://") {
            if let Some(local) = self.image_cache.get(&raw) {
                return local.clone();
            }
            if self.image_pending.insert(raw.clone()) {
                self.send(Command::FetchImage(raw.clone()), cx);
            }
            return String::new();
        }
        raw
    }

    /// The full typed header (Vue `.typed-object-header` section).
    pub(crate) fn render_typed_header(
        &mut self,
        entry: &Entry,
        note_type: &NoteType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let presentation = get_note_type_presentation(Some(note_type));
        let props = parse_entry_header_props(entry);
        let is_book = note_type.id == SYSTEM_TYPE_BOOK_ID;
        let is_person = note_type.id == SYSTEM_TYPE_PERSON_ID;
        let is_plain = note_type.id == SYSTEM_TYPE_NOTE_ID;
        let image_field_id = presentation.image_field_id.clone();

        // Vue `tableFields` — featured+secondary deduped (games fold all
        // into secondary anyway); plain notes hide empty fields; person/book
        // drop the image field from the table.
        let allow_empty = !is_plain;
        let mut seen = HashSet::new();
        let table_fields: Vec<ResolvedNoteTypeField> = presentation
            .featured_fields
            .iter()
            .chain(presentation.secondary_fields.iter())
            .filter(|f| seen.insert(f.field.id.clone()))
            .filter(|f| allow_empty || meaningful(props.get(&f.field.id)))
            .filter(|f| {
                !((is_person || is_book) && image_field_id.as_deref() == Some(f.field.id.as_str()))
            })
            .cloned()
            .collect();

        let cover_src = image_field_id
            .clone()
            .map(|id| self.display_image_src(&props, &id, cx))
            .unwrap_or_default();
        let description = presentation
            .description_field
            .as_ref()
            .and_then(|f| props.get(&f.field.id))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let person_name = person_display_name(&props);
        let title_text = if is_person && !person_name.is_empty() {
            person_name
        } else if entry.title.trim().is_empty() {
            note_type.name.clone()
        } else {
            entry.title.clone()
        };

        // Hero visual: book → cover button; person → avatar circle; other
        // types with an image field → square thumbnail.
        let visual = if is_book {
            Some(self.book_cover_el(
                &cover_src,
                &title_text,
                image_field_id.clone().unwrap_or_default(),
                &entry.id,
                cx,
            ))
        } else if is_person {
            Some(self.person_avatar_el(&cover_src, image_field_id.as_deref(), entry, cx))
        } else {
            (!cover_src.is_empty()).then(|| {
                img(SharedString::from(cover_src.clone()))
                    .w(px(96.))
                    .h(px(96.))
                    .rounded(px(20.))
                    .object_fit(gpui::ObjectFit::Cover)
                    .into_any_element()
            })
        };

        let mut hero_content = div().flex_1().min_w_0().flex().flex_col().gap_2();
        // Vue `editableTitle` = isBookEntry; person shows composed name,
        // other types keep their title in the outer titlebar.
        if is_book {
            let input =
                self.header_input(&entry.id, "title", &entry.title, "Без названия", window, cx);
            hero_content = hero_content.child(
                div()
                    .id("typed-header-title")
                    .debug_selector(|| "typed-header-title".into())
                    .child(
                        Input::new(&input)
                            .appearance(false)
                            .text_size(px(30.))
                            .h(px(38.)),
                    ),
            );
        } else if is_person {
            hero_content = hero_content.child(
                div()
                    .id("typed-header-title")
                    .debug_selector(|| "typed-header-title".into())
                    .text_size(px(30.))
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(c(FG()))
                    .child(title_text.clone()),
            );
        }
        if let Some(desc_field) = &presentation.description_field {
            if !is_plain || !description.is_empty() {
                let input = self.header_input(
                    &entry.id,
                    &desc_field.field.id,
                    &description,
                    desc_field
                        .field
                        .placeholder
                        .as_deref()
                        .unwrap_or("Краткое описание объекта"),
                    window,
                    cx,
                );
                hero_content = hero_content.child(
                    div()
                        .id("typed-header-description")
                        .debug_selector(|| "typed-header-description".into())
                        .child(
                            Input::new(&input)
                                .appearance(false)
                                .text_size(px(13.))
                                .h(px(28.)),
                        ),
                );
            }
        }

        let mut section = div()
            .id("typed-note-header")
            .debug_selector(|| "typed-note-header".into())
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .id("typed-header-type-badge")
                    .debug_selector(|| "typed-header-type-badge".into())
                    .px_2()
                    .py_1()
                    .rounded_full()
                    .bg(rgba(FG(), 0.06))
                    .border_1()
                    .border_color(c(BORDER()))
                    .text_size(px(10.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(c(MUTED_FG()))
                    .w_auto()
                    .child(note_type.name.clone()),
            );

        let has_hero = is_book || is_person || !cover_src.is_empty();
        let hero = div().flex().flex_row().gap_4().items_start();
        if let Some(v) = visual {
            section = section.child(hero.child(v).child(hero_content));
        } else if has_hero {
            section = section.child(hero.child(hero_content));
        } else {
            section = section.child(hero_content);
        }

        // Secondary property table: «Тип объекта» select + visible fields.
        let type_options: Vec<(String, String)> = self
            .note_types
            .iter()
            .filter(|n| n.id != SYSTEM_TYPE_IMAGE_ID && n.id != SYSTEM_TYPE_JOURNAL_ID)
            .map(|n| (n.id.clone(), n.name.clone()))
            .collect();
        let mut table = div()
            .id("typed-header-fields")
            .debug_selector(|| "typed-header-fields".into())
            .flex()
            .flex_col()
            .child(self.type_row(entry, note_type, type_options, cx));
        for field in &table_fields {
            table = table.child(self.prop_row(entry, field, &props, window, cx));
        }
        section = section.child(table);

        if is_book {
            let weak = cx.weak_entity();
            section = section.child(
                div()
                    .id("book-metadata-btn")
                    .debug_selector(|| "book-metadata-btn".into())
                    .a11y_button("Получить данные")
                    .w_auto()
                    .px_4()
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .rounded_md()
                    .bg(c(FG()))
                    .text_color(c(BG()))
                    .text_size(px(13.))
                    .cursor_pointer()
                    .on_click(move |_, window, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.open_metadata_modal(window, cx);
                        });
                    })
                    .child("Получить данные"),
            );
        }
        section
    }
}
