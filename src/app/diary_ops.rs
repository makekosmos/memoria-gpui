//! Diary (M6) — store ops half of the `BubbleDiaryView` port:
//! `startDiary` migration kickoff, composer/reply/edit/delete state and the
//! `Command` senders. View markup lives in `diary.rs`.
use gpui::{prelude::*, Context, Entity, Focusable, Window};
use gpui_component::input::TextareaState;
use memoria_editor_gpui::{EditorEvent, MemoriaEditor};
use memoria_gpui::content::markdown_to_tiptap_doc;
use memoria_gpui::diary::{
    bubble_plain_text, parse_bubble_draft, strip_tags_from_tiptap_doc, BubbleKind,
    BubbleTimelineNode, LOCAL_BUBBLES_STORAGE_KEY,
};
use memoria_gpui::store::{BubblePatch, Command};
use memoria_gpui::time::now_millis;

use super::Memoria;

impl Memoria {
    /// `startDiary` — first visit migrates legacy journal/local bubbles, then
    /// lists; later visits just re-list. `labelNow` refreshes so
    /// `Вчера, HH:MM` labels resolve against arrival time.
    pub(crate) fn start_diary(&mut self, cx: &mut Context<Self>) {
        self.label_now = now_millis();
        if self.diary_started {
            self.send(Command::ListBubbles, cx);
            return;
        }
        self.diary_started = true;
        let local = self.prefs.extra.get(LOCAL_BUBBLES_STORAGE_KEY).cloned();
        self.send(
            Command::MigrateDiary {
                local_bubbles_json: local,
            },
            cx,
        );
    }

    /// Compact composer entity — one per session (Vue `composerEditor`).
    pub(crate) fn diary_composer_state(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<MemoriaEditor> {
        if let Some(editor) = &self.diary_composer {
            return editor.clone();
        }
        let editor = cx.new(|cx| MemoriaEditor::new_compact("", "Напиши мысль...", window, cx));
        self._subs
            .push(cx.subscribe(&editor, |this, _, ev: &EditorEvent, cx| {
                if matches!(ev, EditorEvent::Submit) {
                    this.submit_diary_draft(cx);
                }
            }));
        self.diary_composer = Some(editor.clone());
        editor
    }

    /// `addDraftBubble` — `editor.getText()` becomes the parse input, the
    /// stripped tiptap doc rides along as `contentJson`.
    pub(crate) fn submit_diary_draft(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.diary_composer.clone() else {
            return;
        };
        let doc = markdown_to_tiptap_doc(&editor.read(cx).markdown());
        let input = bubble_plain_text(&doc);
        if parse_bubble_draft(&input).0.is_empty() {
            return;
        }
        self.send_bubble_write(
            Command::CreateBubble {
                input,
                kind: BubbleKind::Plain,
                parent_id: None,
                content_json: Some(strip_tags_from_tiptap_doc(&doc)),
            },
            cx,
        );
    }

    /// `runBubbleWrite` — count in-flight bubble writes so a `Changed` push
    /// can't force a mid-write refetch.
    pub(crate) fn send_bubble_write(&mut self, command: Command, cx: &mut Context<Self>) {
        self.active_bubble_writes += 1;
        self.send(command, cx);
    }

    /// Reply composer — one shared textarea, rebound per thread root.
    pub(crate) fn reply_input_state(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<TextareaState> {
        if let Some(state) = &self.reply_input {
            return state.clone();
        }
        // Vue reply textarea has no placeholder — keep it empty.
        let state = cx.new(|cx| TextareaState::new(window, cx));
        self.reply_input = Some(state.clone());
        state
    }

    pub(crate) fn edit_input_state(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<TextareaState> {
        if let Some(state) = &self.edit_input {
            return state.clone();
        }
        let state = cx.new(|cx| TextareaState::new(window, cx));
        self.edit_input = Some(state.clone());
        state
    }

    /// `draftFromNode` — `"text #tag #tag"`.
    pub(crate) fn draft_from_node(node: &BubbleTimelineNode) -> String {
        let mut draft = node.text.clone();
        for tag in &node.tags {
            draft.push_str(" #");
            draft.push_str(tag);
        }
        draft.trim().to_string()
    }

    /// `startEditing` — single edit session; entering re-fills the draft.
    pub(crate) fn start_edit_bubble(
        &mut self,
        node: &BubbleTimelineNode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = self.edit_input_state(window, cx);
        let draft = Self::draft_from_node(node);
        input.update(cx, |s, cx| {
            s.set_value(draft, window, cx);
            // Vue `nextTick(() => editInputRef.value?.focus())`.
            s.focus_handle(cx).focus(window, cx);
        });
        self.bubble_delete_armed = None;
        self.editing_bubble = Some(node.id.clone());
        self.kind_menu_for = None;
        cx.notify();
    }

    pub(crate) fn cancel_edit_bubble(&mut self, cx: &mut Context<Self>) {
        self.editing_bubble = None;
        self.bubble_delete_armed = None;
        cx.notify();
    }

    /// `commitEdit` → `updateBubble` — `input` re-derives text/tags/content.
    pub(crate) fn commit_edit_bubble(&mut self, node_id: &str, cx: &mut Context<Self>) {
        let Some(input) = self.edit_input.clone() else {
            return;
        };
        let raw = input.read(cx).value().to_string();
        if parse_bubble_draft(&raw).0.is_empty() {
            return;
        }
        self.send_bubble_write(
            Command::UpdateBubble {
                id: node_id.to_string(),
                patch: BubblePatch {
                    input: Some(raw),
                    kind: None,
                },
            },
            cx,
        );
    }

    /// `requestDelete` — two-step («Удалить» → «Точно удалить»).
    pub(crate) fn request_delete_bubble(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.bubble_delete_armed.as_deref() == Some(id) {
            self.bubble_delete_armed = None;
            self.editing_bubble = None;
            self.send_bubble_write(Command::DeleteBubble(id.to_string()), cx);
        } else {
            self.bubble_delete_armed = Some(id.to_string());
            cx.notify();
        }
    }

    /// `replyToBubble` — plain draft input; the API mints the tiptap doc.
    pub(crate) fn submit_bubble_reply(&mut self, root_id: &str, cx: &mut Context<Self>) {
        let Some(input) = self.reply_input.clone() else {
            return;
        };
        let raw = input.read(cx).value().to_string();
        if parse_bubble_draft(&raw).0.is_empty() {
            return;
        }
        self.send_bubble_write(
            Command::CreateBubble {
                input: raw,
                kind: BubbleKind::Plain,
                parent_id: Some(root_id.to_string()),
                content_json: None,
            },
            cx,
        );
    }

    /// `date-select` → `scrollToDate` — deferred to render so `track_scroll`
    /// child bounds exist. Vue aligns "center"; this gpui-kit only offers
    /// first-visible/top (GAP, noted in PARITY.md).
    pub(crate) fn select_diary_date(&mut self, date_key: &str, cx: &mut Context<Self>) {
        self.diary_jump = Some(date_key.to_string());
        cx.notify();
    }

    /// Vue `findIndex(b => b.date === date)` — the raw field, not the
    /// derived key (a dateless bubble never jumps, matching Vue).
    pub(crate) fn diary_jump_target(&self, date_key: &str) -> Option<usize> {
        self.bubbles
            .iter()
            .position(|n| n.date.as_deref() == Some(date_key))
    }
}
