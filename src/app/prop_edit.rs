//! Typed-header edit plumbing — header `InputState` lifecycle, commits that
//! turn into `Command::SaveEntry` (Engine `upsert_object`), type changes,
//! and the property-picker open/apply ops (`prop_picker.rs` renders it).
use gpui::{prelude::*, Context, Window};
use gpui_component::input::{InputEvent, InputState};
use serde_json::Value;

use memoria_model::model::{Entry, ResolvedNoteTypeField};
use memoria_model::object_fields::format_object_field_value;
use memoria_model::object_views::{entry_display_title, parse_entry_header_props};
use memoria_model::store::Command;

use super::types::PropPicker;
use super::Memoria;

impl Memoria {
    /// Lazily create/reuse an `InputState` for a header field. `key` is the
    /// field id ("title"/"description"/field id); Enter and blur both commit.
    pub(crate) fn header_input(
        &mut self,
        entry_id: &str,
        key: &str,
        initial: &str,
        placeholder: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Entity<InputState> {
        if self.header_entry != entry_id {
            self.header_entry = entry_id.to_string();
            self.header_inputs.clear();
        }
        if let Some(state) = self.header_inputs.get(key) {
            return state.clone();
        }
        let state = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder(placeholder.to_string());
            if !initial.is_empty() {
                s.set_value(initial.to_string(), window, cx);
            }
            s
        });
        let owned = key.to_string();
        self._subs
            .push(cx.subscribe(&state, move |this, _, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    this.commit_header_input(&owned, cx);
                }
            }));
        self.header_inputs.insert(key.to_string(), state.clone());
        state
    }

    /// Pull the input text for `key` and write it back to the entry.
    fn commit_header_input(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(state) = self.header_inputs.get(key).cloned() else {
            return;
        };
        let text = state.read(cx).value().to_string();
        if key == "title" {
            self.set_entry_title(&text, cx);
        } else {
            self.set_header_prop(key, Value::from(text), cx);
        }
    }

    /// `headerPropChange` — update `header_props_json` on the open entry and
    /// persist via `SaveEntry` (Engine `upsert_object`).
    pub(crate) fn set_header_prop(&mut self, field_id: &str, value: Value, cx: &mut Context<Self>) {
        let Some(entry) = self.current.as_mut() else {
            return;
        };
        let mut props = parse_entry_header_props(entry);
        props.insert(field_id.to_string(), value);
        entry.header_props_json = Some(serde_json::to_string(&props).unwrap_or_default());
        let updated = entry.clone();
        if let Some(row) = self.list.iter_mut().find(|e| e.id == updated.id) {
            *row = updated.clone();
        }
        self.send(Command::SaveEntry(Box::new(updated)), cx);
        cx.notify();
    }

    /// `titleCommit`/`updateTitle` — newlines collapse to spaces, then save.
    fn set_entry_title(&mut self, title: &str, cx: &mut Context<Self>) {
        let clean = title.replace(['\r', '\n'], " ");
        let Some(entry) = self.current.as_mut() else {
            return;
        };
        if entry.title == clean {
            return;
        }
        entry.title = clean;
        let updated = entry.clone();
        if let Some(row) = self.list.iter_mut().find(|e| e.id == updated.id) {
            *row = updated.clone();
        }
        self.send(Command::SaveEntry(Box::new(updated)), cx);
        cx.notify();
    }

    /// `objectTypeChange` — resets header props via
    /// `createHeaderPropsForTypeChange` (person types inherit split title).
    pub(crate) fn set_entry_type(&mut self, type_id: &str, cx: &mut Context<Self>) {
        let Some(entry) = self.current.clone() else {
            return;
        };
        if entry.type_id.as_deref() == Some(type_id) {
            return;
        }
        let next_type = self.note_types.iter().find(|n| n.id == type_id).cloned();
        let props = memoria_model::header_props::create_header_props_for_type_change(
            next_type.as_ref(),
            &entry.title,
        )
        .unwrap_or_default();
        let mut updated = entry.clone();
        updated.type_id = Some(type_id.to_string());
        updated.header_props_json = Some(serde_json::to_string(&props).unwrap_or_default());
        if let Some(row) = self.list.iter_mut().find(|e| e.id == updated.id) {
            *row = updated.clone();
        }
        self.current = Some(updated.clone());
        self.send(Command::SaveEntry(Box::new(updated)), cx);
        cx.notify();
    }

    /// Picker options for a field — `pickerOptions` port: relation fields
    /// list candidate entries (filtered by `allowed_object_types`), selects
    /// list formatted `options`.
    pub(crate) fn open_prop_picker(
        &mut self,
        entry: &Entry,
        field: &ResolvedNoteTypeField,
        x: f32,
        y: f32,
        cx: &mut Context<Self>,
    ) {
        let multiple = field.field.kind == "multi_select"
            || (field.field.kind == "relation" && field.field.multiple != Some(false));
        let options: Vec<(String, String)> = if field.field.kind == "relation" {
            let allowed = field.field.allowed_object_types.clone().unwrap_or_default();
            self.list
                .iter()
                .filter(|e| e.id != entry.id && e.deleted_at.is_none())
                .filter(|e| {
                    allowed.is_empty()
                        || e.type_id
                            .as_deref()
                            .map(|t| allowed.iter().any(|a| a == t))
                            .unwrap_or(false)
                })
                .map(|e| (e.id.clone(), entry_display_title(e)))
                .collect()
        } else {
            field
                .field
                .options
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|o| {
                    let label = format_object_field_value(field, &Value::from(o.as_str()));
                    (o.clone(), if label.is_empty() { o } else { label })
                })
                .collect()
        };
        let props = parse_entry_header_props(entry);
        let selected = match props.get(&field.field.id) {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
            Some(Value::String(s)) if !s.is_empty() => vec![s.clone()],
            _ => Vec::new(),
        };
        self.prop_picker = Some(PropPicker {
            entry_id: entry.id.clone(),
            field_id: field.field.id.clone(),
            multiple,
            options,
            selected,
            x,
            y,
        });
        cx.notify();
    }

    /// Picker option click: single → replace + close (empty clears); multiple
    /// → toggle + persist immediately (Vue emits `update:modelValue` live).
    pub(crate) fn pick_prop_value(&mut self, value: &str, cx: &mut Context<Self>) {
        let Some(picker) = self.prop_picker.clone() else {
            return;
        };
        // Stale picker (user navigated away) — drop it, don't apply.
        if self.current.as_ref().map(|e| e.id.as_str()) != Some(picker.entry_id.as_str()) {
            self.prop_picker = None;
            cx.notify();
            return;
        }
        if picker.multiple {
            let mut next = picker.selected.clone();
            if value.is_empty() {
                next.clear();
            } else if let Some(i) = next.iter().position(|v| v == value) {
                next.remove(i);
            } else {
                next.push(value.to_string());
            }
            if let Some(p) = self.prop_picker.as_mut() {
                p.selected = next.clone();
            }
            self.set_header_prop(&picker.field_id, Value::from(next), cx);
        } else {
            self.prop_picker = None;
            if picker.field_id == "__object_type" {
                self.set_entry_type(value, cx);
            } else {
                self.set_header_prop(&picker.field_id, Value::from(value), cx);
            }
        }
    }
}
