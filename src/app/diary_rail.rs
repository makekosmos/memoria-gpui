//! `BubbleTimelineItem` rail — the kind dot — plus the kind dropdown as a
//! root-level overlay (backdrop + Esc dismissal, like `render_ctx_menu`;
//! Vue gets both from the shared `Dropdown` component).
use gpui::{div, prelude::*, px, Context, MouseButton};
use memoria_model::diary::{BubbleKind, BubbleTimelineNode};
use memoria_model::store::{BubblePatch, Command};

use super::diary_item::kind_dot_color;
use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;

impl Memoria {
    /// `.bubble-timeline-item__rail` — kind dot; opens the dropdown overlay.
    pub(crate) fn diary_rail(
        &self,
        node: &BubbleTimelineNode,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let id = node.id.clone();
        div()
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
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &gpui::MouseDownEvent, _, cx| {
                            this.kind_menu_for =
                                if this.kind_menu_for.as_ref().map(|(open, _)| open.as_str())
                                    == Some(id.as_str())
                                {
                                    None
                                } else {
                                    Some((id.clone(), ev.position))
                                };
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
    }

    /// Kind dropdown overlay — backdrop click dismisses (Vue `Dropdown`
    /// outside-click), options anchored at the recorded click point.
    pub(crate) fn render_kind_menu(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let (id, position) = self.kind_menu_for.clone()?;
        let mut menu = div()
            .id(format!("kind-menu-{id}"))
            .debug_selector(|| "bubble-kind-menu".into())
            .absolute()
            .left(position.x + px(8.))
            .top(position.y + px(12.))
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
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        for kind in BubbleKind::OPTIONS {
            let node_id = id.clone();
            let sel = format!("bubble-kind-option-{}", kind.as_str());
            menu = menu.child(
                div()
                    .id(format!("bubble-kind-option-{node_id}-{}", kind.as_str()))
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
        Some(
            div()
                .id("kind-menu-backdrop")
                .debug_selector(|| "kind-menu-backdrop".into())
                .absolute()
                .inset_0()
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.kind_menu_for = None;
                        cx.notify();
                    }),
                )
                .child(menu),
        )
    }
}
