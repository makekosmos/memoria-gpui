//! `BubbleTimelineItem` port — one flat-list row: rail (kind dot + dropdown),
//! card (rich text + time edit affordance + tags). Edit/reply forms and the
//! «Ответить» affordance live in `diary_forms.rs`; shared chrome in this file.
use gpui::{div, prelude::*, px, Context, Window};
use memoria_model::diary::{
    format_bubble_occurrence_label, node_occurrence, BubbleKind, BubbleTimelineNode,
};

use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;

/// `var(--border-color-strong, var(--border))` — the strong token isn't a
/// theme export, so the plain `border` fallback applies.
pub(crate) fn border_strong() -> gpui::Hsla {
    c(BORDER())
}

pub(crate) fn kind_dot_color(kind: BubbleKind) -> gpui::Hsla {
    kind.color().map(c).unwrap_or_else(border_strong)
}

/// Small bordered action button (`bubble-card__edit-actions` / reply chrome).
pub(crate) fn small_button(
    id: impl Into<gpui::ElementId>,
    selector: impl Fn() -> String + 'static,
    label: &'static str,
    cx: &mut Context<Memoria>,
    f: impl Fn(&mut Memoria, &mut Context<Memoria>) + 'static,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .debug_selector(selector)
        .a11y_button(label)
        .min_h(px(28.))
        .px(px(9.6))
        .flex()
        .items_center()
        .border_1()
        .border_color(c(BORDER()))
        .rounded(px(6.))
        .text_size(px(12.))
        .text_color(c(FG()))
        .cursor_pointer()
        .hover(|s| s.bg(rgba(FG(), 0.07)))
        .on_click(cx.listener(move |this, _, _, cx| f(this, cx)))
}

impl Memoria {
    /// Absolute rail thread segment (`--reply::before` /
    /// `--continues-thread::after` / `--replying::before`).
    pub(crate) fn thread_line(
        &self,
        top: gpui::Pixels,
        height: Option<gpui::Pixels>,
        to_bottom: bool,
    ) -> gpui::Div {
        let mut line = div().absolute().left(px(9.)).top(top).w(px(2.));
        if let Some(h) = height {
            line = line.h(h);
        }
        if to_bottom {
            line = line.bottom_0();
        }
        line.bg(border_strong())
    }

    /// One `.bubble-timeline__virtual-row`: article + thread affordance.
    /// `continues_thread`/`reply_target` are computed by `render_diary` from
    /// the flat list (`nextNode.parentId === threadRootId`).
    pub(crate) fn diary_row(
        &mut self,
        node: &BubbleTimelineNode,
        thread_root_id: &str,
        continues_thread: bool,
        reply_target: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let node_id = node.id.clone();
        let replying_here = reply_target.is_some() && self.reply_target.as_deref() == reply_target;
        let occurrence_label =
            format_bubble_occurrence_label(Some(&node_occurrence(node)), self.label_now);

        let mut article = div()
            .id(format!("bubble-node-{}", node.id))
            .debug_selector(move || format!("bubble-node-{node_id}"))
            .role(gpui::Role::Group)
            .aria_label(format!("Запись {occurrence_label}"))
            .group(format!("thread-{thread_root_id}"))
            .relative()
            .flex()
            .min_w_0()
            .pt(px(4.));

        if node.parent_id.is_some() {
            article = article.child(self.thread_line(px(0.), Some(px(18.8)), false));
        }
        if continues_thread || replying_here {
            article = article.child(self.thread_line(px(18.8), None, true));
        }

        article = article
            .child(self.diary_rail(node, cx))
            .child(self.diary_card(node, window, cx));

        let mut row = div().flex().flex_col().min_w_0().child(article);
        if let Some(root) = reply_target {
            row = if replying_here {
                row.child(self.diary_reply_form(root, &occurrence_label, window, cx))
            } else {
                row.child(self.diary_thread_actions(thread_root_id, root, cx))
            };
        }
        row
    }

    /// `.bubble-timeline-item__content` — card body or the edit form.
    fn diary_card(
        &mut self,
        node: &BubbleTimelineNode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let editing = self.editing_bubble.as_deref() == Some(node.id.as_str());
        div().flex_1().min_w_0().pb(px(15.2)).child(
            div()
                .rounded(px(8.))
                .pt(px(4.))
                .pr(px(7.2))
                .pb(px(5.6))
                .flex()
                .flex_col()
                .min_w_0()
                .child(if editing {
                    self.diary_edit_form(node, window, cx).into_any_element()
                } else {
                    self.diary_card_body(node, cx).into_any_element()
                }),
        )
    }

    /// `bubble-card__line` + `bubble-card__tags` — read mode.
    fn diary_card_body(
        &self,
        node: &BubbleTimelineNode,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = node.id.clone();
        let label = format_bubble_occurrence_label(Some(&node_occurrence(node)), self.label_now);
        let node_for_edit = node.clone();
        let mut body = div().flex().flex_col().min_w_0().child(
            div()
                .flex()
                .items_baseline()
                .gap(px(12.))
                .min_w_0()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(15.))
                        .text_color(rgba(FG(), 0.92))
                        .children(self.render_bubble_blocks(node)),
                )
                .child(
                    div()
                        .id(format!("bubble-time-{}", node.id))
                        .debug_selector(move || format!("bubble-time-{id}"))
                        .a11y_button("Редактировать запись")
                        .flex_none()
                        .ml_auto()
                        .rounded(px(6.))
                        .px(px(5.1))
                        .py(px(2.9))
                        .font_family("Berkeley Mono")
                        .text_size(px(11.5))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgba(FG(), 0.46))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(FG(), 0.07)).text_color(rgba(FG(), 0.82)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.start_edit_bubble(&node_for_edit, window, cx);
                        }))
                        .child(label),
                ),
        );

        if !node.tags.is_empty() {
            let mut tags = div()
                .debug_selector(move || format!("bubble-tags-{}", node.id))
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(5.6))
                .mt(px(8.));
            let accent = node.kind.color().unwrap_or(BORDER());
            for tag in &node.tags {
                tags = tags.child(
                    div()
                        .border_1()
                        .border_color(mix(accent, 0.30, BORDER()))
                        .rounded_full()
                        .bg(rgba(accent, 0.09))
                        .px(px(7.7))
                        .py(px(1.9))
                        .text_size(px(11.5))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgba(FG(), 0.86))
                        .child(tag.clone()),
                );
            }
            body = body.child(tags);
        }
        body
    }
}
