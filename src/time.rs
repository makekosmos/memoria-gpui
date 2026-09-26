//! Millis ↔ ISO-8601 conversion without a chrono dependency. Covers exactly
//! the shapes Vue Memoria round-trips (`Date.parse` / `Date.toISOString`).

/// Unix epoch millis for a civil date (Howard Hinnant's days-from-civil).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    (
        if mp < 10 { y } else { y + 1 },
        mp + 3,
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

/// `Date.parse(...)` for RFC-3339 / ISO-8601 shapes Vue Memoria stores:
/// `YYYY-MM-DD`, `...T HH:MM`, `HH:MM:SS(.frac)?`, `Z` or `±HH:MM`.
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
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
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
    let mut rest = &rest[5..];
    let mut sec = 0;
    let mut frac: i64 = 0;
    if rest.len() >= 2 && rest.as_bytes()[0] == b':' {
        sec = rest.get(1..3)?.parse().ok()?;
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
    ms += h * 3_600_000 + min * 60_000 + sec * 1_000 + frac;
    match rest.as_bytes().first()? {
        b'Z' | b'z' => Some(ms),
        b'+' | b'-' if rest.len() >= 6 && rest.as_bytes()[3] == b':' => {
            let oh: i64 = rest.get(1..3)?.parse().ok()?;
            let om: i64 = rest.get(4..6)?.parse().ok()?;
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

/// `new Date(value).getTime()` for Engine timestamp wire shapes
/// (ISO string or epoch millis number); None when unparsable.
pub fn timestamp_to_millis(value: &serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f.trunc() as i64)),
        serde_json::Value::String(s) => iso_to_millis(s).or_else(|| s.parse().ok()),
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
