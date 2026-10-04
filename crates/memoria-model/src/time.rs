//! Millis ↔ ISO-8601 conversion without a chrono dependency. Covers exactly
//! the shapes Vue Memoria round-trips (`Date.parse` / `Date.toISOString`).

/// Unix epoch millis for a civil date (Howard Hinnant's days-from-civil).
/// `pub(crate)` — `local_time` reuses it for wall-clock conversion.
pub(crate) fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub(crate) fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    (
        if mp < 10 { y } else { y + 1 },
        if mp < 10 { mp + 3 } else { mp - 9 },
        doy - (153 * mp + 2) / 5 + 1,
    )
}

/// `new Date(ms).toISOString()` — always `YYYY-MM-DDTHH:MM:SS.sssZ`.
pub fn millis_to_iso(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    let (y, m, d) = civil_from_days(days);
    let h = rem / 3_600_000;
    let min = rem % 3_600_000 / 60_000;
    let s = rem % 60_000 / 1000;
    let ms = rem % 1000;
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{min:02}:{s:02}.{ms:03}Z")
}

pub(crate) fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

/// `Date.parse(...)` for RFC-3339 / ISO-8601 shapes Vue Memoria stores:
/// `YYYY-MM-DD`, `...T HH:MM`, `HH:MM:SS(.frac)?`, `Z` or `±HH:MM`.
/// Zone-less date-times are rejected — JS parses them as *local* time and
/// the Rust port has no TZ data; Engine emits `Z`-suffixed stamps.
pub fn iso_to_millis(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 10 {
        return None;
    }
    let y: i64 = s.get(0..4)?.parse().ok()?;
    if s.as_bytes()[4] != b'-' || s.as_bytes()[7] != b'-' {
        return None;
    }
    let m: i64 = s.get(5..7)?.parse().ok()?;
    let d: i64 = s.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=days_in_month(y, m)).contains(&d) {
        return None;
    }
    let mut ms = days_from_civil(y, m, d) * 86_400_000;
    let rest = &s[10..];
    if rest.is_empty() {
        return Some(ms);
    }
    if !matches!(rest.as_bytes()[0], b'T' | b't' | b' ') {
        return None;
    }
    let rest = &rest[1..];
    if rest.len() < 5 || rest.as_bytes()[2] != b':' {
        return None;
    }
    let h: i64 = rest.get(0..2)?.parse().ok()?;
    let min: i64 = rest.get(3..5)?.parse().ok()?;
    // ISO allows 24:00 (== next midnight) only when min/sec/frac are zero.
    if !(0..=24).contains(&h) || !(0..=59).contains(&min) {
        return None;
    }
    let mut rest = &rest[5..];
    let mut sec = 0;
    let mut frac: i64 = 0;
    if rest.len() >= 2 && rest.as_bytes()[0] == b':' {
        sec = rest.get(1..3)?.parse().ok()?;
        if !(0..=59).contains(&sec) {
            return None;
        }
        rest = &rest[3..];
    }
    if rest.starts_with('.') {
        let digits: String = rest[1..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        // Date.parse semantics: first three digits, right-padded to ms.
        let mut padded = digits.chars().take(3).collect::<String>();
        while padded.len() < 3 {
            padded.push('0');
        }
        frac = padded.parse::<i64>().ok()?;
        rest = &rest[1 + digits.len()..];
    }
    if h == 24 && (min != 0 || sec != 0 || frac != 0) {
        return None;
    }
    ms += h * 3_600_000 + min * 60_000 + sec * 1_000 + frac;
    // Trailing junk after the zone is a parse failure, not a match.
    match (rest.as_bytes().first()?, rest.len()) {
        (b'Z' | b'z', 1) => Some(ms),
        (b'+' | b'-', 6) if rest.as_bytes()[3] == b':' => {
            let oh: i64 = rest.get(1..3)?.parse().ok()?;
            let om: i64 = rest.get(4..6)?.parse().ok()?;
            if !(0..=23).contains(&oh) || !(0..=59).contains(&om) {
                return None;
            }
            let off = oh * 3_600_000 + om * 60_000;
            Some(if rest.as_bytes()[0] == b'+' {
                ms - off
            } else {
                ms + off
            })
        }
        _ => None,
    }
}

/// `Date.parse(value)` semantics for Engine timestamp wire shapes — only
/// ISO strings parse; numbers/numeric strings are `NaN` (`None`) like JS.
pub fn timestamp_to_millis(value: &serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::String(s) => iso_to_millis(s),
        _ => None,
    }
}

/// Current Unix epoch millis (`Date.now()`).
pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_parse_parity() {
        // ISO shapes Vue accepts.
        assert!(iso_to_millis("2024-01-01").is_some());
        assert!(iso_to_millis("2024-02-29").is_some());
        assert_eq!(
            iso_to_millis("2024-01-01T24:00:00Z"),
            iso_to_millis("2024-01-02T00:00:00Z")
        );
        assert_eq!(
            iso_to_millis("2024-06-01T12:30:00+02:30"),
            iso_to_millis("2024-06-01T10:00:00Z")
        );
        // Shapes `Date.parse` rejects (NaN → fallback).
        for bad in [
            "2023-02-29",           // not a leap year
            "2024-04-31",           // April has 30 days
            "2024-01-01T25:00Z",    // hour out of range
            "2024-01-01T24:01Z",    // 24:xx only valid at :00
            "2024-01-01T12:60Z",    // minute out of range
            "2024-01-01T23:59:60Z", // leap seconds rejected
            "2024-01-01T10:00",     // zone-less == local time in JS; no TZ here
            "2024-01-01T00:00Zx",   // trailing junk
            "1700000000000",        // numeric string is not a date
            "garbage",
        ] {
            assert_eq!(iso_to_millis(bad), None, "{bad}");
        }
        // `Date.parse(number)` / numeric strings are NaN → `None`.
        assert_eq!(
            timestamp_to_millis(&serde_json::json!(1700000000000i64)),
            None
        );
        assert_eq!(
            timestamp_to_millis(&serde_json::json!("1700000000000")),
            None
        );
        assert!(timestamp_to_millis(&serde_json::json!("2024-01-01T00:00:00.000Z")).is_some());
    }
}
