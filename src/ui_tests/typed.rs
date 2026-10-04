//! M5 typed-object UI tests — typed header, property edits, the option
//! picker, cover modal (URL + OS file drop), and book-metadata import, all
//! against the in-memory demo Engine stand-in.
#![allow(clippy::missing_panics_doc)]

use gpui::TestAppContext;
use serde_json::Value;

use memoria_model::model::Entry;
use memoria_model::object_views::parse_entry_header_props;
use memoria_model::routes::Route;

use super::*;

/// `header_props_json` of `id` as the app currently knows it.
pub(super) fn header_props(
    app: &Entity<Memoria>,
    cx: &VisualTestContext,
    id: &str,
) -> serde_json::Map<String, Value> {
    app.read_with(cx, |a, _| {
        a.list
            .iter()
            .find(|e| e.id == id)
            .map(parse_entry_header_props)
            .unwrap_or_default()
    })
}

/// Push a pre-made entry into the demo Engine and refresh the list.
pub(super) fn seed_entry(app: &Entity<Memoria>, cx: &mut VisualTestContext, entry: Entry) {
    app.update(cx, |a, cx| {
        if let crate::app::Backend::Demo(store) = &mut a.backend {
            store.entries.push(entry);
        }
        a.send(memoria_model::store::Command::LoadList(Vec::new()), cx);
    });
    redraw(cx);
}

pub(super) fn seeded_entry(id: &str, type_id: &str, title: &str, props_json: &str) -> Entry {
    Entry {
        id: id.into(),
        title: title.into(),
        content_json: memoria_model::content::write_entry_markdown("тело").to_string(),
        content_loaded: Some(true),
        type_id: Some(type_id.into()),
        header_props_json: Some(props_json.into()),
        ..Default::default()
    }
}

/// A book renders the typed header: cover button, editable title, prop rows.
/// Editing a text prop persists through `SaveEntry` (demo Engine).
#[gpui::test]
fn book_header_prop_edit_saves(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    click(cx, "everything-card-b-1");
    assert_eq!(route_of(cx, &app), Route::Entry("b-1".into()));
    redraw(cx);

    assert!(cx.debug_bounds("typed-note-header").is_some());
    assert!(cx.debug_bounds("typed-header-type-badge").is_some());
    assert!(cx.debug_bounds("book-cover").is_some());
    assert!(cx.debug_bounds("typed-header-title").is_some());
    assert!(cx.debug_bounds("prop-row-author").is_some());
    assert!(cx.debug_bounds("prop-row-language").is_some());
    assert!(cx.debug_bounds("prop-pick-object-type").is_some());
    assert!(cx.debug_bounds("book-metadata-btn").is_some());

    click(cx, "prop-input-author");
    // The seed already sets "Лев Толстой" — clear the input (select-all is
    // not keymap-bound under the test platform) before typing the new value.
    cx.update(|window, cx| {
        let state = app
            .read(cx)
            .header_inputs
            .get("author")
            .cloned()
            .expect("author input state");
        state.update(cx, |s, cx| s.set_value(String::new(), window, cx));
    });
    type_text(cx, "Лев Николаевич Толстой");
    cx.simulate_keystrokes("enter");
    redraw(cx);
    let props = header_props(&app, cx, "b-1");
    assert_eq!(
        props.get("author").and_then(Value::as_str),
        Some("Лев Николаевич Толстой"),
        "author prop must persist via SaveEntry"
    );
}

/// Select-kind prop opens the option picker; picking a value persists.
#[gpui::test]
fn prop_picker_select_persists(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    click(cx, "everything-card-b-1");
    redraw(cx);

    click(cx, "prop-pick-language");
    redraw(cx);
    assert!(cx.debug_bounds("prop-picker-panel").is_some());
    click(cx, "prop-option-Русский");
    redraw(cx);
    assert!(cx.debug_bounds("prop-picker-panel").is_none());
    let props = header_props(&app, cx, "b-1");
    assert_eq!(
        props.get("language").and_then(Value::as_str),
        Some("Русский")
    );
}

/// «Тип объекта» row opens the type picker; picking a type re-types the
/// entry and resets header props (Vue `objectTypeChange`).
#[gpui::test]
fn object_type_picker_retypes_entry(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    click(cx, "everything-card-n-1");
    redraw(cx);
    // Plain notes hide empty fields — only the type row shows.
    assert!(cx.debug_bounds("prop-row-object-type").is_some());
    assert!(cx.debug_bounds("prop-row-related_notes").is_none());

    click(cx, "prop-pick-object-type");
    redraw(cx);
    click(cx, "prop-option-Книга");
    redraw(cx);
    app.read_with(cx, |a, _| {
        assert_eq!(
            a.current.as_ref().and_then(|e| e.type_id.as_deref()),
            Some("book_obj")
        );
    });
    assert!(cx.debug_bounds("book-cover").is_some());
}

/// Custom type whose schema exposes a visible relation field — system types
/// keep `related_notes`/`photo` hidden or in the hero (Vue puts note links
/// in their own section), so a custom type exercises the relation picker.
fn rel_type() -> memoria_model::model::NoteType {
    memoria_model::model::NoteType {
        id: "rel_test_obj".into(),
        name: "Тип со связями".into(),
        slug: "rel-test".into(),
        icon: None,
        color: None,
        schema_json: r#"{"fields":[{"id":"related","label":"Связи","kind":"relation","required":false,"visible":true,"read_only":false,"link_type":"related","multiple":true,"system":false}]}"#
            .into(),
        header_template_json: r#"{"kind":"default","primaryFieldIds":[],"secondaryFieldIds":[],"imageFieldId":null}"#.into(),
        ui_schema_json: Some(r#"{"featured_fields":[],"visible_fields":["related"],"hidden_fields":["created_at","updated_at","deleted_at"],"read_only_fields":[],"field_order":["related"],"header_layout":"inline","default_layout":"page","default_template_id":null,"collection_name":"Связанные"}"#.into()),
        created_at: 0,
        updated_at: 0,
        extra: Default::default(),
    }
}

/// Relation prop with a value shows its row; the picker lists candidate
/// entries and toggling adds the relation (multi by default).
#[gpui::test]
fn relation_picker_toggles_entries(cx: &mut TestAppContext) {
    let (app, cx) = launch(cx);
    redraw(cx);
    app.update(cx, |a, _| a.note_types.push(rel_type()));
    seed_entry(
        &app,
        cx,
        seeded_entry("r-1", "rel_test_obj", "Связанная", r#"{"related":["n-2"]}"#),
    );
    click(cx, "everything-card-r-1");
    redraw(cx);
    assert!(cx.debug_bounds("prop-row-related").is_some());

    click(cx, "prop-pick-related");
    redraw(cx);
    assert!(cx.debug_bounds("prop-picker-panel").is_some());
    click(cx, "prop-option-Война и мир");
    redraw(cx);
    let props = header_props(&app, cx, "r-1");
    let related: Vec<&str> = props
        .get("related")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    assert!(
        related.contains(&"n-2") && related.contains(&"b-1"),
        "got {related:?}"
    );
    // Multi-select stays open after a pick.
    assert!(cx.debug_bounds("prop-picker-panel").is_some());
}
