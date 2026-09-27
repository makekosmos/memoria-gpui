//! Bubble diary model — pure-Rust port of
//! `src/components/bubbles/bubbleDiaryModel.ts` (Vue SoT @7ccbb9f).
//!
//! Layout mirrors the Vue file's sections: `text` (draft parsing, tags,
//! tiptap plain text, tag stripping), `timeline` (occurrence/labels/threads),
//! `journal` (legacy dated journal import), `storage` (the
//! `memoria-bubble-diary-local-bubbles` localStorage blob),
//! `calendar` (the week model behind `BubbleDiaryCalendarSidebar`),
//! `render_model` (the display tree `BubbleTiptapRenderer` walks).

pub mod calendar;
pub mod journal;
pub mod render_model;
pub mod storage;
pub mod text;
pub mod timeline;

pub use calendar::*;
pub use journal::*;
pub use render_model::*;
pub use storage::*;
pub use text::*;
pub use timeline::*;

use serde::Serialize;
use serde_json::Value;

/// `BubbleKind` — `"plain" | "idea" | "task" | "highlight"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BubbleKind {
    #[default]
    Plain,
    Idea,
    Task,
    Highlight,
}

impl BubbleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Idea => "idea",
            Self::Task => "task",
            Self::Highlight => "highlight",
        }
    }

    /// `BUBBLE_KIND_OPTIONS[].label` — Russian UI labels.
    pub fn label(self) -> &'static str {
        match self {
            Self::Plain => "Просто",
            Self::Idea => "Идея",
            Self::Task => "Задача",
            Self::Highlight => "Подсветить",
        }
    }

    /// Non-plain kinds carry a fixed accent hex (Vue `BUBBLE_KIND_OPTIONS[].color`);
    /// `plain` resolves to the border-strong theme token at the call site.
    pub fn color(self) -> Option<u32> {
        match self {
            Self::Plain => None,
            Self::Idea => Some(0x017AFF),
            Self::Task => Some(0x4de64d),
            Self::Highlight => Some(0xFF703A),
        }
    }

    pub const OPTIONS: [BubbleKind; 4] = [Self::Plain, Self::Idea, Self::Task, Self::Highlight];
}

/// `normalizeBubbleKind` — unknown values fall back to `plain`.
pub fn normalize_bubble_kind(value: &Value) -> BubbleKind {
    match value.as_str() {
        Some("idea") => BubbleKind::Idea,
        Some("task") => BubbleKind::Task,
        Some("highlight") => BubbleKind::Highlight,
        _ => BubbleKind::Plain,
    }
}

/// `BubbleTimelineNode` — one diary row. Field names serialize camelCase so
/// `encodeLocalBubblesStorage` output matches `JSON.stringify` key order.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BubbleTimelineNode {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default)]
    pub time: String,
    /// JS `number`; `f64` keeps non-integer storage values lossless.
    /// Serialized via `js_number` so integral values write `123` like
    /// `JSON.stringify`, not `123.0`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "ser_option_f64_js"
    )]
    pub sort_key: Option<f64>,
    #[serde(default)]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_json: Option<Value>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub kind: BubbleKind,
}

/// `BubbleThreadNode` is the same shape plus the resolved `parentId`.
pub type BubbleThreadNode = BubbleTimelineNode;

/// `reply_to` ARK link fields the thread normalizer consumes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReplyLink {
    pub id: String,
    pub source_object_id: String,
    pub target_object_id: String,
}

/// `JSON.stringify` number semantics for `Option<f64>` — integral values
/// emit without a fractional part (`123`, not `123.0`).
fn ser_option_f64_js<S: serde::Serializer>(
    value: &Option<f64>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(f) if f.is_finite() && f.fract() == 0.0 && f.abs() <= 9.007_199_254_740_992e15 => {
            serializer.serialize_i64(*f as i64)
        }
        Some(f) => serializer.serialize_f64(*f),
        None => serializer.serialize_none(),
    }
}

/// `LOCAL_BUBBLES_STORAGE_KEY`.
pub const LOCAL_BUBBLES_STORAGE_KEY: &str = "memoria-bubble-diary-local-bubbles";
/// `LOCAL_BUBBLES_STORAGE_VERSION`.
pub const LOCAL_BUBBLES_STORAGE_VERSION: u64 = 1;

/// `REPLY_LINK_TYPE` from `kepler-bubble-api.ts`.
pub const REPLY_LINK_TYPE: &str = "reply_to";
