//! Port of `src/lib/kepler-bubble-api.ts` — create/edit/delete/list/migrate
//! diary bubbles over the `ArkBridge` seam. Bubbles are `com.kosmos.note`
//! (or legacy `system-type-journal`/`note_obj`) objects with
//! `propsJson.entry_kind === "bubble"`; replies are `reply_to` object links.

use serde_json::{json, Map, Value};

use crate::diary::{
    bubble_occurrence_millis, bubble_plain_text, format_bubble_date_key, normalize_bubble_kind,
    normalize_bubble_threads, normalize_tags, parse_bubble_draft, plain_text_to_tiptap_doc,
    BubbleKind, BubbleTimelineNode, ReplyLink, REPLY_LINK_TYPE,
};
use crate::model::{ArkObjectLink, ArkObjectRecord};

pub use super::bubble_migrate::{migrate_diary, migrate_journal_entry, migrate_local_blob};
use crate::store::transport::{ArkBridge, EngineError};

/// The three type ids `listRaw` scans (`isBubbleObject` accepts all three).
pub const BUBBLE_TYPE_IDS: [&str; 3] = ["com.kosmos.note", "system-type-journal", "note_obj"];

/// `createBubbleApi(ark)` — cloneable over any `ArkBridge`.
#[derive(Clone)]
pub struct BubbleApi<B: ArkBridge> {
    bridge: B,
}

/// `updateBubble` patch — `input` replaces text/tags/content; `kind` only
/// flips the kind (content, including unknown tiptap nodes, is preserved).
#[derive(Clone, Debug, Default)]
pub struct BubblePatch {
    pub input: Option<String>,
    pub kind: Option<BubbleKind>,
}

impl<B: ArkBridge> BubbleApi<B> {
    pub fn new(bridge: B) -> Self {
        Self { bridge }
    }

    /// `bubble_migrate` impl lives in a sibling module.
    pub(crate) fn bridge(&self) -> &B {
        &self.bridge
    }

    /// `listRaw` — all bubble objects (deduped by id) + every object link.
    fn list_raw(&self) -> Result<(Vec<ArkObjectRecord>, Vec<ArkObjectLink>), EngineError> {
        let mut by_id: Map<String, Value> = Map::new();
        for type_id in BUBBLE_TYPE_IDS {
            // `.catch(() => [])` — a missing type id must not fail the list.
            for object in self
                .bridge
                .list_objects_by_type(type_id)
                .unwrap_or_default()
            {
                by_id.insert(
                    object
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .into(),
                    object,
                );
            }
        }
        let objects: Vec<ArkObjectRecord> = by_id
            .into_values()
            .filter_map(|v| serde_json::from_value(v).ok())
            .filter(|o: &ArkObjectRecord| is_bubble_object(o))
            .collect();
        let links: Vec<ArkObjectLink> = self
            .bridge
            .list_object_links()?
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();
        Ok((objects, links))
    }

    /// `listBubbles` — thread-normalized bubbles; invalid reply links are
    /// cleaned up in the background like Vue's `Promise.all(...catch(()=>{}))`.
    pub fn list_bubbles(&self) -> Result<Vec<BubbleTimelineNode>, EngineError> {
        let (objects, links) = self.list_raw()?;
        let reply_links: Vec<ReplyLink> = links
            .iter()
            .filter(|l| l.link_type == REPLY_LINK_TYPE)
            .map(|l| ReplyLink {
                id: l.id.clone(),
                source_object_id: l.source_object_id.clone(),
                target_object_id: l.target_object_id.clone(),
            })
            .collect();
        let bubbles: Vec<BubbleTimelineNode> = objects.iter().map(object_to_bubble).collect();
        let (threaded, invalid_ids) = normalize_bubble_threads(bubbles, &reply_links);
        for id in invalid_ids {
            let _ = self.bridge.delete_object_link(&id);
        }
        Ok(threaded)
    }

    /// `createBubble` — returns the new bubble id. `parentId` must exist and
    /// be a root.
    pub fn create_bubble(
        &self,
        input: &str,
        kind: BubbleKind,
        parent_id: Option<&str>,
        content_json: Option<Value>,
    ) -> Result<String, EngineError> {
        let (text, tags) = parse_bubble_draft(input);
        if text.is_empty() {
            return Err(engine_err("Bubble text is empty"));
        }
        let now = crate::time::now_millis();
        let id = uuid::Uuid::new_v4().to_string();
        let node = BubbleTimelineNode {
            id: id.clone(),
            created_at: Some(crate::time::millis_to_iso(now)),
            updated_at: Some(crate::time::millis_to_iso(now)),
            content_json: Some(content_json.unwrap_or_else(|| plain_text_to_tiptap_doc(&text))),
            text,
            tags,
            kind,
            ..Default::default()
        };
        self.bridge.upsert_object(bubble_object(&node, None)?)?;
        if let Some(parent_id) = parent_id {
            let timeline = self.list_bubbles()?;
            let parent = timeline.iter().find(|b| b.id == parent_id);
            match parent {
                Some(p) if p.parent_id.is_none() => {}
                _ => return Err(engine_err("Replies can target roots only")),
            }
            self.bridge.upsert_object_link(json!({
                "id": link_id(&id, parent_id),
                "sourceObjectId": id,
                "targetObjectId": parent_id,
                "linkType": REPLY_LINK_TYPE,
                "createdAt": crate::time::millis_to_iso(now),
            }))?;
        }
        Ok(id)
    }

    /// `updateBubble` — preserves `createdAt` and unrelated `propsJson` keys.
    pub fn update_bubble(&self, id: &str, patch: BubblePatch) -> Result<(), EngineError> {
        let existing: ArkObjectRecord = serde_json::from_value(self.bridge.get_object(id)?)
            .map_err(|_| engine_err("Bubble not found"))?;
        if !is_bubble_object(&existing) {
            return Err(engine_err("Bubble not found"));
        }
        let current = object_to_bubble(&existing);
        let draft = patch.input.as_deref().map(parse_bubble_draft);
        if let Some((text, _)) = &draft {
            if text.is_empty() {
                return Err(engine_err("Bubble text is empty"));
            }
        }
        let next = BubbleTimelineNode {
            text: draft.as_ref().map(|d| d.0.clone()).unwrap_or(current.text),
            content_json: match &draft {
                Some((text, _)) => Some(plain_text_to_tiptap_doc(text)),
                None => current.content_json,
            },
            tags: draft.map(|d| d.1).unwrap_or(current.tags),
            kind: patch.kind.unwrap_or(current.kind),
            updated_at: Some(crate::time::millis_to_iso(crate::time::now_millis())),
            ..current
        };
        self.bridge
            .upsert_object(bubble_object(&next, Some(&existing))?)?;
        Ok(())
    }

    /// `deleteBubble` — remove every `reply_to` link touching the node, then
    /// the object itself. Deleting a root promotes children to roots.
    pub fn delete_bubble(&self, id: &str) -> Result<(), EngineError> {
        let links: Vec<ArkObjectLink> = self
            .bridge
            .list_object_links()?
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();
        for link in links.iter().filter(|l| {
            l.link_type == REPLY_LINK_TYPE && (l.source_object_id == id || l.target_object_id == id)
        }) {
            self.bridge.delete_object_link(&link.id)?;
        }
        self.bridge.delete_object(id)?;
        Ok(())
    }
}

pub(crate) fn engine_err(message: &str) -> EngineError {
    EngineError::Rpc(message.to_string())
}

/// `isBubbleObject` — recognized type, `entry_kind === "bubble"`, not deleted.
pub fn is_bubble_object(object: &ArkObjectRecord) -> bool {
    BUBBLE_TYPE_IDS.contains(&object.type_id.as_str())
        && object.props_json.get("entry_kind").and_then(Value::as_str) == Some("bubble")
        // `!object.deletedAt` — absent/null/"" are all "not deleted".
        && object
            .deleted_at
            .as_ref()
            .map(|v| !crate::model::js_truthy(v))
            .unwrap_or(true)
}

/// `objectToBubble` — `date`/`time`/`sortKey` derive from `createdAt`.
fn object_to_bubble(object: &ArkObjectRecord) -> BubbleTimelineNode {
    let content_json = crate::content::read_entry_tiptap_doc(&object.content_json);
    // `new Date(object.createdAt)` — ISO strings and epoch ms both parse.
    let created =
        crate::time::timestamp_to_millis(&object.created_at).or_else(|| object.created_at.as_i64());
    let valid = created.is_some();
    let created = created.unwrap_or(0);
    BubbleTimelineNode {
        id: object.id.clone(),
        created_at: match &object.created_at {
            Value::String(s) => Some(s.clone()),
            Value::Null => valid.then(|| crate::time::millis_to_iso(created)),
            // Non-string wire values (e.g. epoch ms) are preserved verbatim
            // like Vue; occurrence still resolves via `date`/`time` below.
            other => Some(other.to_string()),
        },
        updated_at: match &object.updated_at {
            Value::String(s) => Some(s.clone()),
            // Vue passes `updatedAt` through verbatim — a non-string value
            // (e.g. numeric) must stay different from `createdAt` so the
            // «изменено» marker still fires; stringifying keeps it.
            Value::Null => valid.then(|| crate::time::millis_to_iso(created)),
            other => Some(other.to_string()),
        },
        date: valid.then(|| format_bubble_date_key(created)),
        time: if valid {
            let c = crate::local_time::local_civil(created);
            format!("{:02}:{:02}", c.hour, c.minute)
        } else {
            String::new()
        },
        sort_key: valid.then_some(created as f64),
        text: bubble_plain_text(&content_json),
        content_json: Some(content_json),
        tags: normalize_tags(object.props_json.get("tags").unwrap_or(&Value::Null)),
        kind: normalize_bubble_kind(object.props_json.get("bubble_kind").unwrap_or(&Value::Null)),
        ..Default::default()
    }
}

/// `linkId(childId, parentId)`.
fn link_id(child_id: &str, parent_id: &str) -> String {
    format!("{child_id}:{REPLY_LINK_TYPE}:{parent_id}")
}

/// `bubbleObject` — the persisted wire shape.
pub(crate) fn bubble_object(
    bubble: &BubbleTimelineNode,
    existing: Option<&ArkObjectRecord>,
) -> Result<Value, EngineError> {
    let occurrence = bubble_occurrence_millis(bubble)
        .ok_or_else(|| engine_err("Bubble occurrence timestamp is unresolved"))?;
    let created_at = existing
        .map(|e| e.created_at.clone())
        .filter(|v| !v.is_null())
        .unwrap_or_else(|| json!(crate::time::millis_to_iso(occurrence)));
    let updated_at = bubble
        .updated_at
        .clone()
        .map(Value::from)
        .unwrap_or_else(|| json!(crate::time::millis_to_iso(crate::time::now_millis())));
    let content_json = crate::content::write_entry_tiptap_doc(
        bubble
            .content_json
            .clone()
            .unwrap_or_else(|| plain_text_to_tiptap_doc(&bubble.text)),
    );
    let mut props = existing
        .and_then(|e| e.props_json.as_object().cloned())
        .unwrap_or_default();
    props.insert("entry_kind".into(), json!("bubble"));
    props.insert("bubble_kind".into(), json!(bubble.kind.as_str()));
    props.insert("tags".into(), json!(normalize_tags(&json!(bubble.tags))));
    Ok(json!({
        "id": bubble.id,
        "typeId": "com.kosmos.note",
        "title": bubble.text.chars().take(80).collect::<String>(),
        "contentJson": content_json,
        "propsJson": Value::Object(props),
        "createdAt": created_at,
        "updatedAt": updated_at,
        "deletedAt": Value::Null,
    }))
}
