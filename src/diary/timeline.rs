//! `bubbleDiaryModel.ts` timeline section — occurrence millis, `HH:MM` /
//! `Вчера` / `D мон` labels, date keys, and the one-level thread normalizer.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::local_time::{civil_at, civil_date_key, local_civil, local_ms, local_offset_minutes};

use super::{BubbleTimelineNode, ReplyLink};

/// `DATE_KEY_PATTERN` — `/^\d{4}-\d{2}-\d{2}$/`.
pub static DATE_KEY_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").expect("date key re"));

const MONTH_LABELS: [&str; 12] = [
    "янв", "фев", "мар", "апр", "май", "июн", "июл", "авг", "сен", "окт", "ноя", "дек",
];

/// `formatBubbleDateKey(new Date(ms))` — local `YYYY-MM-DD`.
pub fn format_bubble_date_key(ms: i64) -> String {
    let c = local_civil(ms);
    civil_date_key(c.year, c.month, c.day)
}

/// `bubbleDateKey(node, fallback)` — node's own date key or today's.
pub fn bubble_date_key(node: &BubbleTimelineNode, fallback_ms: i64) -> String {
    super::storage::normalize_bubble_date_key_str(node.date.as_deref(), Some(node.time.as_str()))
        .unwrap_or_else(|| format_bubble_date_key(fallback_ms))
}

/// `bubbleOccurrenceMillis` — createdAt → sortKey → date+time (validated
/// local construction) → `draft-<ms>` id fallback. Returns `None` like the
/// Vue `null`.
pub fn bubble_occurrence_millis(node: &BubbleTimelineNode) -> Option<i64> {
    if let Some(created) = &node.created_at {
        if let Some(parsed) = crate::time::iso_to_millis(created) {
            return Some(parsed);
        }
    }
    if let Some(sort_key) = node.sort_key {
        if sort_key.is_finite() {
            return Some(sort_key as i64);
        }
    }
    static TIME_PATTERN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\d{2}:\d{2}$").expect("time re"));
    if let Some(ms) = (|| -> Option<i64> {
        let date = node
            .date
            .as_deref()
            .filter(|d| DATE_KEY_PATTERN.is_match(d))?;
        if !TIME_PATTERN.is_match(&node.time) {
            return None;
        }
        let (y, mo, d) = parse_date_key_parts(date)?;
        let (h, mi) = parse_time_parts(&node.time)?;
        let ms = local_ms(y, mo, d, h, mi);
        let c = civil_at(ms, local_offset_minutes(ms));
        ((c.year, c.month, c.day, c.hour, c.minute) == (y, mo, d, h, mi)).then_some(ms)
    })() {
        return Some(ms);
    }
    static DRAFT_ID: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^draft-(\d{10,})$").expect("draft re"));
    DRAFT_ID
        .captures(&node.id)
        .and_then(|caps| caps[1].parse::<i64>().ok())
}

fn parse_date_key_parts(key: &str) -> Option<(i64, i64, i64)> {
    let mut it = key.split('-');
    let y = it.next()?.parse().ok()?;
    let m = it.next()?.parse().ok()?;
    let d = it.next()?.parse().ok()?;
    Some((y, m, d))
}

fn parse_time_parts(key: &str) -> Option<(i64, i64)> {
    let mut it = key.split(':');
    let h = it.next()?.parse().ok()?;
    let m = it.next()?.parse().ok()?;
    Some((h, m))
}

/// `node.createdAt ?? node.sortKey ?? node.time` as the JSON scalar
/// `formatBubbleOccurrenceLabel` consumes.
pub fn node_occurrence(node: &BubbleTimelineNode) -> Value {
    if let Some(created) = &node.created_at {
        return Value::String(created.clone());
    }
    if let Some(sort_key) = node.sort_key {
        return serde_json::json!(sort_key);
    }
    Value::String(node.time.clone())
}

/// `formatBubbleOccurrenceLabel` — occurrence is `createdAt ?? sortKey ??
/// time` (string ISO/date or ms); `now` is epoch ms. Local calendar edges.
pub fn format_bubble_occurrence_label(occurrence: Option<&Value>, now_ms: i64) -> String {
    let Some(value) = occurrence else {
        return String::new();
    };
    let occ_ms = match value {
        Value::String(s) => match crate::time::iso_to_millis(s) {
            Some(ms) => ms,
            None => return String::new(),
        },
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.is_finite() => f as i64,
            _ => return String::new(),
        },
        _ => return String::new(),
    };
    occurrence_label_from_ms(occ_ms, now_ms)
}

/// Label for a resolved instant — the shared tail of the Vue function.
pub fn occurrence_label_from_ms(occ_ms: i64, now_ms: i64) -> String {
    let occ = civil_at(occ_ms, local_offset_minutes(occ_ms));
    let time = format!("{:02}:{:02}", occ.hour, occ.minute);
    let day_start = crate::local_time::local_day_start(occ_ms);
    let today_start = crate::local_time::local_day_start(now_ms);
    if day_start == today_start {
        return time;
    }
    let now_c = local_civil(now_ms);
    let (yy, ym, yd) = crate::local_time::civil_add_days(now_c.year, now_c.month, now_c.day, -1);
    let yesterday_start = local_ms(yy, ym, yd, 0, 0);
    if day_start == yesterday_start {
        return format!("Вчера, {time}");
    }
    let date_label = format!(
        "{} {}",
        occ.day,
        MONTH_LABELS[(occ.month - 1).clamp(0, 11) as usize]
    );
    if occ.year == now_c.year {
        format!("{date_label}, {time}")
    } else {
        format!("{date_label} {}, {time}", occ.year)
    }
}

/// `bubbleSortValue`.
fn bubble_sort_value(bubble: &BubbleTimelineNode) -> i64 {
    bubble_occurrence_millis(bubble).unwrap_or(0)
}

/// `normalizeBubbleThreads` — one-level reply threading. Roots newest-first;
/// replies oldest-first. Self-loops, dangling endpoints, multi-target links,
/// deeper-than-one relationships and cycles land in `invalid_link_ids` and
/// their children stay visible as roots.
pub fn normalize_bubble_threads(
    bubbles: Vec<BubbleTimelineNode>,
    reply_links: &[ReplyLink],
) -> (Vec<BubbleTimelineNode>, Vec<String>) {
    let by_id: HashMap<&str, &BubbleTimelineNode> =
        bubbles.iter().map(|b| (b.id.as_str(), b)).collect();
    let mut links_by_child: HashMap<&str, Vec<&ReplyLink>> = HashMap::new();
    let mut invalid: Vec<String> = Vec::new();
    let mut invalid_set: HashSet<String> = HashSet::new();
    let mark_invalid = |id: &str, invalid: &mut Vec<String>, set: &mut HashSet<String>| {
        if set.insert(id.to_string()) {
            invalid.push(id.to_string());
        }
    };

    for link in reply_links {
        if link.source_object_id == link.target_object_id
            || !by_id.contains_key(link.source_object_id.as_str())
            || !by_id.contains_key(link.target_object_id.as_str())
        {
            mark_invalid(&link.id, &mut invalid, &mut invalid_set);
            continue;
        }
        links_by_child
            .entry(link.source_object_id.as_str())
            .or_default()
            .push(link);
    }

    let mut parent_by_child: HashMap<String, String> = HashMap::new();
    for (child_id, links) in &links_by_child {
        let targets: HashSet<&str> = links.iter().map(|l| l.target_object_id.as_str()).collect();
        if targets.len() != 1 {
            for link in links {
                mark_invalid(&link.id, &mut invalid, &mut invalid_set);
            }
            continue;
        }
        parent_by_child.insert((*child_id).to_string(), links[0].target_object_id.clone());
        for link in &links[1..] {
            mark_invalid(&link.id, &mut invalid, &mut invalid_set);
        }
    }

    // Deeper-than-one / cycles: a child cannot itself be a parent.
    let candidate_children: HashSet<&String> = parent_by_child.keys().collect();
    let deeper: Vec<String> = parent_by_child
        .iter()
        .filter(|(_, parent)| candidate_children.contains(parent))
        .map(|(child, _)| child.clone())
        .collect();
    for child_id in deeper {
        parent_by_child.remove(&child_id);
        if let Some(links) = links_by_child.get(child_id.as_str()) {
            for link in links {
                mark_invalid(&link.id, &mut invalid, &mut invalid_set);
            }
        }
    }

    let mut roots: Vec<&BubbleTimelineNode> = bubbles
        .iter()
        .filter(|b| !parent_by_child.contains_key(&b.id))
        .collect();
    roots.sort_by(|l, r| {
        bubble_sort_value(r)
            .cmp(&bubble_sort_value(l))
            .then_with(|| l.id.cmp(&r.id))
    });

    let mut threaded: Vec<BubbleTimelineNode> = Vec::with_capacity(bubbles.len());
    for root in roots {
        let mut root_node = root.clone();
        root_node.parent_id = None;
        threaded.push(root_node);
        let mut replies: Vec<&BubbleTimelineNode> = bubbles
            .iter()
            .filter(|b| parent_by_child.get(&b.id) == Some(&root.id))
            .collect();
        replies.sort_by(|l, r| {
            bubble_sort_value(l)
                .cmp(&bubble_sort_value(r))
                .then_with(|| l.id.cmp(&r.id))
        });
        for reply in replies {
            let mut node = reply.clone();
            node.parent_id = Some(root.id.clone());
            threaded.push(node);
        }
    }

    (threaded, invalid)
}
