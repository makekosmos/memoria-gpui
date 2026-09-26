//! `ObjectPropertyPicker.vue` port — anchored overlay panel listing field
//! options; backdrop click dismisses. Single-select picks and closes,
//! multi-select toggles in place. Also owns the person avatar hero visual
//! (which is itself a picker over image-type entries).
use gpui::{div, img, prelude::*, px, Context, MouseButton, SharedString, Window};
use gpui_component::scroll::ScrollableElement;
use serde_json::Value;

use memoria_gpui::model::Entry;
use memoria_gpui::object_views::{entry_display_title, parse_entry_header_props};
use memoria_gpui::system_types_data::SYSTEM_TYPE_IMAGE_ID;

use super::types::PropPicker;
use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;

impl Memoria {
    /// Picker overlay — backdrop + anchored panel (Vue `panelStyle` anchors
    /// under the trigger; we anchor at the recorded click position).
    pub(crate) fn render_prop_picker(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let picker = self.prop_picker.clone()?;
        // Keep the panel inside the viewport — hitboxes past the window edge
        // are clipped by the content mask and never receive clicks.
        let viewport = window.viewport_size();
        let panel_x = picker.x.min(f32::from(viewport.width) - 252.).max(8.);
        let panel_y = picker.y.min(f32::from(viewport.height) - 252.).max(8.);
        let mut list = div()
            .flex()
            .flex_col()
            .max_h(px(240.))
            .overflow_y_scrollbar();
        if !picker.multiple {
            let weak = cx.weak_entity();
            list = list.child(
                div()
                    .id("prop-picker-empty")
                    .debug_selector(|| "prop-picker-empty".into())
                    .a11y_menu_item("Без значения")
                    .px_3()
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .rounded_md()
                    .text_size(px(13.))
                    .text_color(rgba(FG(), 0.5))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(FG(), 0.08)))
                    .on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| this.pick_prop_value("", cx));
                    })
                    .child("Без значения"),
            );
        }
        if picker.options.is_empty() {
            list = list.child(
                div()
                    .px_3()
                    .py_2()
                    .text_size(px(12.))
                    .text_color(c(MUTED_FG()))
                    .child("Нет вариантов"),
            );
        }
        for (value, label) in &picker.options {
            let v = value.clone();
            let checked = picker.selected.contains(value);
            let weak = cx.weak_entity();
            list = list.child(
                div()
                    .id(SharedString::from(format!("prop-option-{v}")))
                    .debug_selector({
                        let s = format!("prop-option-{label}");
                        move || s.clone()
                    })
                    .a11y_menu_item(label.clone())
                    .px_3()
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded_md()
                    .text_size(px(13.))
                    .text_color(c(FG()))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(FG(), 0.08)))
                    .on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| this.pick_prop_value(&v, cx));
                    })
                    .when(picker.multiple, |d| {
                        d.child(div().w(px(12.)).text_color(c(ACCENT())).child(if checked {
                            "●"
                        } else {
                            "○"
                        }))
                    })
                    .child(label.clone()),
            );
        }
        Some(
            div()
                .id("prop-picker-backdrop")
                .debug_selector(|| "prop-picker-backdrop".into())
                .absolute()
                .inset_0()
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.prop_picker = None;
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .id("prop-picker-panel")
                        .debug_selector(|| "prop-picker-panel".into())
                        .absolute()
                        .occlude()
                        .left(px(panel_x))
                        .top(px(panel_y))
                        .w(px(240.))
                        .bg(c(SIDEBAR_BG()))
                        .border_1()
                        .border_color(c(BORDER()))
                        .rounded_lg()
                        .p_1()
                        .flex()
                        .flex_col()
                        .child(list),
                ),
        )
    }

    /// Person avatar: image or `+` placeholder; clicking opens the picker
    /// limited to image-type entries (Vue `avatarPickerOptions`).
    pub(crate) fn person_avatar_el(
        &mut self,
        cover_src: &str,
        image_field_id: Option<&str>,
        entry: &Entry,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let mut el = div()
            .id("typed-header-avatar")
            .debug_selector(|| "typed-header-avatar".into())
            .w(px(128.))
            .h(px(128.))
            .rounded_full()
            .bg(rgba(FG(), 0.08))
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center();
        if !cover_src.is_empty() {
            el = el.child(
                img(SharedString::from(cover_src.to_string()))
                    .size_full()
                    .object_fit(gpui::ObjectFit::Cover),
            );
        } else {
            el = el.child(
                div()
                    .text_size(px(30.))
                    .text_color(rgba(FG(), 0.42))
                    .child("+"),
            );
        }
        if let Some(fid) = image_field_id {
            let weak = cx.weak_entity();
            let eid = entry.id.clone();
            let fid = fid.to_string();
            el = el
                .cursor_pointer()
                .on_click(move |ev: &gpui::ClickEvent, _, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        let options: Vec<(String, String)> = this
                            .list
                            .iter()
                            .filter(|e| e.type_id.as_deref() == Some(SYSTEM_TYPE_IMAGE_ID))
                            .map(|e| (e.id.clone(), entry_display_title(e)))
                            .collect();
                        let props = this
                            .current
                            .as_ref()
                            .map(parse_entry_header_props)
                            .unwrap_or_default();
                        let selected = props
                            .get(&fid)
                            .and_then(Value::as_str)
                            .map(|s| vec![s.to_string()])
                            .unwrap_or_default();
                        this.prop_picker = Some(PropPicker {
                            entry_id: eid.clone(),
                            field_id: fid.clone(),
                            multiple: false,
                            options,
                            selected,
                            x: f32::from(ev.position().x),
                            y: f32::from(ev.position().y),
                        });
                        cx.notify();
                    });
                });
        }
        el.into_any_element()
    }
}
