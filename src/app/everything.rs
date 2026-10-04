//! `EverythingView` + `EverythingItemCard` ports — the «Всё» grid: supported
//! types (note + typed objects), `updated_at` desc, add-card, right-click
//! context menu (pin / sticker / delete).
use gpui::{div, prelude::*, px, Context, MouseButton, MouseDownEvent, SharedString};
use gpui_component::scroll::ScrollableElement;

use memoria_model::model::Entry;
use memoria_model::object_views::entry_display_title;
use memoria_model::preview::{entry_preview, PREVIEW_LIMIT};
use memoria_model::routes::Route;
use memoria_model::sidebar_model::IconId;
use memoria_model::store::Command;
use memoria_model::system_types_data::SYSTEM_TYPE_COLLECTION_ID;

use super::{icon, Memoria};
use crate::a11y::A11y;
use crate::theme::*;

impl Memoria {
    /// Entries the Everything grid shows — Vue filters to note/book-capable
    /// types; here that means "not a collection stub and not journal-internal".
    pub(crate) fn everything_entries(&self) -> Vec<Entry> {
        self.list
            .iter()
            .filter(|e| e.type_id.as_deref() != Some(SYSTEM_TYPE_COLLECTION_ID))
            .cloned()
            .collect()
    }

    /// «Всё» screen — header + masonry-ish column wrap of cards.
    pub(crate) fn render_everything(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let entries = self.everything_entries();
        let mut grid = div()
            .id("everything-grid")
            .flex()
            .flex_wrap()
            .gap_3()
            .items_start();

        // Add card — «Новая заметка» (createNewEntry()).
        let weak = cx.weak_entity();
        grid = grid.child(
            div()
                .id("everything-add")
                .debug_selector(|| "everything-add-card".into())
                .a11y_button("Новая заметка")
                .w(px(220.))
                .h(px(140.))
                .rounded_lg()
                .border_1()
                .border_color(c(BORDER()))
                .border_dashed()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_color(c(MUTED_FG()))
                .hover(|s| s.bg(rgba(FG(), 0.04)).text_color(c(FG())))
                .on_click(move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| this.create_entry(None, cx));
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(icon(IconId::Plus, 16., rgba(FG(), 0.6)))
                        .child("Новая заметка"),
                ),
        );

        for entry in &entries {
            grid = grid.child(self.everything_card(entry, cx));
        }

        div()
            .id("everything-view")
            .debug_selector(|| "everything-view".into())
            .size_full()
            .overflow_y_scrollbar()
            .p(px(36.))
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(c(FG()))
                    .child("Всё"),
            )
            .when(entries.is_empty(), |d| {
                d.child(
                    div()
                        .pt_8()
                        .text_size(px(13.))
                        .text_color(c(MUTED_FG()))
                        .child("Пока пусто — создайте первую заметку"),
                )
            })
            .child(grid)
    }

    /// `EverythingItemCard` — title + 800-char preview + type icon; right
    /// click opens the context menu, left click navigates.
    fn everything_card(&mut self, entry: &Entry, cx: &mut Context<Self>) -> impl IntoElement {
        let id = entry.id.clone();
        let title = entry_display_title(entry);
        let preview = entry_preview(&entry.content_json, PREVIEW_LIMIT);
        let type_icon = entry
            .type_id
            .as_deref()
            .and_then(|t| self.note_types.iter().find(|nt| nt.id == t))
            .and_then(|nt| nt.icon.clone());

        let pinned = self.prefs.pinned_entry_ids.contains(&entry.id);
        let click_id = id.clone();
        let ctx_id = id.clone();
        let weak = cx.weak_entity();
        let weak_ctx = cx.weak_entity();

        div()
            .id(SharedString::from(format!("card-{id}")))
            .debug_selector(move || format!("everything-card-{id}"))
            .a11y_button(title.clone())
            .w(px(220.))
            .h(px(140.))
            .rounded_lg()
            .border_1()
            .border_color(c(BORDER()))
            .bg(c(CARD()))
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .cursor_pointer()
            .overflow_hidden()
            .hover(|s| s.bg(rgba(FG(), 0.05)))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.navigate(Route::Entry(click_id.clone()), cx)
                });
            })
            .on_mouse_down(MouseButton::Right, move |ev: &MouseDownEvent, _, cx| {
                let _ = weak_ctx.update(cx, |this, cx| {
                    let p: gpui::Point<gpui::Pixels> = ev.position;
                    this.open_ctx_menu(ctx_id.clone(), f32::from(p.x), f32::from(p.y), cx);
                });
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(icon(
                        memoria_model::sidebar_model::icon_for_name(type_icon.as_deref()),
                        15.,
                        rgba(FG(), 0.7),
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(13.))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(c(FG()))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(title),
                    )
                    .when(pinned, |d| d.child(icon(IconId::Pin, 12., c(ACCENT())))),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .text_color(c(MUTED_FG()))
                    .overflow_hidden()
                    .child(preview),
            )
    }

    /// `createNewEntry(typeId?)` — untitled entry through `saveEntry`, then
    /// navigate. Tiptap editing lands in M3; the shell still creates + opens.
    pub(crate) fn create_entry(&mut self, type_id: Option<String>, cx: &mut Context<Self>) {
        let now = memoria_model::time::now_millis();
        let entry = Entry {
            id: uuid::Uuid::new_v4().to_string(),
            title: String::new(),
            content_json: memoria_model::content::write_entry_markdown("").to_string(),
            content_loaded: Some(true),
            created_at: now,
            updated_at: now,
            type_id,
            header_props_json: Some(
                serde_json::to_string(
                    &memoria_model::entry_titles::create_untitled_entry_header_props(),
                )
                .unwrap_or_default(),
            ),
            ..Default::default()
        };
        self.send(Command::SaveEntry(Box::new(entry.clone())), cx);
        // Optimistic land (Vue pushes the unsaved entry then persists).
        self.list.insert(0, entry.clone());
        self.navigate(Route::Entry(entry.id), cx);
    }
}
