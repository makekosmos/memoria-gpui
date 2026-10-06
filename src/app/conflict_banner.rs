//! `EntryConflictBanner` — banner rendering + button row.
use gpui::{div, prelude::*, px, Context};

use memoria_model::entry_conflicts::unresolved_entry_conflicts;
use memoria_model::store::Command;

use super::types::{ConflictOp, ViewAction};
use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;

impl Memoria {
    /// Banner above the content area (`entry-conflict-banner` test id).
    pub(crate) fn render_conflict_banner(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let conflict = self.active_conflict()?;
        let state_label = Self::conflict_state_label(conflict.state);
        let count = unresolved_entry_conflicts(&self.conflicts.conflicts).len();
        let remote_missing = conflict.remote.is_none();

        let cid = conflict.id.clone();
        let cid2 = conflict.id.clone();
        let cid3 = conflict.id.clone();
        let cid4 = conflict.id.clone();

        Some(
            div()
                .id("entry-conflict-banner")
                .debug_selector(|| "entry-conflict-banner".into())
                .mx_4()
                .mt_2()
                .mb_1()
                .px_4()
                .py_3()
                .rounded_lg()
                .border_1()
                .border_color(c(BORDER()))
                .bg(c(SIDEBAR_BG()))
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(c(FG()))
                                .child(format!("Конфликт версий — {state_label}")),
                        )
                        .when(count > 1, |d| {
                            d.child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(c(MUTED_FG()))
                                    .child(format!("Конфликтов: {count}")),
                            )
                        }),
                )
                .child(div().text_size(px(12.)).text_color(c(MUTED_FG())).child(
                    if remote_missing {
                        "Удалённая копия отсутствует — запись была удалена.".to_string()
                    } else {
                        format!(
                            "Локальная версия изменена: {}",
                            memoria_model::dates::format_russian_date_ms(conflict.local.updated_at)
                        )
                    },
                ))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .flex_wrap()
                        .child(conflict_button(
                            cx,
                            "conflict-accept",
                            "Принять удалённую",
                            true,
                            {
                                let cid = cid.clone();
                                Box::new(move |this: &mut Memoria, cx| {
                                    this.send_accept_remote(cid.clone(), cx)
                                })
                            },
                        ))
                        .child(conflict_button(
                            cx,
                            "conflict-keep-copy",
                            "Оставить локальную копию",
                            false,
                            {
                                let cid = cid2.clone();
                                Box::new(move |this: &mut Memoria, cx| {
                                    this.keep_conflict_copy(cid.clone(), cx)
                                })
                            },
                        ))
                        .child(conflict_button(
                            cx,
                            "conflict-copy-local",
                            "Скопировать локальный текст",
                            false,
                            {
                                let cid = cid3.clone();
                                Box::new(move |this: &mut Memoria, cx| {
                                    this.copy_conflict_local(&cid, cx)
                                })
                            },
                        ))
                        .child(conflict_button(
                            cx,
                            "conflict-recheck",
                            "Проверить снова",
                            false,
                            {
                                let cid = cid4.clone();
                                Box::new(move |this: &mut Memoria, cx| {
                                    this.conflict_op = Some((cid.clone(), ConflictOp::Recheck));
                                    let entry_id = this
                                        .conflicts
                                        .conflicts
                                        .iter()
                                        .find(|c| c.id == cid)
                                        .map(|c| c.entry_id.clone());
                                    if let Some(entry_id) = entry_id {
                                        this.send(
                                            Command::LoadEntry {
                                                id: entry_id,
                                                content_only: true,
                                            },
                                            cx,
                                        );
                                    }
                                })
                            },
                        ))
                        .child(conflict_button(
                            cx,
                            "conflict-cancel",
                            "Отменить решение",
                            false,
                            {
                                let cid = conflict.id.clone();
                                Box::new(move |this: &mut Memoria, cx| {
                                    this.cancel_conflict(&cid, cx)
                                })
                            },
                        )),
                ),
        )
    }
}

/// One banner action row — secondary outline style, accent for `primary`.
fn conflict_button(
    cx: &mut Context<Memoria>,
    id: &'static str,
    label: &'static str,
    primary: bool,
    f: ViewAction,
) -> gpui::Stateful<gpui::Div> {
    let weak = cx.weak_entity();
    let mut el = div()
        .id(id)
        .debug_selector(move || id.to_string())
        .a11y_button(label)
        .px_3()
        .h(px(28.))
        .flex()
        .items_center()
        .rounded_md()
        .text_size(px(12.))
        .cursor_pointer();
    el = if primary {
        el.text_color(c(BG())).bg(c(ACCENT()))
    } else {
        el.text_color(c(FG()))
            .border_1()
            .border_color(c(BORDER()))
            .hover(|s| s.bg(rgba(FG(), 0.08)))
    };
    el.on_click(move |_, _, cx| {
        let _ = weak.update(cx, |this, cx| f(this, cx));
    })
    .child(label)
}
