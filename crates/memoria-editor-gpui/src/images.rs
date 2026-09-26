//! Image rows: `Payload::Image` src → `Resource` → platform asset pipeline
//! (`ImgResourceLoader` handles fs/HTTP + decode lazily and notifies on
//! completion). Loading → skeleton stub; decode error → broken-image stub
//! (Vue `objectImages.ts` parity).

use gpui::{fill, point, px, quad, size, App, Bounds, Pixels, Point, Resource, SharedUri, Window};
use std::path::PathBuf;
use std::sync::Arc;

use crate::editor::MemoriaEditor;
use crate::style;

/// `kosmos-local-image://file/<pct>` — the core's `local_image_url` scheme.
pub const LOCAL_IMAGE_SCHEME: &str = "kosmos-local-image://file/";

/// Resolve a markdown image `src` to a GPUI `Resource`.
/// * `kosmos-local-image://file/<enc>` → decoded fs path
/// * `http(s)://…` / other URI with a scheme → `Uri`
/// * absolute `/…` or `X:\…` → `Path`
/// * relative → joined on `image_root` when the app set one, else `Uri`
///   (asset-source lookup).
pub fn resolve(e: &MemoriaEditor, src: &str) -> Option<Resource> {
    let t = src.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(rest) = t.strip_prefix(LOCAL_IMAGE_SCHEME) {
        return Some(Resource::Path(Arc::from(
            PathBuf::from(percent_decode(rest)).as_path(),
        )));
    }
    if t.starts_with("http://") || t.starts_with("https://") {
        return Some(Resource::Uri(SharedUri::from(t.to_string())));
    }
    if t.contains("://") {
        return Some(Resource::Uri(SharedUri::from(t.to_string())));
    }
    let p = PathBuf::from(t);
    if p.is_absolute() || looks_like_windows_path(t) {
        return Some(Resource::Path(p.into()));
    }
    match &e.image_root {
        Some(root) => Some(Resource::Path(root.join(t).into())),
        None => Some(Resource::Uri(SharedUri::from(t.to_string()))),
    }
}

fn looks_like_windows_path(t: &str) -> bool {
    let b = t.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
}

/// Percent-decode (UTF-8) — inverse of the `encodeURIComponent` used by
/// `localImageUrl` in `localImages.ts`.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Paint an image row. `h` is the row's measured height (a fixed preview
/// height — Vue caps the block at the column width and keeps aspect).
pub fn paint_image_row(
    e: &MemoriaEditor,
    src: &str,
    origin: Point<Pixels>,
    h: Pixels,
    window: &mut Window,
    cx: &mut App,
) {
    let col_w = px(style::wrap_width(f32::from(e.bounds.size.width)));
    match resolve(e, src) {
        Some(res) => {
            match window.use_asset::<gpui::ImgResourceLoader>(&res, cx) {
                Some(Ok(img)) => {
                    // Fit inside (col_w × h), keep aspect (contain).
                    let scale = window.scale_factor();
                    let nw = px(img.size(0).width.0 as f32 / scale);
                    let nh = px(img.size(0).height.0 as f32 / scale);
                    if nw <= px(0.) || nh <= px(0.) {
                        // Zero-size decode — same broken stub as an error.
                        stub(origin, h, col_w, true, window);
                        return;
                    }
                    let r = (col_w / nw).min(h / nh).min(1.0);
                    let (w, hh) = (nw * r, nh * r);
                    let b = Bounds::new(origin, size(w, hh));
                    window
                        .paint_image(
                            b,
                            b,
                            gpui::Corners::all(px(style::CODE_RADIUS)),
                            img,
                            0,
                            false,
                        )
                        .ok();
                }
                Some(Err(_)) => stub(origin, h, col_w, true, window),
                None => stub(origin, h, col_w, false, window),
            }
        }
        None => stub(origin, h, col_w, true, window),
    }
}

/// Loading / broken stub — a muted rounded rect (Vue shows a skeleton while
/// `img` loads and a broken-file chip on error).
fn stub(origin: Point<Pixels>, h: Pixels, w: Pixels, broken: bool, window: &mut Window) {
    let bg = if broken {
        style::widget_bg()
    } else {
        style::inline_code_bg()
    };
    window.paint_quad(quad(
        Bounds::new(origin, size(w.min(px(320.)), h)),
        gpui::Corners::all(px(style::CODE_RADIUS)),
        bg,
        gpui::Edges::default(),
        gpui::transparent_black(),
        gpui::BorderStyle::default(),
    ));
    if broken {
        // Centered marker dot — minimal "image failed" affordance.
        window.paint_quad(fill(
            Bounds::new(
                point(
                    origin.x + w.min(px(320.)) / 2. - px(3.),
                    origin.y + h / 2. - px(3.),
                ),
                size(px(6.), px(6.)),
            ),
            style::marker_color(),
        ));
    }
}

/// Pixels an image row should occupy before the image is measured.
pub const IMAGE_ROW_H: f32 = 64.;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_local_image_scheme() {
        let p = percent_decode("tmp%20dir%2Fx%20y.png");
        assert_eq!(p, "tmp dir/x y.png");
    }
}
