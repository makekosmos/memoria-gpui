//! Date formatting ported from `objectFieldFormatting.ts` /
//! `TrashSettings.vue` — readable Russian dates and the relative
//! "Удалено …" labels used by the trash list.

use crate::time::iso_to_millis;
use serde_json::Value;

const MONTHS_GENITIVE: [&str; 12] = [
    "января",
    "февраля",
    "марта",
    "апреля",
    "мая",
    "июня",
    "июля",
    "августа",
    "сентября",
    "октября",
    "ноября",
    "декабря",
];

/// `formatReadableRussianDate` — `«12 марта 2026 года»`. Non-date scalars
/// pass through as text, matching the JS fallback `String(value)`.
pub fn format_readable_russian_date(value: &Value) -> String {
    let millis = match value {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) if !s.trim().is_empty() => iso_to_millis(s),
        _ => None,
    };
    let Some(ms) = millis else {
        return match value {
            Value::Null | Value::Bool(_) => String::new(),
            other => match other {
                Value::String(s) => s.clone(),
                _ => other.to_string(),
            },
        };
    };
    format_russian_date_ms(ms)
}

/// `«12 марта 2026 года»` from epoch millis (UTC — Vue `new Date(ms)` uses
/// local time; UTC keeps it deterministic and matches the Engine's stored
/// ISO strings, which are already UTC).
pub fn format_russian_date_ms(ms: i64) -> String {
    let (y, m, d) = civil_from_ms(ms);
    format!("{d} {} {y} года", MONTHS_GENITIVE[(m - 1) as usize])
}

/// `formatTimeAgo` from TrashSettings.vue — elapsed 24h periods, not calendar
/// days: `floor((now - deletedAt)/86400000)` → сегодня/вчера/`N дн.`/`N нед.`.
/// The Vue template prefixes the result with «Удалено ».
pub fn trash_time_ago_label(deleted_ms: i64, now_ms: i64) -> String {
    let days = (now_ms - deleted_ms).div_euclid(86_400_000);
    if days <= 0 {
        "сегодня".into()
    } else if days == 1 {
        "вчера".into()
    } else if days < 7 {
        format!("{days} дн. назад")
    } else {
        format!("{} нед. назад", days / 7)
    }
}

/// Howard Hinnant's civil-from-days algorithm.
fn civil_from_ms(ms: i64) -> (i64, i64, i64) {
    let z = ms.div_euclid(86_400_000) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn readable_date() {
        // 2026-01-01T00:00:00Z = 1767225600000
        assert_eq!(
            format_russian_date_ms(1_767_225_600_000),
            "1 января 2026 года"
        );
        assert_eq!(
            format_readable_russian_date(&json!("2024-06-05T10:00:00Z")),
            "5 июня 2024 года"
        );
        assert_eq!(
            format_readable_russian_date(&json!(1_767_225_600_000i64)),
            "1 января 2026 года"
        );
        assert_eq!(format_readable_russian_date(&json!("текст")), "текст");
        assert_eq!(format_readable_russian_date(&Value::Null), "");
    }

    #[test]
    fn trash_labels() {
        let now = 1_773_273_600_000i64; // 2026-03-11T00:00:00Z
                                        // Elapsed-day math (not calendar days) — mirrors formatTimeAgo.
        assert_eq!(trash_time_ago_label(now - 3_600_000, now), "сегодня");
        assert_eq!(trash_time_ago_label(now - 30 * 3_600_000, now), "вчера");
        assert_eq!(
            trash_time_ago_label(now - 3 * 86_400_000, now),
            "3 дн. назад"
        );
        assert_eq!(
            trash_time_ago_label(now - 6 * 86_400_000, now),
            "6 дн. назад"
        );
        assert_eq!(
            trash_time_ago_label(now - 7 * 86_400_000, now),
            "1 нед. назад"
        );
        assert_eq!(
            trash_time_ago_label(now - 21 * 86_400_000, now),
            "3 нед. назад"
        );
    }
}
