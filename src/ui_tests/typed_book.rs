//! M5 book-object UI tests — cover modal (URL + OS file drop), book
//! metadata import, plus the dedicated image object view.
#![allow(clippy::missing_panics_doc)]

use gpui::{ExternalPaths, FileDropEvent, PlatformInput, TestAppContext};
use serde_json::Value;

use memoria_gpui::system_types_data::{SYSTEM_TYPE_GAME_ID, SYSTEM_TYPE_IMAGE_ID};

use super::typed::{header_props, seed_entry, seeded_entry};
use super::*;

/// Cover modal: URL save + external file drop both persist through Engine
/// ops (URL direct, drop via `images.storeCover`).
#[gpui::test]
fn cover_modal_url_and_file_drop(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    click(cx, "everything-card-b-1");
    redraw(cx);

    click(cx, "book-cover");
    redraw(cx);
    assert!(cx.debug_bounds("cover-modal").is_some());
    assert!(cx.debug_bounds("cover-dropzone").is_some());

    click(cx, "cover-url-input");
    type_text(cx, "https://example.com/cover.png");
    click(cx, "cover-save");
    redraw(cx);
    assert!(cx.debug_bounds("cover-modal").is_none());
    let props = header_props(&app, cx, "b-1");
    assert_eq!(
        props.get("cover_image").and_then(Value::as_str),
        Some("https://example.com/cover.png")
    );

    // File drop → Command::StoreCover → demo Engine returns the stored path.
    click(cx, "book-cover");
    redraw(cx);
    let pos = cx
        .debug_bounds("cover-dropzone")
        .expect("dropzone")
        .center();
    let paths = ExternalPaths(
        vec![std::path::PathBuf::from("/tmp/cover.png")]
            .into_iter()
            .collect(),
    );
    cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::FileDrop(FileDropEvent::Entered {
                position: pos,
                paths,
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::FileDrop(FileDropEvent::Pending { position: pos }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::FileDrop(FileDropEvent::Submit { position: pos }),
            cx,
        );
        window.dispatch_event(PlatformInput::FileDrop(FileDropEvent::Ended), cx);
    });
    redraw(cx);
    let props = header_props(&app, cx, "b-1");
    assert_eq!(
        props.get("cover_image").and_then(Value::as_str),
        Some("/demo/book-covers/b-1-stored.png"),
        "Engine-stored cover path must land in the image prop"
    );
}

/// Metadata import: ISBN → demo `lookupIsbn` → preview rows → apply writes
/// selected props + title.
#[gpui::test]
fn book_metadata_import_isbn_flow(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    click(cx, "everything-card-b-1");
    redraw(cx);

    click(cx, "book-metadata-btn");
    redraw(cx);
    assert!(cx.debug_bounds("book-metadata-modal").is_some());

    click(cx, "book-metadata-source");
    type_text(cx, "0-306-40615-2");
    click(cx, "book-metadata-find");
    redraw(cx);
    assert!(cx.debug_bounds("book-metadata-preview").is_some());
    assert!(cx.debug_bounds("meta-row-author").is_some());
    assert!(cx.debug_bounds("meta-row-isbn").is_some());

    click(cx, "metadata-apply");
    redraw(cx);
    assert!(cx.debug_bounds("book-metadata-modal").is_none());
    let props = header_props(&app, cx, "b-1");
    assert_eq!(
        props.get("author").and_then(Value::as_str),
        Some("Станислав Лем")
    );
    assert_eq!(
        props.get("isbn").and_then(Value::as_str),
        Some("9780306406157")
    );
    assert_eq!(props.get("page_count").and_then(Value::as_i64), Some(224));
    app.read_with(cx, |a, _| {
        assert_eq!(
            a.current.as_ref().map(|e| e.title.as_str()),
            Some("Солярис")
        );
    });
}

/// Game entry: multi-select genres toggle and persist as an array; a
/// read-only system field renders as plain text without a control.
#[gpui::test]
fn game_fields_multi_select_and_readonly(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    seed_entry(
        &app,
        cx,
        seeded_entry(
            "g-1",
            SYSTEM_TYPE_GAME_ID,
            "Назад в будущее",
            r#"{"genres":["RPG"],"play_status":"in_progress","total_playtime_seconds":3600}"#,
        ),
    );
    click(cx, "everything-card-g-1");
    redraw(cx);
    assert!(cx.debug_bounds("typed-note-header").is_some());
    assert!(cx.debug_bounds("prop-row-genres").is_some());
    // Read-only field — renders its formatted value, no pick control.
    assert!(cx.debug_bounds("prop-row-total_playtime_seconds").is_some());
    assert!(cx
        .debug_bounds("prop-pick-total_playtime_seconds")
        .is_none());

    click(cx, "prop-pick-genres");
    redraw(cx);
    click(cx, "prop-option-Action");
    redraw(cx);
    let props = header_props(&app, cx, "g-1");
    let genres: Vec<&str> = props
        .get("genres")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    assert!(
        genres.contains(&"RPG") && genres.contains(&"Action"),
        "got {genres:?}"
    );
}

/// Image-type entries render the dedicated image object view.
#[gpui::test]
fn image_object_view_renders(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    seed_entry(
        &app,
        cx,
        seeded_entry(
            "i-1",
            SYSTEM_TYPE_IMAGE_ID,
            "Скриншот",
            r#"{"image":"/demo/img.png","file_name":"img.png"}"#,
        ),
    );
    click(cx, "everything-card-i-1");
    redraw(cx);
    assert!(cx.debug_bounds("image-object-view").is_some());
}
