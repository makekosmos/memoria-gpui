//! Diary (M6) — `BubbleDiaryView` port: the view shell (composer card +
//! timeline scroll + optional calendar sidebar). Store ops live in
//! `diary_ops.rs`; per-row markup in `diary_item`, the rich renderer in
//! `diary_blocks`, the sidebar in `diary_calendar`.
use gpui::{div, prelude::*, px, Context, Entity, Focusable, Window};
use gpui_component::Sizable;
use memoria_editor_gpui::MemoriaEditor;
use memoria_model::content::markdown_to_tiptap_doc;
use memoria_model::diary::{bubble_plain_text, parse_bubble_draft};

use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;

impl Memoria {
    pub(crate) fn render_diary(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // `labelNow` re-resolves on every diary render (Vue additionally
        // schedules a midnight timer + visibilitychange — residual GAP:
        // labels can stay stale only while the app sits idle at midnight).
        self.label_now = memoria_model::time::now_millis();
        if let Some(date) = self.diary_jump.take() {
            if let Some(ix) = self.diary_jump_target(&date) {
                self.diary_scroll.scroll_to_item(ix);
            }
        }
        let composer = self.diary_composer_state(window, cx);

        // Each bubble row is a direct child of the tracked scroll element so
        // `scroll_to_item` indexes line up with `self.bubbles` (the 700px
        // centering constraint moves onto the row wrapper).
        let mut rows = Vec::with_capacity(self.bubbles.len());
        for ix in 0..self.bubbles.len() {
            let node = self.bubbles[ix].clone();
            let next = self.bubbles.get(ix + 1);
            let thread_root_id = node.parent_id.clone().unwrap_or_else(|| node.id.clone());
            let continues_thread =
                next.is_some_and(|n| n.parent_id.as_deref() == Some(thread_root_id.as_str()));
            let reply_target = (!continues_thread).then(|| thread_root_id.clone());
            let row = self.diary_row(
                &node,
                &thread_root_id,
                continues_thread,
                reply_target.as_deref(),
                window,
                cx,
            );
            rows.push(
                div()
                    .w_full()
                    .max_w(px(700.))
                    .mx_auto()
                    .child(row)
                    .into_any_element(),
            );
        }

        let timeline = div()
            .id("diary-timeline")
            .debug_selector(|| "diary-timeline".into())
            .role(gpui::Role::Region)
            .aria_label("Лента дневника")
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .track_scroll(&self.diary_scroll)
            .pb(px(96.))
            .children(rows)
            .when(self.bubbles.is_empty() && self.bubbles_loaded, |d| {
                d.child(
                    div()
                        .debug_selector(|| "bubble-timeline-empty".into())
                        .w_full()
                        .max_w(px(700.))
                        .mx_auto()
                        .border_1()
                        .border_color(c(BORDER()))
                        .rounded(px(8.))
                        .py(px(19.))
                        .text_size(px(13.))
                        .text_color(muted_fg_mix(0.92))
                        .flex()
                        .justify_center()
                        .child("Пока нет записей"),
                )
            });

        div()
            .id("diary-view")
            .debug_selector(|| "diary-view".into())
            .role(gpui::Role::Main)
            .aria_label("Дневник")
            .size_full()
            .min_w_0()
            .flex()
            .overflow_hidden()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .gap(px(18.))
                    .pt(px(10.))
                    .child(self.diary_composer_card(composer, cx))
                    .child(timeline),
            )
            .when(self.diary_calendar_open, |d| {
                d.child(self.diary_calendar_sidebar(cx))
            })
    }

    /// `.bubble-composer` card — sticky in Vue; here it sits above the
    /// timeline scroll region (same visual result).
    fn diary_composer_card(
        &self,
        editor: Entity<MemoriaEditor>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let md = editor.read(cx).markdown();
        let doc = markdown_to_tiptap_doc(&md);
        let can_submit = !parse_bubble_draft(&bubble_plain_text(&doc)).0.is_empty();
        div()
            .id("bubble-composer")
            .debug_selector(|| "bubble-composer".into())
            .role(gpui::Role::Region)
            .aria_label("Новая мысль")
            .w_full()
            .max_w(px(700.))
            .mx_auto()
            .min_h(px(114.))
            .flex()
            .flex_col()
            .border_1()
            .border_color(mix(BG(), 0.84, 0xffffff))
            .rounded(px(8.))
            .bg(mix(BG(), 0.92, 0xffffff))
            .shadow_sm()
            .p(px(16.))
            .cursor_text()
            // Vue `focusComposer` — card padding clicks focus the editor.
            .on_click(cx.listener(|this, _, window, cx| {
                if let Some(editor) = &this.diary_composer {
                    editor.read(cx).focus_handle(cx).focus(window, cx);
                }
            }))
            .child(
                div()
                    .id("bubble-composer-editor")
                    .debug_selector(|| "bubble-composer-editor".into())
                    .min_h(px(48.))
                    .flex_1()
                    .pb(px(8.))
                    .child(editor),
            )
            .child(
                div().h(px(32.)).flex().justify_end().child(
                    div()
                        .id("bubble-composer-submit")
                        .debug_selector(|| "bubble-composer-submit".into())
                        .a11y_button("Записать")
                        .size(px(32.))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(can_submit, |d| {
                            d.bg(gpui::white())
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, _, cx| this.submit_diary_draft(cx)))
                        })
                        .when(!can_submit, |d| d.opacity(0.45))
                        .child(
                            gpui_component::Icon::new(gpui::assets::IconName::ArrowUp)
                                .with_size(px(15.))
                                .text_color(c(BG())),
                        ),
                ),
            )
    }
}
