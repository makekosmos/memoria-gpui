//! Port of `tests/systemTypes.test.ts` — book/image system-type contracts.

use memoria_gpui::icon_resolver::object_icon_uri;
use memoria_gpui::note_type_fields::resolve_note_type_fields;
use memoria_gpui::note_type_schemas::{parse_header_template, parse_note_type_definition};
use memoria_gpui::note_types::parse_note_type_ui_schema;
use memoria_gpui::system_types::is_system_type;
use memoria_gpui::system_types_data::{
    system_types, SYSTEM_TYPE_BOOK, SYSTEM_TYPE_BOOK_ID, SYSTEM_TYPE_IMAGE, SYSTEM_TYPE_IMAGE_ID,
};

#[test]
fn book_system_type_matches_the_metadata_contract() {
    assert_eq!(SYSTEM_TYPE_BOOK_ID, "book_obj");
    assert!(is_system_type(SYSTEM_TYPE_BOOK_ID));
    assert!(system_types().iter().any(|t| t.id == SYSTEM_TYPE_BOOK_ID));

    assert_eq!(SYSTEM_TYPE_BOOK.name, "Книга");
    assert_eq!(SYSTEM_TYPE_BOOK.slug, "book_obj");
    assert_eq!(SYSTEM_TYPE_BOOK.icon.as_deref(), Some("book"));
    assert_eq!(SYSTEM_TYPE_BOOK.color, None);

    let definition =
        parse_note_type_definition(&SYSTEM_TYPE_BOOK.schema_json).expect("book schema parses");
    let fields: Vec<(String, String, bool)> = definition
        .fields
        .iter()
        .map(|f| (f.id.clone(), f.kind.clone(), f.required))
        .collect();
    assert_eq!(
        fields,
        [
            ("cover_image", "image", false),
            ("author", "text", false),
            ("isbn", "text", false),
            ("page_count", "number", false),
            ("language", "select", false),
            ("publisher", "text", false),
            ("published_date", "text", false),
            ("source_url", "url", false),
        ]
        .map(|(id, kind, required)| (id.to_string(), kind.to_string(), required))
    );
    let language = definition
        .fields
        .iter()
        .find(|f| f.id == "language")
        .unwrap();
    let options = language.options.clone().unwrap_or_default();
    assert!(options.contains(&"Русский".to_string()));
    assert!(options.contains(&"Английский".to_string()));

    let header =
        parse_header_template(&SYSTEM_TYPE_BOOK.header_template_json).expect("header parses");
    assert_eq!(header.image_field_id.as_deref(), Some("cover_image"));

    let ui = parse_note_type_ui_schema(SYSTEM_TYPE_BOOK.ui_schema_json.as_deref())
        .expect("ui schema parses");
    assert_eq!(
        ui.featured_fields.as_deref(),
        Some(&["author".to_string()][..])
    );
    let expected: Vec<String> = [
        "cover_image",
        "author",
        "isbn",
        "page_count",
        "language",
        "publisher",
        "published_date",
        "source_url",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(ui.visible_fields.as_deref(), Some(&expected[..]));
    assert_eq!(ui.field_order.as_deref(), Some(&expected[..]));
    assert_eq!(ui.collection_name.as_deref(), Some("Книги"));
}

#[test]
fn image_system_type_matches_the_schema_contract() {
    assert_eq!(SYSTEM_TYPE_IMAGE_ID, "image_obj");
    assert!(is_system_type(SYSTEM_TYPE_IMAGE_ID));
    assert!(system_types().iter().any(|t| t.id == SYSTEM_TYPE_IMAGE_ID));

    let definition =
        parse_note_type_definition(&SYSTEM_TYPE_IMAGE.schema_json).expect("image schema parses");
    let ids: Vec<&str> = definition.fields.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "image",
            "file_name",
            "mime_type",
            "size_bytes",
            "width",
            "height",
            "resolution",
            "source_path",
            "alt_text",
        ]
    );
    let image = definition.fields.iter().find(|f| f.id == "image").unwrap();
    assert_eq!(image.kind, "image");
    assert!(image.required);
    assert_eq!(image.visible, Some(true));
    assert_eq!(image.read_only, Some(false));
    let source_path = definition
        .fields
        .iter()
        .find(|f| f.id == "source_path")
        .unwrap();
    assert_eq!(source_path.visible, Some(false));
    assert_eq!(source_path.read_only, Some(true));
    assert_eq!(source_path.system, Some(true));

    let ui = parse_note_type_ui_schema(SYSTEM_TYPE_IMAGE.ui_schema_json.as_deref())
        .expect("ui schema parses");
    assert_eq!(ui.collection_name.as_deref(), Some("Изображения"));
    let visible: Vec<String> = [
        "image",
        "file_name",
        "size_bytes",
        "width",
        "height",
        "resolution",
        "alt_text",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(ui.visible_fields.as_deref(), Some(&visible[..]));
    let hidden: Vec<String> = [
        "created_at",
        "updated_at",
        "deleted_at",
        "mime_type",
        "source_path",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(ui.hidden_fields.as_deref(), Some(&hidden[..]));
    let read_only: Vec<String> = [
        "mime_type",
        "source_path",
        "size_bytes",
        "width",
        "height",
        "resolution",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(ui.read_only_fields.as_deref(), Some(&read_only[..]));

    let resolved = resolve_note_type_fields(Some(&SYSTEM_TYPE_IMAGE));
    let image = resolved.iter().find(|f| f.field.id == "image").unwrap();
    assert!(image.visible);
    assert!(!image.read_only);
    let mime = resolved.iter().find(|f| f.field.id == "mime_type").unwrap();
    assert!(!mime.visible);
    assert!(mime.read_only);
    let resolution = resolved
        .iter()
        .find(|f| f.field.id == "resolution")
        .unwrap();
    assert!(resolution.visible);
    assert!(resolution.read_only);
}

#[test]
fn image_icon_resolves_to_dedicated_svg() {
    let uri = object_icon_uri(Some("image"));
    let svg = uri
        .split(',')
        .nth(1)
        .map(|part| {
            // `decodeURIComponent` equivalent for the resolver output.
            percent_decode(part)
        })
        .unwrap_or_default();
    assert!(svg.contains("<svg"));
    assert!(svg.contains("<rect"));
    assert!(svg.contains("<circle"));
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
            if let Ok(b) = u8::from_str_radix(hex, 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
