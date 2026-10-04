//! `BubbleDiaryCalendarSidebar` port — week columns (Monday-anchored),
//! day rows with count dots, today/week-edge accents; click jumps the
//! timeline via `select_diary_date`.
use gpui::{div, prelude::*, px, Context};

use memoria_model::diary::build_weeks;

use super::Memoria;
use crate::theme::*;

impl Memoria {
    pub(crate) fn diary_calendar_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let weeks = build_weeks(&self.bubbles, self.label_now);
        let mut timeline = div()
            .id("diary-calendar-timeline")
            .size_full()
            .overflow_y_scroll()
            .pt(px(10.))
            .pb(px(18.))
            .text_size(px(12.8))
            .flex()
            .flex_col();

        for week in weeks {
            let mut week_el = div().relative().flex().flex_col().pl(px(24.)).pr(px(10.));
            // `.week-label` — absolute, bottom-left of the week column.
            week_el = week_el.child(
                div()
                    .absolute()
                    .bottom(px(-6.))
                    .left(px(28.))
                    .text_size(px(11.5))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(muted_fg_mix(0.56))
                    .whitespace_nowrap()
                    .child(week.label.clone()),
            );
            for day in week.days {
                let key = day.key.clone();
                let mut day_el = div()
                    .id(format!("diary-calendar-day-{}", day.key))
                    .debug_selector(move || format!("diary-calendar-day-{key}"))
                    .h(px(22.))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_end()
                    .cursor_pointer()
                    .text_color(if day.today {
                        c(ACCENT())
                    } else if day.week_edge {
                        mix(ACCENT(), 0.72, FG())
                    } else {
                        c(MUTED_FG())
                    });
                // Count dots — wrap, right-aligned, capped at 48.
                if day.count > 0 {
                    let mut dots = div()
                        .flex()
                        .flex_wrap()
                        .justify_end()
                        .items_start()
                        .max_w(px(82.))
                        .mr(px(5.));
                    for _ in 0..day.count.min(48) {
                        dots = dots.child(
                            div()
                                .size(px(4.))
                                .m(px(1.))
                                .mr(px(2.))
                                .rounded_full()
                                .bg(c(FG()))
                                .opacity(0.9),
                        );
                    }
                    day_el = day_el.child(dots);
                }
                let key2 = day.key.clone();
                day_el = day_el
                    .child(
                        div()
                            .w(px(20.))
                            .h(px(1.))
                            .mr(px(8.))
                            .bg(c(FG()))
                            .opacity(0.24),
                    )
                    .child(
                        div()
                            .w(px(10.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(day.day_name),
                    )
                    .child(
                        div()
                            .w(px(18.))
                            .mr(px(4.))
                            .opacity(0.62)
                            .child(day.day_number.to_string()),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_diary_date(&key2, cx);
                    }));
                week_el = week_el.child(day_el);
            }
            timeline = timeline.child(week_el);
        }

        div()
            .id("diary-calendar-sidebar")
            .debug_selector(|| "diary-calendar-sidebar".into())
            .role(gpui::Role::Complementary)
            .aria_label("Календарь дневника")
            .w(px(180.))
            .flex_none()
            .h_full()
            .bg(c(SIDEBAR_BG()))
            .text_color(muted_fg_mix(0.84))
            .border_l_1()
            .border_color(c(BORDER()))
            .child(timeline)
    }
}
