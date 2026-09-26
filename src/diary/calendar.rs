//! `BubbleDiaryCalendarSidebar` model — weeks from the oldest entry to today.
//! Weeks anchor on Monday; the first week ends today; subsequent weeks step
//! back 7 days until they cover `oldest − 40d`. Days render newest-first.

use std::collections::HashMap;

use crate::local_time::{civil_add_days, civil_at, civil_date_key, civil_weekday};

use super::timeline::{bubble_date_key, format_bubble_date_key};
use super::BubbleTimelineNode;

/// `CalendarDay`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarDay {
    pub key: String,
    /// `S M T W T F S` by `getDay()`.
    pub day_name: &'static str,
    pub day_number: u32,
    pub count: usize,
    pub today: bool,
    /// `getDay() === 0` — Sunday edge (accent tint in the Vue CSS).
    pub week_edge: bool,
}

/// `CalendarWeek`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarWeek {
    pub key: String,
    /// `"{ru short month of week start} {year}"`.
    pub label: String,
    /// Newest-first within the week.
    pub days: Vec<CalendarDay>,
}

const DAY_NAMES: [&str; 7] = ["S", "M", "T", "W", "T", "F", "S"];

/// `date.toLocaleDateString("ru-RU", {month:"short"}).replace(".","")` —
/// CLDR abbreviated (genitive) months.
const MONTH_SHORT: [&str; 12] = [
    "янв", "февр", "мар", "апр", "мая", "июн", "июл", "авг", "сент", "окт", "нояб", "дек",
];

/// `dateCounts` — `bubbleDateKey` → entry count.
pub fn date_counts(bubbles: &[BubbleTimelineNode], now_ms: i64) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for bubble in bubbles {
        *counts.entry(bubble_date_key(bubble, now_ms)).or_default() += 1;
    }
    counts
}

fn parse_date_key(key: &str) -> Option<(i64, i64, i64)> {
    let mut it = key.split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let d: i64 = it.next()?.parse().ok()?;
    it.next().is_none().then_some((y, m, d))
}

/// `oldestDate` — local start-of-today clamped to the oldest counted date.
fn oldest_civil(counts: &HashMap<String, usize>, now_ms: i64) -> (i64, i64, i64) {
    let now = civil_at(now_ms, crate::local_time::local_offset_minutes(now_ms));
    let mut oldest = (now.year, now.month, now.day);
    let mut oldest_days = crate::time::days_from_civil(oldest.0, oldest.1, oldest.2);
    for key in counts.keys() {
        if let Some((y, m, d)) = parse_date_key(key) {
            let days = crate::time::days_from_civil(y, m, d);
            if days < oldest_days {
                oldest_days = days;
                oldest = (y, m, d);
            }
        }
    }
    oldest
}

/// `buildWeeks(oldestDate, dateCounts, todayKey)` — civil-date arithmetic only
/// (`getDay`/`setDate` never hit wall-clock DST when stepping whole days).
pub fn build_weeks(bubbles: &[BubbleTimelineNode], now_ms: i64) -> Vec<CalendarWeek> {
    let counts = date_counts(bubbles, now_ms);
    let today_key = format_bubble_date_key(now_ms);
    let now_c = civil_at(now_ms, crate::local_time::local_offset_minutes(now_ms));

    // `while (current.getDay() !== 1) current.setDate(current.getDate() - 1)`
    let mut week_start = (now_c.year, now_c.month, now_c.day);
    while civil_weekday(week_start.0, week_start.1, week_start.2) != 1 {
        week_start = civil_add_days(week_start.0, week_start.1, week_start.2, -1);
    }
    let first_week_end = (now_c.year, now_c.month, now_c.day);

    let oldest = oldest_civil(&counts, now_ms);
    let padded_oldest = civil_add_days(oldest.0, oldest.1, oldest.2, -40);

    let mut weeks = vec![create_week(week_start, first_week_end, &counts, &today_key)];
    while gt_civil(week_start, padded_oldest) {
        let week_end = civil_add_days(week_start.0, week_start.1, week_start.2, -1);
        week_start = civil_add_days(week_end.0, week_end.1, week_end.2, -6);
        weeks.push(create_week(week_start, week_end, &counts, &today_key));
    }
    weeks
}

fn gt_civil(a: (i64, i64, i64), b: (i64, i64, i64)) -> bool {
    crate::time::days_from_civil(a.0, a.1, a.2) > crate::time::days_from_civil(b.0, b.1, b.2)
}

/// `createWeek` — iterate start..=end ascending, then reverse.
fn create_week(
    start: (i64, i64, i64),
    end: (i64, i64, i64),
    counts: &HashMap<String, usize>,
    today: &str,
) -> CalendarWeek {
    let mut days = Vec::new();
    let mut cursor = start;
    while !gt_civil(cursor, end) {
        let key = civil_date_key(cursor.0, cursor.1, cursor.2);
        let wd = civil_weekday(cursor.0, cursor.1, cursor.2);
        days.push(CalendarDay {
            count: counts.get(&key).copied().unwrap_or(0),
            today: key == today,
            key,
            day_name: DAY_NAMES[wd as usize],
            day_number: cursor.2 as u32,
            week_edge: wd == 0,
        });
        cursor = civil_add_days(cursor.0, cursor.1, cursor.2, 1);
    }
    days.reverse();
    CalendarWeek {
        key: format!(
            "{}:{}",
            civil_date_key(start.0, start.1, start.2),
            civil_date_key(end.0, end.1, end.2)
        ),
        label: format!(
            "{} {}",
            MONTH_SHORT[(start.1 - 1).clamp(0, 11) as usize],
            start.0
        ),
        days,
    }
}
