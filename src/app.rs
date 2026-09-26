//! Root shell. M0: a placeholder surface so gates have a real view to compile;
//! navigation and page routing land with the M-tasks in PARITY.md.
use crate::pages::Page;
use crate::store::Engine;
use crate::theme::*;
use gpui::{div, px, Context, IntoElement, ParentElement, Render, Styled, Window};

pub struct Memoria {
    #[allow(dead_code)]
    page: Page,
    #[allow(dead_code)]
    engine: Engine,
}

impl Memoria {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            page: Page::Everything,
            engine: Engine::default(),
        }
    }

    #[allow(dead_code)]
    pub fn page(&self) -> Page {
        self.page
    }
}

impl Render for Memoria {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(c(BG()))
            .text_color(c(FG()))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div().flex().flex_col().gap(px(8.)).child("Memoria").child(
                    div()
                        .text_color(c(MUTED_FG()))
                        .child("Заметки, дневник и знания для Kosmos"),
                ),
            )
    }
}
