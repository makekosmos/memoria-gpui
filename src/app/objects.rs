//! `TypeObjectsView` (typed collection table) + `ImageObjectView` ports.
//! Typed fields/books are M5; this view renders label + formatted field
//! values for display (read-only grid).
use gpui::{div, prelude::*, px, Context, SharedString, Window};
use gpui_component::scroll::ScrollableElement;
use serde_json::Value;

use memoria_model::dates::format_readable_russian_date;
use memoria_model::model::{Entry, NoteType};
use memoria_model::object_fields::format_object_field_value;
use memoria_model::object_views::entry_display_title;
use memoria_model::routes::Route;
use memoria_model::system_types_data::{SYSTEM_TYPE_IMAGE_ID, SYSTEM_TYPE_PERSON_ID};

use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;

impl Memoria {
    /// «Коллекция»: all entries of the collection's type + a typed header.
    /// Vue `openTypeCollection(noteTypeId)` — the route carries the *type* id;
    /// the collection entry is only a pointer (`object_type_id`), not required.
    pub(crate) fn render_collection(
        &mut self,
        type_id: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let target = type_id.to_string();
        let nt = self.note_types.iter().find(|n| n.id == target).cloned();
        let entries: Vec<Entry> = self
            .list
            .iter()
            .filter(|e| e.type_id.as_deref() == Some(target.as_str()))
            .cloned()
            .collect();
        let is_image = target == SYSTEM_TYPE_IMAGE_ID;
        let is_person = target == SYSTEM_TYPE_PERSON_ID;

        let mut list_el = div().id("type-objects-list").flex().flex_col();
        if is_image {
            // ImageObjectView gallery grid.
            let mut grid = div().flex().flex_wrap().gap_3();
            for e in &entries {
                grid = grid.child(self.image_tile(e, cx));
            }
            list_el = list_el.child(grid);
        } else {
            list_el = list_el.child(self.collection_table(&entries, nt.as_ref(), is_person, cx));
        }
        if entries.is_empty() {
            list_el = list_el.child(
                div()
                    .pt_6()
                    .text_size(px(13.))
                    .text_color(c(MUTED_FG()))
                    .child("Здесь пока пусто"),
            );
        }

        div()
            .id("type-objects-view")
            .debug_selector(|| "type-objects-view".into())
            .size_full()
            .overflow_y_scrollbar()
            .p(px(36.))
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(c(FG()))
                    .child(memoria_model::note_types::get_note_type_collection_name(
                        nt.as_ref(),
                    )),
            )
            .child(list_el)
    }

    /// Column header + rows: name, then displayed fields, then date columns.
    fn collection_table(
        &mut self,
        entries: &[Entry],
        nt: Option<&NoteType>,
        is_person: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // summaryFields — ≤2 visible non-media fields (TypeObjectsView.vue).
        let fields = nt
            .map(memoria_model::object_views::summary_fields)
            .unwrap_or_default();

        let header_cell = |label: &str, w: gpui::DefiniteLength| {
            div()
                .w(w)
                .px_2()
                .text_size(px(11.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(c(MUTED_FG()))
                .overflow_hidden()
                .whitespace_nowrap()
                .child(label.to_string())
        };

        let mut table = div()
            .flex()
            .flex_col()
            .rounded_lg()
            .border_1()
            .border_color(c(BORDER()))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .bg(rgba(FG(), 0.04))
                    .h(px(30.))
                    .child(header_cell("Название", px(260.).into()))
                    .children(
                        fields
                            .iter()
                            .map(|f| header_cell(&f.field.label, px(140.).into())),
                    )
                    .child(header_cell("Обновлено", px(110.).into())),
            );

        for e in entries {
            let eid = e.id.clone();
            let sel = format!("object-row-{eid}");
            let weak = cx.weak_entity();
            let weak_ctx = cx.weak_entity();
            let ctx_id = eid.clone();
            let mut row = div()
                .id(SharedString::from(format!("row-{eid}")))
                .debug_selector(move || sel.clone())
                .flex()
                .items_center()
                .h(px(34.))
                .border_t_1()
                .border_color(c(BORDER()))
                .cursor_pointer()
                .a11y_button(entry_display_title(e))
                .hover(|s| s.bg(rgba(FG(), 0.05)))
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    let _ =
                        weak.update(cx, |this, cx| this.navigate(Route::Entry(eid.clone()), cx));
                })
                .on_mouse_down(gpui::MouseButton::Right, move |ev, _, cx| {
                    let _ = weak_ctx.update(cx, |this, cx| {
                        this.open_ctx_menu(
                            ctx_id.clone(),
                            f32::from(ev.position.x),
                            f32::from(ev.position.y),
                            cx,
                        );
                    });
                })
                .child(
                    div()
                        .w(px(260.))
                        .px_2()
                        .text_size(px(12.))
                        .text_color(c(FG()))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(if is_person {
                            memoria_model::object_views::person_display_name(e)
                        } else {
                            entry_display_title(e)
                        }),
                );
            let props = memoria_model::object_views::parse_entry_header_props(e);
            for f in &fields {
                let val = props.get(&f.field.id).cloned().unwrap_or(Value::Null);
                let text = format_object_field_value(f, &val);
                row = row.child(
                    div()
                        .w(px(140.))
                        .px_2()
                        .text_size(px(12.))
                        .text_color(c(FG()))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(if text.is_empty() { "—".into() } else { text }),
                );
            }
            row = row.child(
                div()
                    .w(px(110.))
                    .px_2()
                    .text_size(px(11.))
                    .text_color(c(MUTED_FG()))
                    .child(format_readable_russian_date(&Value::from(e.updated_at))),
            );
            table = table.child(row);
        }
        table
    }

    /// Full note route dispatch — image object → ImageObjectView, else
    /// M3 live-preview note editor.
    pub(crate) fn render_entry_view(
        &mut self,
        entry: &Entry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_image = entry.type_id.as_deref() == Some(SYSTEM_TYPE_IMAGE_ID);
        if is_image {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .child(self.render_image_object(entry, cx))
                .into_any_element();
        }
        self.render_note(entry, window, cx).into_any_element()
    }
}
