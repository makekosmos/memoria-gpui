//! Local wall-clock bridge — JS `Date` getters (`getHours`, `getDay`,
//! `new Date(y, m, d, h, min)`) are *local* time; `time.rs` is UTC-only civil
//! math. This module resolves the host UTC offset via `chrono::Local`
//! (DST-aware per instant) and converts local civil fields ↔ epoch millis.
//!
//! Pure helpers take an explicit `offset_min` so tests stay deterministic in
//! any host timezone; the `local_*` entry points resolve the real offset.

use crate::time::{civil_from_days, days_from_civil, days_in_month};

/// Local civil decomposition of an epoch instant (JS `Date` getters).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Civil {
    pub year: i64,
    /// 1–12 (JS `getMonth() + 1`).
    pub month: i64,
    /// Day of month, 1–31.
    pub day: i64,
    pub hour: i64,
    pub minute: i64,
    pub second: i64,
    pub milli: i64,
    /// JS `getDay()`: 0 = Sunday … 6 = Saturday.
    pub weekday: i64,
}

/// `date.getTimezoneOffset()` sign convention flipped: minutes **ahead** of
/// UTC (Europe/Moscow → +180). Resolved for the given instant so DST is right.
pub fn local_offset_minutes(ms: i64) -> i64 {
    use chrono::{Local, Offset, TimeZone};
    Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|t| (t.offset().fix().local_minus_utc() / 60) as i64)
        .unwrap_or(0)
}

/// Civil fields of `ms` under an explicit fixed offset (`offset_min` ahead of
/// UTC). 1970-01-01 was a Thursday, hence the `+4` bias for `weekday`.
pub fn civil_at(ms: i64, offset_min: i64) -> Civil {
    let shifted = ms + offset_min * 60_000;
    let days = shifted.div_euclid(86_400_000);
    let rem = shifted.rem_euclid(86_400_000);
    let (year, month, day) = civil_from_days(days);
    Civil {
        year,
        month,
        day,
        hour: rem / 3_600_000,
        minute: rem % 3_600_000 / 60_000,
        second: rem % 60_000 / 1_000,
        milli: rem % 1_000,
        weekday: (days + 4).rem_euclid(7),
    }
}

/// Local civil fields of `ms` in the host zone.
pub fn local_civil(ms: i64) -> Civil {
    civil_at(ms, local_offset_minutes(ms))
}

/// `new Date(y, m, d, h, min)` under an explicit fixed offset — the civil
/// instant minus the offset. No normalization of out-of-range fields (diary
/// callers validate `YYYY-MM-DD`/`HH:MM` first, like Vue's round-trip check).
pub fn local_ms_at(
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    offset_min: i64,
) -> i64 {
    days_from_civil(year, month, day) * 86_400_000 + hour * 3_600_000 + minute * 60_000
        - offset_min * 60_000
}

/// `new Date(y, m, d, h, min).getTime()` in the host zone. Two passes so the
/// offset is evaluated at (approximately) the target instant — correct across
/// DST boundaries for real zones.
pub fn local_ms(year: i64, month: i64, day: i64, hour: i64, minute: i64) -> i64 {
    let guess = local_ms_at(
        year,
        month,
        day,
        hour,
        minute,
        local_offset_minutes(year_ms_hint(year, month, day, hour, minute)),
    );
    local_ms_at(year, month, day, hour, minute, local_offset_minutes(guess))
}

fn year_ms_hint(year: i64, month: i64, day: i64, hour: i64, minute: i64) -> i64 {
    // UTC midpoint guess — only used to pick the DST window to evaluate.
    days_from_civil(year, month, day) * 86_400_000 + hour * 3_600_000 + minute * 60_000
}

/// `new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()` — local
/// midnight of the day containing `ms`.
pub fn local_day_start(ms: i64) -> i64 {
    let c = local_civil(ms);
    local_ms(c.year, c.month, c.day, 0, 0)
}

/// `setDate(getDate() + delta)` on pure civil fields (day-stepping can't hit
/// DST wall-clock issues — the civil tuple re-normalizes through
/// `days_from_civil`).
pub fn civil_add_days(year: i64, month: i64, day: i64, delta: i64) -> (i64, i64, i64) {
    civil_from_days(days_from_civil(year, month, day) + delta)
}

/// `YYYY-MM-DD` key for a civil date (JS `formatBubbleDateKey` output shape).
pub fn civil_date_key(year: i64, month: i64, day: i64) -> String {
    format!("{year:04}-{month:02}-{day:02}")
}

/// JS `getDay()` for a civil date — timezone-independent.
pub fn civil_weekday(year: i64, month: i64, day: i64) -> i64 {
    (days_from_civil(year, month, day) + 4).rem_euclid(7)
}

/// `daysInMonth` re-export for calendar stepping.
pub fn month_len(year: i64, month: i64) -> i64 {
    days_in_month(year, month)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_round_trip_and_weekday() {
        let c = civil_at(0, 0);
        assert_eq!((c.year, c.month, c.day, c.weekday), (1970, 1, 1, 4));
        // 2026-01-01 was a Thursday (4).
        let c = civil_at(
            crate::time::iso_to_millis("2026-01-01T00:00:00Z").unwrap(),
            0,
        );
        assert_eq!((c.year, c.month, c.day, c.weekday), (2026, 1, 1, 4));
        assert_eq!(civil_weekday(2026, 7, 5), 0); // Sunday
    }

    #[test]
    fn offset_math_matches_js_local_semantics() {
        // UTC+03:00: local 2026-01-01 09:00 == 06:00 UTC.
        let ms = local_ms_at(2026, 1, 1, 9, 0, 180);
        assert_eq!(ms, 1_767_225_600_000 - 10_800_000 + 32_400_000);
        let c = civil_at(ms, 180);
        assert_eq!((c.day, c.hour, c.minute), (1, 9, 0));
    }

    #[test]
    fn day_start_and_stepping() {
        // Fixed-offset determinism: day-start of 2026-07-10 23:59 UTC+0.
        let ms = crate::time::iso_to_millis("2026-07-10T23:59:00Z").unwrap();
        let c = civil_at(ms, 0);
        assert_eq!(civil_add_days(c.year, c.month, c.day, 1), (2026, 7, 11));
        assert_eq!(civil_add_days(2026, 12, 31, 1), (2027, 1, 1));
        assert_eq!(month_len(2026, 2), 28);
        assert_eq!(month_len(2024, 2), 29);
    }

    #[test]
    fn local_offset_resolves_to_a_number() {
        // Host zone is whatever it is — the contract is finite minutes.
        assert!(local_offset_minutes(0).abs() <= 14 * 60);
        // `local_ms` round-trips the wall clock in the resolved offset.
        let ms = local_ms(2026, 1, 1, 9, 30);
        let off = local_offset_minutes(ms);
        let c = civil_at(ms, off);
        assert_eq!(
            (c.year, c.month, c.day, c.hour, c.minute),
            (2026, 1, 1, 9, 30)
        );
    }
}
