//! `BubbleTimelineItem` rail — the kind dot + its kind dropdown. The open
//! menu returns separately so `diary_row` can mount it on `article` (paint
//! order above the card).
use gpui::{div, prelude::*, px, Context};
use memoria_gpui::diary::{BubbleKind, BubbleTimelineNode};
use memoria_gpui::store::{BubblePatch, Command};

use super::diary_item::kind_dot_color;
use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;

impl Memoria {
    /// `.bubble-timeline-item__rail` — kind dot; the open dropdown is
    /// returned separately so it can mount on `article` (paint order).
    pub(crate) fn diary_rail(
        &self,
        node: &BubbleTimelineNode,
        cx: &mut Context<Self>,
    ) -> (gpui::Div, Option<gpui::Stateful<gpui::Div>>) {
        let menu_open = self.kind_menu_for.as_deref() == Some(node.id.as_str());
        let id = node.id.clone();
        let rail = div()
            .w(px(20.))
            .flex_none()
            .mr(px(12.))
            .pt(px(4.8))
            .relative()
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .id(format!("bubble-kind-{id}"))
                    .debug_selector({
                        let id = id.clone();
                        move || format!("bubble-kind-{id}")
                    })
                    .a11y_button(format!("Тип записи: {}", node.kind.label()))
                    .size(px(20.))
                    .rounded_full()
                    .bg(kind_dot_color(node.kind))
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.86))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.kind_menu_for = if this.kind_menu_for.as_deref() == Some(id.as_str()) {
                            None
                        } else {
                            Some(id.clone())
                        };
                        cx.stop_propagation();
                        cx.notify();
                    })),
            );

        if menu_open {
            let mut menu = div()
                .id(format!("kind-menu-{}", node.id))
                .debug_selector(|| "bubble-kind-menu".into())
                .absolute()
                .top(px(28.))
                .left_0()
                .flex()
                .flex_col()
                .min_w(px(140.))
                .py(px(4.))
                .bg(c(POPOVER()))
                .border_1()
                .border_color(c(BORDER()))
                .rounded(px(8.))
                .shadow_lg()
                .occlude()
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation());
            for kind in BubbleKind::OPTIONS {
                let node_id = node.id.clone();
                let sel = format!("bubble-kind-option-{}", kind.as_str());
                menu = menu.child(
                    div()
                        .id(format!("bubble-kind-option-{}-{}", node.id, kind.as_str()))
                        .debug_selector(move || sel.clone())
                        .a11y_menu_item(kind.label())
                        .flex()
                        .items_center()
                        .gap_2()
                        .px(px(10.))
                        .py(px(5.))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(FG(), 0.07)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.kind_menu_for = None;
                            this.send_bubble_write(
                                Command::UpdateBubble {
                                    id: node_id.clone(),
                                    patch: BubblePatch {
                                        input: None,
                                        kind: Some(kind),
                                    },
                                },
                                cx,
                            );
                        }))
                        .child(
                            div()
                                .size(px(14.))
                                .rounded_full()
                                .bg(kind_dot_color(kind))
                                .flex_none(),
                        )
                        .child(div().text_size(px(12.5)).child(kind.label())),
                );
            }
            return (rail, Some(menu));
        }
        (rail, None)
    }
}
