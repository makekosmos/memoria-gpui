//! Live-refresh gating for the open note — wraps the M1
//! `should_apply_remote_entry` port with the note-switch override.

use memoria_gpui::content;
use memoria_gpui::live_refresh;
use memoria_gpui::model::Entry;

/// Should a loaded/refreshed entry replace the editor text? Switching to a
/// different note always applies; for the same note the M1 live-refresh
/// guards decide (self-echo skip, dirty skip).
pub(super) fn refresh_decision(
    fresh: &Entry,
    current: Option<&Entry>,
    editor_markdown: &str,
    dirty: bool,
) -> live_refresh::RemoteEntryDecision {
    if current.is_some_and(|c| c.id != fresh.id) || current.is_none() {
        return live_refresh::RemoteEntryDecision::Apply;
    }
    live_refresh::should_apply_remote_entry(&live_refresh::RemoteEntryParams {
        fresh,
        current_content_json: &content::write_entry_markdown(editor_markdown).to_string(),
        current_entry: current,
        is_editor_dirty: dirty,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use live_refresh::RemoteEntryDecision;
    use memoria_gpui::content::write_entry_markdown;

    fn entry(id: &str, md: &str, updated_at: i64) -> Entry {
        Entry {
            id: id.into(),
            title: "t".into(),
            content_json: write_entry_markdown(md).to_string(),
            updated_at,
            ..Default::default()
        }
    }

    #[test]
    fn remote_change_while_typing_does_not_wipe_input() {
        let current = entry("n1", "old", 10);
        let fresh = entry("n1", "remote rewrite", 11);
        // Editor holds unsaved edits ("my draft") → refresh must skip.
        let d = refresh_decision(&fresh, Some(&current), "my draft", true);
        assert_eq!(d, RemoteEntryDecision::SkipDirty);
    }

    #[test]
    fn remote_self_echo_is_skipped_even_when_clean() {
        let current = entry("n1", "same text", 10);
        let fresh = entry("n1", "same text", 12);
        let d = refresh_decision(&fresh, Some(&current), "same text", false);
        assert_eq!(d, RemoteEntryDecision::SkipSameContent);
    }

    #[test]
    fn clean_editor_applies_remote_change() {
        let current = entry("n1", "old", 10);
        let fresh = entry("n1", "new remote", 11);
        let d = refresh_decision(&fresh, Some(&current), "old", false);
        assert_eq!(d, RemoteEntryDecision::Apply);
    }

    #[test]
    fn switching_notes_applies_even_when_dirty() {
        let current = entry("n1", "old", 10);
        let fresh = entry("n2", "other note", 11);
        let d = refresh_decision(&fresh, Some(&current), "unsaved edits", true);
        assert_eq!(d, RemoteEntryDecision::Apply);
    }
}
