//! `ObjectPropertyField.vue` port — per-kind controls for the typed-header
//! property table (text input, checkbox, pick row) plus the «Тип объекта»
//! row. The commit/persist plumbing lives in `prop_edit.rs`.
use gpui::{div, prelude::*, px, Context, SharedString, Window};
use gpui_component::input::Input;
use serde_json::{Map, Value};

use memoria_model::model::{Entry, NoteType, ResolvedNoteTypeField};
use memoria_model::object_fields::format_object_field_value;

use super::types::PropPicker;
use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;

impl Memoria {
    /// One secondary-table row: label + per-kind control.
    pub(crate) fn prop_row(
        &mut self,
        entry: &Entry,
        field: &ResolvedNoteTypeField,
        props: &Map<String, Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let value = props.get(&field.field.id).cloned().unwrap_or(Value::Null);
        let display = format_object_field_value(field, &value);
        let fid = field.field.id.clone();
        let eid = entry.id.clone();

        let control: gpui::AnyElement = if field.read_only {
            div()
                .text_size(px(13.))
                .text_color(c(FG()))
                .child(if display.is_empty() {
                    "—".into()
                } else {
                    display
                })
                .into_any_element()
        } else {
            match field.field.kind.as_str() {
                "boolean" => self.bool_control(
                    &eid,
                    &fid,
                    field,
                    value.as_bool().unwrap_or(false),
                    &display,
                    cx,
                ),
                "select" | "multi_select" | "relation" => {
                    self.pick_control(entry, field, &display, cx)
                }
                _ => {
                    // text | long_text | number | date | url | image → input.
                    let input = self.header_input(
                        &eid,
                        &fid,
                        value.as_str().unwrap_or_default(),
                        field
                            .field
                            .placeholder
                            .as_deref()
                            .unwrap_or("Введите значение"),
                        window,
                        cx,
                    );
                    div()
                        .id(SharedString::from(format!("prop-input-{fid}")))
                        .debug_selector({
                            let s = format!("prop-input-{fid}");
                            move || s.clone()
                        })
                        .child(
                            Input::new(&input)
                                .appearance(false)
                                .text_size(px(13.))
                                .h(px(28.)),
                        )
                        .into_any_element()
                }
            }
        };

        div()
            .id(SharedString::from(format!("prop-row-{eid}-{fid}")))
            .debug_selector({
                let s = format!("prop-row-{fid}");
                move || s.clone()
            })
            .flex()
            .items_center()
            .gap_3()
            .py_1()
            .child(
                div()
                    .w(px(180.))
                    .flex_shrink_0()
                    .text_size(px(12.))
                    .text_color(c(MUTED_FG()))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(field.field.label.clone()),
            )
            .child(div().flex_1().min_w_0().child(control))
    }

    /// «Тип объекта» row — read display + opens the type picker.
    pub(crate) fn type_row(
        &self,
        entry: &Entry,
        note_type: &NoteType,
        options: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let weak = cx.weak_entity();
        let eid = entry.id.clone();
        let selected = vec![note_type.id.clone()];
        div()
            .id("prop-row-object-type")
            .debug_selector(|| "prop-row-object-type".into())
            .flex()
            .items_center()
            .gap_3()
            .py_1()
            .child(
                div()
                    .w(px(180.))
                    .flex_shrink_0()
                    .text_size(px(12.))
                    .text_color(c(MUTED_FG()))
                    .child("Тип объекта"),
            )
            .child(
                div()
                    .id("prop-pick-object-type")
                    .debug_selector(|| "prop-pick-object-type".into())
                    .a11y_button("Тип объекта")
                    .flex_1()
                    .px_2()
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(FG(), 0.06)))
                    .on_click(move |ev: &gpui::ClickEvent, _, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.prop_picker = Some(PropPicker {
                                entry_id: eid.clone(),
                                field_id: "__object_type".into(),
                                multiple: false,
                                options: options.clone(),
                                selected: selected.clone(),
                                x: f32::from(ev.position().x),
                                y: f32::from(ev.position().y),
                            });
                            cx.notify();
                        });
                    })
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(c(FG()))
                            .child(note_type.name.clone()),
                    ),
            )
    }

    /// Boolean kind — styled checkbox toggling `headerProps[field]` (Vue
    /// `updateBooleanValue`).
    fn bool_control(
        &mut self,
        eid: &str,
        fid: &str,
        field: &ResolvedNoteTypeField,
        on: bool,
        display: &str,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let weak = cx.weak_entity();
        let fid = fid.to_string();
        div()
            .id(SharedString::from(format!("prop-check-{eid}-{fid}")))
            .debug_selector({
                let s = format!("prop-check-{fid}");
                move || s.clone()
            })
            .a11y_switch(field.field.label.clone(), on)
            .flex()
            .items_center()
            .gap_2()
            .cursor_pointer()
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.set_header_prop(&fid, Value::from(!on), cx);
                });
            })
            .child(
                div()
                    .w(px(16.))
                    .h(px(16.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(c(BORDER()))
                    .bg(if on { c(ACCENT()) } else { c(BG()) })
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(11.))
                    .text_color(c(BG()))
                    .child(if on { "✓" } else { "" }),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(c(FG()))
                    .child(display.to_string()),
            )
            .into_any_element()
    }

    /// Select/multi_select/relation — summary row opening the picker (Vue
    /// `ObjectPropertyPicker` trigger).
    fn pick_control(
        &mut self,
        entry: &Entry,
        field: &ResolvedNoteTypeField,
        display: &str,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let f = field.clone();
        let e = entry.clone();
        let weak = cx.weak_entity();
        let text = if display.is_empty() {
            field
                .field
                .placeholder
                .clone()
                .unwrap_or_else(|| "Выбрать вариант".into())
        } else {
            display.to_string()
        };
        let color = if display.is_empty() {
            rgba(FG(), 0.4)
        } else {
            c(FG())
        };
        div()
            .id(SharedString::from(format!(
                "prop-pick-{}-{}",
                entry.id, field.field.id
            )))
            .debug_selector({
                let s = format!("prop-pick-{}", field.field.id);
                move || s.clone()
            })
            .a11y_button(field.field.label.clone())
            .w_full()
            .px_2()
            .h(px(28.))
            .flex()
            .items_center()
            .rounded_md()
            .cursor_pointer()
            .hover(|s| s.bg(rgba(FG(), 0.06)))
            .on_click(move |ev: &gpui::ClickEvent, _, cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.open_prop_picker(
                        &e,
                        &f,
                        f32::from(ev.position().x),
                        f32::from(ev.position().y),
                        cx,
                    );
                });
            })
            .child(div().text_size(px(13.)).text_color(color).child(text))
            .into_any_element()
    }
}
