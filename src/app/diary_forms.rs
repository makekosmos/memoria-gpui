//! `BubbleTimelineItem` form states — `bubble-card__edit` (textarea +
//! Удалить/Отмена/Обновить), `bubble-thread-actions` («Ответить» on hover),
//! `bubble-reply-draft` (rail stub + textarea + actions).
use gpui::{div, prelude::*, px, Context, Window};
use gpui_component::input::Textarea;
use memoria_model::diary::{parse_bubble_draft, BubbleTimelineNode};

use super::diary_item::{border_strong, small_button};
use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;
use gpui_component::Sizable;

impl Memoria {
    /// `bubble-card__edit` — replaces the read card while `editing_bubble`
    /// points at this node.
    pub(crate) fn diary_edit_form(
        &mut self,
        node: &BubbleTimelineNode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let input = self.edit_input_state(window, cx);
        let node_id = node.id.clone();
        let armed = self.bubble_delete_armed.as_deref() == Some(node_id.as_str());
        let can_commit = !parse_bubble_draft(&input.read(cx).value()).0.is_empty();

        let key_id = node_id.clone();
        let del_id = node_id.clone();
        let upd_id = node_id.clone();
        let sel = format!("bubble-update-{upd_id}");
        let mut update = small_button(
            format!("bubble-update-{upd_id}"),
            move || sel.clone(),
            "Обновить",
            cx,
            move |this, cx| this.commit_edit_bubble(&upd_id, cx),
        )
        .border_color(gpui::transparent_black())
        .bg(c(ACCENT()))
        .text_color(c(ACCENT_FG()))
        .font_weight(gpui::FontWeight::SEMIBOLD);
        if !can_commit {
            update = update.opacity(0.45);
        }

        div()
            .debug_selector(move || format!("bubble-editor-{node_id}"))
            .flex()
            .flex_col()
            .gap(px(8.))
            .min_w_0()
            .on_key_down(cx.listener(move |this, ev: &gpui::KeyDownEvent, _, cx| {
                let k = &ev.keystroke;
                let ctrl = k.modifiers.control || k.modifiers.platform;
                if ctrl && k.key == "enter" {
                    this.commit_edit_bubble(&key_id, cx);
                    cx.stop_propagation();
                } else if k.key == "escape" {
                    this.cancel_edit_bubble(cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .debug_selector({
                        let del_id = del_id.clone();
                        move || format!("bubble-edit-input-{del_id}")
                    })
                    .border_1()
                    .border_color(c(BORDER()))
                    .rounded(px(8.))
                    .bg(rgba(FG(), 0.04))
                    .min_h(px(68.))
                    .child(
                        Textarea::new(&input)
                            .appearance(false)
                            .bordered(false)
                            .h(px(68.)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.4))
                    .child(
                        {
                            let sel = format!("bubble-delete-{del_id}");
                            let click_id = del_id.clone();
                            small_button(
                                format!("bubble-delete-{del_id}"),
                                move || sel.clone(),
                                if armed {
                                    "Точно удалить"
                                } else {
                                    "Удалить"
                                },
                                cx,
                                move |this, cx| this.request_delete_bubble(&click_id, cx),
                            )
                        }
                        .mr_auto()
                        .text_color(mix(DESTRUCTIVE(), 0.78, FG())),
                    )
                    .child(small_button(
                        "bubble-cancel",
                        || "bubble-cancel".into(),
                        "Отмена",
                        cx,
                        |this, cx| this.cancel_edit_bubble(cx),
                    ))
                    .child(update),
            )
    }

    /// `bubble-thread-actions` — «Ответить» pill revealed on thread hover
    /// (`group` shared across all rows of the thread).
    pub(crate) fn diary_thread_actions(
        &self,
        thread_root_id: &str,
        root: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let root = root.to_string();
        let open_id = root.clone();
        let group = format!("thread-{thread_root_id}");
        let sel = format!("bubble-thread-actions-{thread_root_id}");
        div()
            .debug_selector(move || sel.clone())
            .h(px(40.))
            .min_h(px(40.))
            .flex()
            .items_start()
            .mt(px(-10.))
            .pl(px(24.))
            .child(
                div()
                    .id(format!("bubble-reply-{root}"))
                    .debug_selector({
                        let root = root.clone();
                        move || format!("bubble-reply-{root}")
                    })
                    .a11y_button("Ответить")
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .rounded_full()
                    .pl(px(3.))
                    .pr(px(10.))
                    .py(px(2.))
                    .text_size(px(13.5))
                    .text_color(c(MUTED_FG()))
                    .opacity(0.)
                    .group_hover(group, |s| s.opacity(1.))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(FG(), 0.07)).text_color(c(FG())))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.reply_target = Some(open_id.clone());
                        let input = this.reply_input_state(window, cx);
                        input.update(cx, |s, cx| s.set_value("", window, cx));
                        cx.notify();
                    }))
                    .child(
                        gpui_component::Icon::new(gpui::assets::IconName::Reply).with_size(px(15.)),
                    )
                    .child("Ответить"),
            )
    }

    /// `bubble-reply-draft` — rail stub + textarea + Отмена/Ответить.
    pub(crate) fn diary_reply_form(
        &mut self,
        root: &str,
        occurrence_label: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let input = self.reply_input_state(window, cx);
        let root = root.to_string();
        let can_submit = !parse_bubble_draft(&input.read(cx).value()).0.is_empty();
        let key_root = root.clone();
        let submit_root = root.clone();
        let cancel_root = root.clone();
        let mut submit = small_button(
            format!("bubble-reply-submit-{submit_root}"),
            {
                let sel = format!("bubble-reply-submit-{submit_root}");
                move || sel.clone()
            },
            "Ответить",
            cx,
            move |this, cx| this.submit_bubble_reply(&submit_root, cx),
        )
        .border_color(gpui::transparent_black())
        .bg(c(ACCENT()))
        .text_color(c(ACCENT_FG()));
        if !can_submit {
            submit = submit.opacity(0.45);
        }

        div()
            .id(format!("bubble-reply-composer-{root}"))
            .debug_selector({
                let root = root.clone();
                move || format!("bubble-reply-composer-{root}")
            })
            .role(gpui::Role::Group)
            .aria_label(format!("Ответ в ветку {occurrence_label}"))
            .relative()
            .flex()
            .min_w_0()
            .pt(px(4.))
            .on_key_down(cx.listener(move |this, ev: &gpui::KeyDownEvent, _, cx| {
                let k = &ev.keystroke;
                let ctrl = k.modifiers.control || k.modifiers.platform;
                if ctrl && k.key == "enter" {
                    this.submit_bubble_reply(&key_root, cx);
                    cx.stop_propagation();
                }
            }))
            .child(self.thread_line(px(0.), Some(px(18.8)), false))
            .child(
                div()
                    .w(px(20.))
                    .flex_none()
                    .mr(px(12.))
                    .pt(px(4.8))
                    .flex()
                    .justify_center()
                    .child(div().size(px(20.)).rounded_full().bg(border_strong())),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(6.4))
                    .pb(px(15.2))
                    .pr(px(7.2))
                    .child(
                        div()
                            .debug_selector({
                                let root = root.clone();
                                move || format!("bubble-reply-input-{root}")
                            })
                            .border_1()
                            .border_color(c(BORDER()))
                            .rounded(px(6.))
                            .bg(rgba(FG(), 0.04))
                            .min_h(px(54.))
                            .child(
                                Textarea::new(&input)
                                    .appearance(false)
                                    .bordered(false)
                                    .h(px(54.)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(6.4))
                            .child(small_button(
                                format!("bubble-reply-cancel-{cancel_root}"),
                                {
                                    let sel = format!("bubble-reply-cancel-{cancel_root}");
                                    move || sel.clone()
                                },
                                "Отмена",
                                cx,
                                |this, cx| {
                                    this.reply_target = None;
                                    cx.notify();
                                },
                            ))
                            .child(submit),
                    ),
            )
    }
}
