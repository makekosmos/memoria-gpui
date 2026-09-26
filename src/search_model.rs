//! Search helpers — the `useSearch.ts` + `EverythingView.vue` contract:
//! local title/author match is instant, Engine `searchEntries` covers body
//! text, results merge by id and stay sorted by `updated_at` desc.
//! `highlight_ranges` backs the result-row text highlighting.

use crate::model::{Entry, SearchResult};
use crate::object_views::parse_entry_header_props;

/// Case-insensitive substring ranges of `query` inside `text`, as char indices.
/// Used to bold the matched part of a result snippet (Engine matches on body
/// text; the title match is implicit).
pub fn highlight_ranges(text: &str, query: &str) -> Vec<(usize, usize)> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let text_lower = text.to_lowercase();
    let query_lower = query.to_lowercase();
    let mut ranges = Vec::new();
    let mut start = 0usize;
    while let Some(pos) = text_lower[start..].find(&query_lower) {
        let abs = start + pos;
        let end = abs + query_lower.len();
        // byte→char index conversion (Cyrillic-safe).
        let start_char = text[..abs].chars().count();
        let end_char = text[..end.min(text.len())].chars().count();
        ranges.push((start_char, end_char));
        start = end.max(start + 1);
        if start >= text_lower.len() {
            break;
        }
    }
    ranges
}

/// Vue `matchesLocalQuery` — title, author and person name fields.
fn local_match(query_lc: &str, entry: &Entry) -> bool {
    if entry.title.to_lowercase().contains(query_lc) {
        return true;
    }
    let props = parse_entry_header_props(entry);
    for key in ["author", "first_name", "patronymic", "last_name"] {
        if let Some(v) = props.get(key).and_then(|v| v.as_str()) {
            if v.to_lowercase().contains(query_lc) {
                return true;
            }
        }
    }
    false
}

/// Merge of `filteredEntries` + remote results: local hits first by Engine
/// order? No — Vue filters `entries` locally and appends remote-only hits,
/// preserving the underlying `updated_at`-desc ordering of the entry list.
pub fn everything_filtered(
    entries: &[Entry],
    query: &str,
    remote_hits: &[SearchResult],
    entry_by_id: &dyn Fn(&str) -> Option<Entry>,
) -> Vec<Entry> {
    let query_lc = query.trim().to_lowercase();
    if query_lc.is_empty() {
        return entries.to_vec();
    }
    let mut out: Vec<Entry> = entries
        .iter()
        .filter(|e| local_match(&query_lc, e))
        .cloned()
        .collect();
    let mut seen: std::collections::HashSet<String> = out.iter().map(|e| e.id.clone()).collect();
    for hit in remote_hits {
        if seen.contains(&hit.entry_id) {
            continue;
        }
        if let Some(entry) = entry_by_id(&hit.entry_id) {
            seen.insert(entry.id.clone());
            out.push(entry);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(id: &str, title: &str, updated: i64) -> Entry {
        Entry {
            id: id.into(),
            title: title.into(),
            updated_at: updated,
            ..Default::default()
        }
    }

    #[test]
    fn ranges_case_insensitive_unicode() {
        // "Привет МИР привет" — two hits on "мир"/"привет".
        assert_eq!(highlight_ranges("Привет МИР", "мир"), vec![(7, 10)]);
        assert_eq!(highlight_ranges("Привет мир", "Привет"), vec![(0, 6)]);
        assert_eq!(highlight_ranges("aaa", ""), Vec::<(usize, usize)>::new());
        assert_eq!(highlight_ranges("a a a", "a"), vec![(0, 1), (2, 3), (4, 5)]);
    }

    #[test]
    fn local_match_author_and_names() {
        let mut e = entry("a", "Название", 1);
        e.header_props_json = Some(json!({"author": "Толстой"}).to_string());
        let hits = everything_filtered(&[e.clone()], "толст", &[], &|_| None);
        assert_eq!(hits.len(), 1);
        let hits2 = everything_filtered(&[e.clone()], "назва", &[], &|_| None);
        assert_eq!(hits2.len(), 1);
        let hits3 = everything_filtered(&[e], "zzz", &[], &|_| None);
        assert!(hits3.is_empty());
    }

    #[test]
    fn remote_hits_dedupe_and_append() {
        let local = entry("a", "локальный", 10);
        let remote_only = entry("b", "другой", 5);
        let all = vec![local.clone(), remote_only.clone()];
        let hits = vec![
            SearchResult {
                entry_id: "a".into(),
                ..Default::default()
            },
            SearchResult {
                entry_id: "b".into(),
                ..Default::default()
            },
        ];
        let out = everything_filtered(&all, "", &[], &|_| None);
        assert_eq!(out.len(), 2); // empty query → unfiltered
        let out = everything_filtered(&all, "локал", &hits, &|id| {
            all.iter().find(|e| e.id == id).cloned()
        });
        let ids: Vec<&str> = out.iter().map(|e| e.id.as_str()).collect();
        // `a` matched locally (first), `b` appended from remote once.
        assert_eq!(ids, ["a", "b"]);
    }
}
