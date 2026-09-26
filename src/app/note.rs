//! Note object view — M3 `MemoriaEditor` live-preview surface embedded in
//! the M4 shell (title input + editor + object chrome) with M5 typed header.
use gpui::{div, prelude::*, px, Context, SharedString, Window};
use gpui_component::input::Input;
use memoria_gpui::dates::format_russian_date_ms;
use memoria_gpui::model::Entry;
use memoria_gpui::sidebar_model::IconId;

use super::{icon, Memoria};
use crate::a11y::A11y;
use crate::theme::*;

impl Memoria {
    /// Note body — typed header (M5) + editable title + live-preview editor.
    pub(crate) fn render_note(
        &mut self,
        entry: &Entry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let note_type = entry
            .type_id
            .as_deref()
            .and_then(|t| self.note_types.iter().find(|nt| nt.id == t))
            .cloned();
        // Vue: book/person render their title inside the typed header
        // (editable for books); other types keep the titlebar title.
        let header_owns_title = note_type
            .as_ref()
            .map(|nt| {
                nt.id == memoria_gpui::system_types_data::SYSTEM_TYPE_BOOK_ID
                    || nt.id == memoria_gpui::system_types_data::SYSTEM_TYPE_PERSON_ID
            })
            .unwrap_or(false);
        let title_state = self
            .title_input
            .clone()
            .expect("title_state prepared in Render::render");
        let editor = self
            .editor
            .clone()
            .expect("editor_state prepared in Render::render");
        let char_count = editor.update(cx, |e, _| e.char_count());

        let mut title_col = div().flex_1().min_w_0().flex().flex_col().gap_1();
        if !header_owns_title {
            title_col = title_col.child(
                Input::new(&title_state)
                    .text_size(px(22.))
                    .text_color(c(FG()))
                    .appearance(false)
                    .bordered(false)
                    .p_0(),
            );
        }
        title_col = title_col.child(
            div()
                .text_size(px(11.))
                .text_color(c(MUTED_FG()))
                .child(format_russian_date_ms(entry.updated_at)),
        );

        let header = div()
            .id("entry-titlebar")
            .flex()
            .items_start()
            .gap_2()
            .child(title_col)
            .child({
                let eid = entry.id.clone();
                let weak = cx.weak_entity();
                div()
                    .id("entry-menu-btn")
                    .debug_selector(|| "entry-menu-btn".into())
                    .a11y_button("Меню объекта")
                    .w(px(28.))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(FG(), 0.08)))
                    .on_click(move |ev, _, cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.open_ctx_menu(
                                eid.clone(),
                                f32::from(ev.position().x),
                                f32::from(ev.position().y),
                                cx,
                            );
                        });
                    })
                    .child(icon(IconId::EllipsisVertical, 15., rgba(FG(), 0.7)))
            });

        let typed_header = note_type.as_ref().map(|nt| {
            self.render_typed_header(entry, nt, window, cx)
                .into_any_element()
        });

        div()
            .id("note-view")
            .debug_selector(|| "note-view".into())
            .size_full()
            .p(px(36.))
            .flex()
            .flex_col()
            .gap_4()
            .child(header)
            .children(typed_header)
            .child(
                div()
                    .id("note-editor")
                    .debug_selector(|| "note-editor".into())
                    .flex_1()
                    .min_h_0()
                    .child(editor),
            )
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .items_center()
                    .child(
                        div()
                            .id("save")
                            .px_3()
                            .py_1()
                            .rounded(px(6.))
                            .bg(c(ACCENT()))
                            .text_color(c(ACCENT_FG()))
                            .cursor_pointer()
                            .text_size(px(13.))
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _, _, cx| this.save(cx)),
                            )
                            .child("Сохранить"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(c(MUTED_FG()))
                            .child(SharedString::from(format!("{char_count} символов"))),
                    )
                    .children(
                        self.status
                            .clone()
                            .map(|s| div().text_size(px(12.)).text_color(c(MUTED_FG())).child(s)),
                    ),
            )
    }
}
