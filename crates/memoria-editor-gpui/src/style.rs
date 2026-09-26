//! Visual token mapping — Vue `TiptapEditor.vue` CSS → GPUI runs.
//!
//! | Vue CSS (`TiptapEditor.vue` / `index.css`) | GPUI |
//! |---|---|
//! | `.ProseMirror` font-sans 16px / lh 1.7 | `body` 16·z, lh 1.7 |
//! | `h1..h6` 2/1.6/1.3/1.15/1/0.92em, w700, lh1.4 | `heading` sizes, BOLD, lh 1.4 |
//! | `h5` uppercase | uppercased at row build |
//! | `h6` text-secondary | MUTED_FG |
//! | `blockquote` italic, inherit | FontStyle::Italic on quote rows |
//! | `code` mono 0.92em | mono family (size pinned per row — see DESIGN) |
//! | `pre` mono 13px pad 12/14 r8 bg surface-secondary·96%+black·4% | code rows 13px over `code_bg()` |
//! | caret `--eden-accent-color` | ACCENT |
//! | `::selection` accent 22% | rgba(ACCENT, .22) |
//! | revealed markers | MUTED_FG |
//! | placeholder | MUTED_FG 55% |
//! | image alt / link text | ACCENT underline (browser `a` parity) |

use gpui::{px, Font, FontStyle, FontWeight, Hsla, Pixels};
use imago_gpui::theme::*;

/// Body text size (Vue `font-size: 16px`).
pub const BODY_SIZE: f32 = 16.0;
/// Code block text size (Vue `pre { font-size: 13px }`).
pub const CODE_SIZE: f32 = 13.0;
/// Body line-height factor (Vue `line-height: 1.7`).
pub const BODY_LH: f32 = 1.7;
/// Heading line-height factor (Vue `h1..h6 { line-height: 1.4 }`).
pub const HEADING_LH: f32 = 1.4;
/// Gap between top-level blocks (Vue `> * { margin-bottom: 27.2px }`).
pub const BLOCK_GAP: f32 = 27.2;
/// Editor content column (Vue `.tiptap-body-shell { max-width: 760px }`).
pub const CONTENT_WIDTH: f32 = 760.0;
/// Vue `--tiptap-page-gutter: clamp(24px, 4vw, 40px)`.
pub fn gutter(bounds_w: f32) -> f32 {
    (bounds_w * 0.04).clamp(24.0, 40.0)
}
/// X origin of the text column inside element bounds.
pub fn text_origin_x(bounds_w: f32) -> f32 {
    ((bounds_w - CONTENT_WIDTH) / 2.0).max(0.0) + gutter(bounds_w)
}
/// Wrap width for the text column.
pub fn wrap_width(bounds_w: f32) -> f32 {
    (bounds_w.min(CONTENT_WIDTH) - 2.0 * gutter(bounds_w)).max(80.0)
}
/// Code block corner radius (Vue `border-radius: 8px`).
pub const CODE_RADIUS: f32 = 8.0;
/// Code block padding (Vue `padding: 12px 14px`).
pub const CODE_PAD_X: f32 = 14.0;
pub const CODE_PAD_Y: f32 = 12.0;
/// Cursor blink half-period; browsers blink ~530ms, GPUI example uses 500ms.
pub const BLINK_MS: u64 = 500;
/// Caret width.
pub const CARET_W: f32 = 2.0;

pub const FONT_SANS: &str = "Inter";
pub const FONT_MONO: &str = "JetBrains Mono";

#[derive(Clone, Copy, Debug)]
pub struct BlockStyle {
    pub size: Pixels,
    pub line_height: Pixels,
    pub weight: FontWeight,
    pub uppercase: bool,
}

/// Zoom factor — applied to every font size/line height (Vue `window.api.zoomSet`
/// scales the whole window; the editor scales text metrics).
#[derive(Clone, Copy, Debug)]
pub struct EditorScale(pub f32);

impl Default for EditorScale {
    fn default() -> Self {
        Self(1.0)
    }
}

impl EditorScale {
    pub fn size(self, base: f32) -> Pixels {
        px(base * self.0)
    }
}

/// Per-block font size/line-height (pre-zoom bases in `BODY_*` constants).
pub fn block_style(tag: &memoria_editor_core::project::BlockTag, z: EditorScale) -> BlockStyle {
    use memoria_editor_core::project::BlockTag as B;
    let (em, lh) = match tag {
        B::Heading(1) => (2.0, HEADING_LH),
        B::Heading(2) => (1.6, HEADING_LH),
        B::Heading(3) => (1.3, HEADING_LH),
        B::Heading(4) => (1.15, HEADING_LH),
        B::Heading(5) => (1.0, HEADING_LH),
        B::Heading(_) => (0.92, HEADING_LH),
        B::CodeBlock { .. } => (CODE_SIZE / BODY_SIZE, BODY_LH),
        _ => (1.0, BODY_LH),
    };
    let size = z.size(BODY_SIZE * em);
    BlockStyle {
        size,
        line_height: size * lh,
        weight: match tag {
            B::Heading(_) => FontWeight::BOLD,
            _ => FontWeight::NORMAL,
        },
        uppercase: matches!(tag, B::Heading(5)),
    }
}

pub fn text_color() -> Hsla {
    c(FG())
}
pub fn muted_color() -> Hsla {
    c(MUTED_FG())
}
pub fn accent() -> Hsla {
    c(ACCENT())
}
/// `::selection` — accent at 22% alpha.
pub fn selection_bg() -> Hsla {
    rgba(ACCENT(), 0.22)
}
/// IME marked-text underline (accent dashed-ish via wavy=false underline).
pub fn marked_underline() -> Hsla {
    c(ACCENT())
}
/// Placeholder «Начните писать...» — muted at reduced alpha like Vue's
/// `color: var(--muted-foreground)`.
pub fn placeholder_color() -> Hsla {
    muted_color()
}
/// Code block panel — `surface-secondary 96% + #000 4%`.
pub fn code_bg() -> Hsla {
    mix(SECONDARY(), 0.96, 0x000000)
}
/// Inline `code` chip background — Vue keeps `background: transparent`; a
/// faint surface mix keeps the run readable inside prose.
pub fn inline_code_bg() -> Hsla {
    fg_mix(0.07)
}
/// Revealed markdown markers (`#`, `**`, `> `) — secondary text.
pub fn marker_color() -> Hsla {
    muted_fg_mix(0.72)
}
/// Image alt-text chip + broken-image stub fill.
pub fn widget_bg() -> Hsla {
    fg_mix(0.06)
}

/// Body font — `var(--font-sans)` first usable face; the text system falls
/// back to the platform default when Inter is absent.
pub fn sans_font() -> Font {
    gpui::font(FONT_SANS)
}
/// Mono font — `var(--font-mono)` head.
pub fn mono_font() -> Font {
    gpui::font(FONT_MONO)
}

/// Resolved font for a span: mono for code marks / code blocks, else sans.
pub fn span_font(code: bool, bold: bool, italic: bool, base: &BlockStyle) -> Font {
    let mut f = if code { mono_font() } else { sans_font() };
    f.weight = if bold {
        FontWeight(base.weight.0.max(FontWeight::BOLD.0))
    } else {
        base.weight
    };
    f.style = if italic {
        FontStyle::Italic
    } else {
        FontStyle::Normal
    };
    f
}
