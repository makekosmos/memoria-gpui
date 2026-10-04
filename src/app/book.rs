//! `BookCover.vue` port — spine-colored cover (dominant color via Engine
//! `images.dominantColor`) with `+` fallback; click opens the cover modal
//! (`cover_modal.rs`). The app renders only Engine-returned local paths.
use gpui::{div, img, prelude::*, px, Context, SharedString};

use memoria_model::store::Command;

use super::Memoria;
use crate::a11y::A11y;
use crate::theme::*;

/// "rgb(136 86 41)" (Engine color format) → gpui `rgb()` for the spine strip.
fn spine_rgb(value: &str) -> Option<u32> {
    let inner = value.trim().strip_prefix("rgb(")?.strip_suffix(')')?;
    let mut parts = inner
        .split_whitespace()
        .filter_map(|p| p.parse::<u32>().ok());
    let (r, g, b) = (parts.next()?, parts.next()?, parts.next()?);
    Some((r << 16) | (g << 8) | b)
}

impl Memoria {
    /// `BookCover` — image + spine strip (dominant color via
    /// `images.dominantColor`), or the `+` fallback. Click opens the cover
    /// modal (Vue `openCoverModal`).
    pub(crate) fn book_cover_el(
        &mut self,
        src: &str,
        _alt: &str,
        image_field_id: String,
        _entry_id: &str,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        if !src.is_empty() {
            self.request_spine_color(src, cx);
        }
        let spine = self
            .spine_colors
            .get(src)
            .and_then(|c| c.as_deref())
            .and_then(spine_rgb)
            .map(gpui::rgb);
        let weak = cx.weak_entity();
        let mut el = div()
            .id("book-cover")
            .debug_selector(|| "book-cover".into())
            .a11y_button("Изменить обложку")
            .relative()
            .w(px(104.))
            .h(px(150.))
            .rounded_md()
            .overflow_hidden()
            .bg(rgba(FG(), 0.08))
            .cursor_pointer()
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.open_cover_modal(&image_field_id, window, cx);
                });
            });
        if src.is_empty() {
            el = el.child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(32.))
                    .font_weight(gpui::FontWeight::LIGHT)
                    .text_color(rgba(FG(), 0.42))
                    .child("+"),
            );
        } else {
            el = el
                .child(
                    img(SharedString::from(src.to_string()))
                        .size_full()
                        .object_fit(gpui::ObjectFit::Cover),
                )
                .child(
                    // Spine overlay: dominant color strip + gloss lines
                    // (Vue `book-cover__layer` gradient).
                    div().absolute().inset_0().child(
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .bottom_0()
                            .w(px(4.))
                            .bg(spine.map_or_else(|| c(CARD()), Into::into)),
                    ),
                );
        }
        el.into_any_element()
    }

    fn request_spine_color(&mut self, src: &str, cx: &mut Context<Self>) {
        if !self.spine_colors.contains_key(src) && self.spine_pending.insert(src.to_string()) {
            self.send(Command::DominantColor(src.to_string()), cx);
        }
    }

    /// `images.fetch` reply — remote URL → stored local path for `img()`.
    /// The Engine-computed dominant color doubles as the spine color.
    pub(crate) fn on_image_fetched(
        &mut self,
        url: String,
        result: Result<(String, Option<String>), String>,
        cx: &mut Context<Self>,
    ) {
        self.image_pending.remove(&url);
        match result {
            Ok((path, color)) => {
                if let Some(color) = color {
                    self.spine_colors.insert(path.clone(), Some(color));
                }
                self.image_cache.insert(url, path);
            }
            Err(_) => {
                self.image_cache.insert(url, String::new());
            }
        }
        cx.notify();
    }
}
