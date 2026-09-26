//! Row spans → `TextRun`s: mark/payload styling, syntax-color merging for code
//! rows, and the IME marked-text underline.
//!
//! Code highlight: `HighlightCache::styles_for` yields `HighlightStyle`s over
//! the block's concatenated non-marker code text (`rows::code_block_text`);
//! each row segment tracks its byte range inside that string so syntax colors
//! map back onto row-local coordinates, skipping revealed fence markers.

use gpui::{px, FontStyle, FontWeight, HighlightStyle, Hsla, TextRun, UnderlineStyle};
use memoria_editor_core::project::{BlockTag, Marks, Payload, Span};
use std::ops::Range;

use crate::rows::Row;
use crate::style::{self, BlockStyle, EditorScale};

/// Style bits a `TextRun` needs beyond color (font choice + decorations).
#[derive(Clone, Copy, Default)]
struct Kind {
    code: bool,
    bold: bool,
    italic: bool,
    strike: bool,
    underline: bool,
    marked: bool,
    /// Inline `code` chip background (Vue keeps it transparent; a faint tint
    /// keeps mono runs visible — documented in DESIGN.md).
    code_bg: bool,
}

struct Seg {
    /// Row-local byte range.
    vis: Range<usize>,
    /// Range inside the block's code text — `Some` for code content only.
    code: Option<Range<usize>>,
    kind: Kind,
    color: Hsla,
}

/// Base (non-syntax) color for a span.
fn base_color(span: &Span, tag: &BlockTag) -> Hsla {
    if span.payload == Payload::Marker || span.marks.has(Marks::MARKER) {
        style::marker_color()
    } else if span.marks.has(Marks::LINK) {
        style::accent()
    } else if span.marks.has(Marks::IMAGE)
        || span.marks.has(Marks::WIDGET)
        || matches!(tag, BlockTag::Heading(6) | BlockTag::Other)
    {
        style::muted_color()
    } else {
        style::text_color()
    }
}

fn base_kind(span: &Span, tag: &BlockTag) -> Kind {
    let m = span.marks;
    let in_code_block = matches!(tag, BlockTag::CodeBlock { .. });
    Kind {
        code: m.has(Marks::CODE) || in_code_block,
        bold: m.has(Marks::BOLD),
        italic: m.has(Marks::ITALIC) || matches!(tag, BlockTag::Quote { .. }),
        strike: m.has(Marks::STRIKE),
        underline: m.has(Marks::LINK),
        marked: false,
        code_bg: m.has(Marks::CODE) && !in_code_block,
    }
}

/// Row text runs — the shaped input for `TextSystem::shape_text`.
///
/// * `code_styles`: syntax ranges over the block's code text (see module docs)
/// * `code_origin`: row's first code byte within the block's code string
/// * `marked`: IME preedit range in **visible** projection coords
pub fn runs_for_row(
    row: &Row,
    proj_text: &str,
    code_styles: Option<&[(Range<usize>, HighlightStyle)]>,
    code_origin: usize,
    marked: Option<Range<usize>>,
    z: EditorScale,
) -> Vec<TextRun> {
    let _ = proj_text;
    let st = style::block_style(&row.tag, z);
    let mut segs: Vec<Seg> = Vec::new();
    let mut code_cursor = code_origin;
    for span in &row.spans {
        if span.vis.is_empty() {
            continue;
        }
        let local = span.vis.start - row.vis.start..span.vis.end - row.vis.start;
        let is_code =
            matches!(row.tag, BlockTag::CodeBlock { .. }) && span.payload != Payload::Marker;
        let code = is_code.then(|| {
            let r = code_cursor..code_cursor + span.vis.len();
            code_cursor += span.vis.len();
            r
        });
        segs.push(Seg {
            vis: local,
            code,
            kind: base_kind(span, &row.tag),
            color: base_color(span, &row.tag),
        });
    }
    fill_gaps(&mut segs, row.vis.end - row.vis.start, &row.tag);
    if let Some(styles) = code_styles {
        overlay_code(&mut segs, styles);
    }
    if let Some(m) = marked {
        let local = m.start.max(row.vis.start)..m.end.min(row.vis.end);
        if !local.is_empty() {
            split(
                &mut segs,
                local.start - row.vis.start..local.end - row.vis.start,
                |s| s.kind.marked = true,
            );
        }
    }
    segs.into_iter()
        .filter(|s| !s.vis.is_empty())
        .map(|s| seg_to_run(s, &st, z))
        .collect()
}

/// Segments may not cover the whole row (empty spans are dropped); fill with
/// default-styled runs so `TextRun`s tile `[0, row_len)`.
fn fill_gaps(segs: &mut Vec<Seg>, len: usize, tag: &BlockTag) {
    let mut pos = 0;
    let mut i = 0;
    while pos < len {
        if i < segs.len() && segs[i].vis.start <= pos {
            pos = segs[i].vis.end.max(pos);
            i += 1;
            continue;
        }
        let end = segs.get(i).map(|s| s.vis.start).unwrap_or(len);
        let span = Span {
            vis: pos..end,
            src: 0..0,
            marks: Marks::default(),
            block: tag.clone(),
            payload: Payload::None,
        };
        segs.insert(
            i,
            Seg {
                vis: pos..end,
                code: None,
                kind: base_kind(&span, tag),
                color: base_color(&span, tag),
            },
        );
        pos = end;
        i += 1;
    }
}

/// Split the segs whose `code` range intersects `[r0, r1)` per style.
fn overlay_code(segs: &mut Vec<Seg>, styles: &[(Range<usize>, HighlightStyle)]) {
    for (r, st) in styles {
        let mut out = Vec::with_capacity(segs.len() + 2);
        for seg in segs.drain(..) {
            let Some(cr) = seg.code.clone() else {
                out.push(seg);
                continue;
            };
            let ov = cr.start.max(r.start)..cr.end.min(r.end);
            if ov.is_empty() {
                out.push(seg);
                continue;
            }
            let (v0, v1) = (seg.vis.start, seg.vis.end);
            if cr.start < ov.start {
                let keep = ov.start - cr.start;
                out.push(Seg {
                    vis: v0..v0 + keep,
                    code: Some(cr.start..ov.start),
                    kind: seg.kind,
                    color: seg.color,
                });
            }
            let mut mid = Seg {
                vis: v0 + (ov.start - cr.start)..v1 - (cr.end - ov.end),
                code: Some(ov.clone()),
                kind: seg.kind,
                color: st.color.unwrap_or(seg.color),
            };
            if let Some(w) = st.font_weight {
                mid.kind.bold = w >= FontWeight::SEMIBOLD;
            }
            if st.font_style == Some(FontStyle::Italic) {
                mid.kind.italic = true;
            }
            out.push(mid);
            if ov.end < cr.end {
                let tail = cr.end - ov.end;
                out.push(Seg {
                    vis: v1 - tail..v1,
                    code: Some(ov.end..cr.end),
                    kind: seg.kind,
                    color: seg.color,
                });
            }
        }
        *segs = out;
    }
}

/// Split segs covering `r` (row-local) and apply `f` to the middle pieces.
fn split(segs: &mut Vec<Seg>, r: Range<usize>, mut f: impl FnMut(&mut Seg)) {
    let mut out = Vec::with_capacity(segs.len() + 2);
    for seg in segs.drain(..) {
        let ov = seg.vis.start.max(r.start)..seg.vis.end.min(r.end);
        if ov.is_empty() {
            out.push(seg);
            continue;
        }
        let base_code = seg.code;
        let (kind, color) = (seg.kind, seg.color);
        if seg.vis.start < ov.start {
            let keep = ov.start - seg.vis.start;
            out.push(Seg {
                vis: seg.vis.start..ov.start,
                code: base_code.clone().map(|c| c.start..c.start + keep),
                kind,
                color,
            });
        }
        let mut mid = Seg {
            vis: ov.clone(),
            code: base_code
                .clone()
                .map(|c| c.start + (ov.start - seg.vis.start)..c.start + (ov.end - seg.vis.start)),
            kind,
            color,
        };
        f(&mut mid);
        out.push(mid);
        if ov.end < seg.vis.end {
            out.push(Seg {
                vis: ov.end..seg.vis.end,
                code: base_code.map(|c| {
                    c.start + (ov.end - seg.vis.start)..c.start + (seg.vis.end - seg.vis.start)
                }),
                kind,
                color,
            });
        }
    }
    *segs = out;
}

fn seg_to_run(s: Seg, st: &BlockStyle, z: EditorScale) -> TextRun {
    let _ = z;
    TextRun {
        len: s.vis.end - s.vis.start,
        font: style::span_font(s.kind.code, s.kind.bold, s.kind.italic, st),
        color: s.color,
        background_color: s.kind.code_bg.then(style::inline_code_bg),
        underline: if s.kind.marked {
            Some(UnderlineStyle {
                thickness: px(1.),
                color: Some(style::marked_underline()),
                wavy: false,
            })
        } else if s.kind.underline {
            Some(UnderlineStyle {
                thickness: px(1.),
                color: Some(style::accent()),
                wavy: false,
            })
        } else {
            None
        },
        strikethrough: s.kind.strike.then(|| gpui::StrikethroughStyle {
            thickness: px(1.),
            color: Some(s.color),
        }),
    }
}
