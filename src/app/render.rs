//! `impl Render for Memoria` — the M1 debug layout: notes sidebar + plain
//! markdown editor + save button.

use super::*;

impl Render for Memoria {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title_state = self.title_state(window, cx);
        let body_state = self.body_state(window, cx);
        if let Some((title, markdown)) = self.pending_fill.take() {
            title_state.update(cx, |s, cx| s.set_value(title, window, cx));
            body_state.update(cx, |s, cx| s.set_value(markdown, window, cx));
            // `set_value` fires InputEvent::Change — a programmatic fill isn't
            // a user edit.
            self.dirty = false;
        }

        let mut sidebar = div()
            .w(px(280.))
            .flex_shrink_0()
            .h_full()
            .bg(c(SIDEBAR_BG()))
            .border_r_1()
            .border_color(c(BORDER()))
            .flex()
            .flex_col()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_color(c(MUTED_FG()))
                    .text_size(px(12.))
                    .child(SharedString::from(format!("Заметки — {}", self.list.len()))),
            );
        let mut rows = div()
            .id("note-rows")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_1();
        for entry in self.list.clone() {
            rows = rows.child(self.list_row(&entry, cx));
        }
        sidebar = sidebar.child(rows);

        let mut main = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .p_4()
            .gap(px(8.));
        if let Some(banner) = self.banner.clone() {
            main = main.child(
                div()
                    .px_3()
                    .py_2()
                    .rounded(px(6.))
                    .bg(c(CARD()))
                    .text_color(c(WARN()))
                    .text_size(px(12.))
                    .child(banner),
            );
        }
        if self.current.is_some() {
            main = main
                .child(
                    Input::new(&title_state)
                        .text_size(px(20.))
                        .text_color(c(FG()))
                        .appearance(false)
                        .bordered(false)
                        .p_0(),
                )
                .child(
                    Textarea::new(&body_state)
                        .text_size(px(13.))
                        .text_color(c(FG()))
                        .appearance(false)
                        .bordered(false)
                        .flex_1()
                        .h_full(),
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
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| this.save(cx)),
                                )
                                .child("Сохранить"),
                        )
                        .children(
                            self.status.clone().map(|s| {
                                div().text_size(px(12.)).text_color(c(MUTED_FG())).child(s)
                            }),
                        ),
                );
        } else {
            main = main.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(c(MUTED_FG()))
                    .child(if self.busy {
                        "Загрузка…"
                    } else {
                        "Выберите заметку"
                    }),
            );
        }

        div()
            .size_full()
            .bg(c(BG()))
            .text_color(c(FG()))
            .flex()
            .child(sidebar)
            .child(main)
    }
}
