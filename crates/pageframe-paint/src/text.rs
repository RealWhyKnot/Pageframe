use anyrender::PaintScene;
use kurbo::{
    Affine, BezPath, Cap, Circle, PathEl, Point, Rect, RoundedRect, Shape as _, Stroke, Vec2,
};
use num_traits::ToPrimitive;
use pageframe_dom::CopyStyleGetters;
use pageframe_dom::inline_trim::{
    LineSpan, LineTrim, is_reordered_preserved_space, settle_line_spans,
};
use pageframe_dom::{BaseDocument, NodeId, node::TextBrush, util::ToColorColor};
use parley::{Affinity, Cursor, Layout, Line, PositionedLayoutItem, Selection};
use peniko::Fill;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use style::properties::generated::longhands::text_decoration_style::computed_value::T as TextDecorationStyle;
use style::properties::generated::longhands::visibility::computed_value::T as Visibility;
use style::values::computed::{
    Display, Length, LengthPercentage, TextDecorationLength, TextDecorationLine,
    TextUnderlinePosition,
};
use style::values::generics::text::{GenericTextDecorationInset, GenericTextDecorationLength};

use crate::color::{Color, ToColorColor as _};
use crate::image_cache::ImageCache;
use crate::{FONT_EMBOLDEN_ENABLED, SELECTION_COLOR};

#[cfg(target_os = "windows")]
mod colr;
mod decoration;
mod fragments;
mod ink;
mod runs;
mod selection;
mod shadow;
#[cfg(target_os = "windows")]
mod windows;

use decoration::flush_line_decorations;
pub(crate) use fragments::{InlineFragment, inline_fragments};
use fragments::{covers_its_start, push_spacer_holes, relative_shift};
pub(crate) use runs::stroke_text;
pub(crate) use selection::{GroupedRun, SelectionPaint, draw_text_selection, grouped_runs};
use shadow::blurred_shadow_image;
pub(crate) use shadow::gaussian_blur;
mod quarter;
mod sampled;
mod upright;

/// Per-font-face cache of the OS/2 `usWinAscent / unitsPerEm` ratio, keyed by the font
/// blob's unique id and face index. `None` is cached for fonts without a usable OS/2 or
/// head table so they aren't re-parsed either.
type WinAscentCache = HashMap<(u64, u32), Option<f32>>;

/// A text decoration resolved from a single "decorating box" (an element that
/// sets `text-decoration-line`), ready to be drawn over a glyph run. Because the
/// `text-decoration-*` properties do not inherit, decorations are propagated from
/// ancestors rather than inherited, so a run may be covered by several of these.
///
/// Only the run-independent (style-derived) values are stored here so an instance
/// can be cached on the ancestor stack and reused across every run within the same
/// decorating box. The `text-decoration-inset` and `text-underline-offset` values
/// depend on the run's advance/font-size and are resolved lazily when drawing.
#[derive(Clone)]
struct ResolvedDecoration {
    line: TextDecorationLine,
    style: TextDecorationStyle,
    color: Color,
    thickness: TextDecorationLength,
    /// `text-underline-offset`; `None` for `auto`. Resolved per run.
    underline_offset: Option<LengthPercentage>,
    /// Whether `text-underline-position: under` is in effect.
    underline_under: bool,
    /// Whether `text-underline-position: from-font` is in effect.
    underline_from_font: bool,
    /// `text-decoration-inset`. Resolved per run.
    inset: GenericTextDecorationInset<LengthPercentage>,
    skips_ink: bool,
}

/// An element on the current ancestor path (inline root -> run node), caching the
/// values resolved from its computed style so descending into the same node across
/// consecutive runs doesn't re-resolve them.
struct DecorationStackEntry {
    node_id: NodeId,
    /// This node's (inherited) text fill colour, used for the glyphs of runs whose
    /// innermost node is this one.
    fill_color: Color,
    /// The decoration this node introduces as a decorating box, if any.
    decoration: Option<ResolvedDecoration>,
    /// This node's (inherited) `text-shadow` list, in CSS pixels.
    shadows: Vec<ResolvedTextShadow>,
    shift: Vec2,
    group: Option<NodeId>,
}

struct ResolvedTextShadow {
    color: Color,
    offset: kurbo::Vec2,
    blur: f64,
}

/// Resolve the cached style values for a single node into a [`DecorationStackEntry`].
fn resolve_decoration_entry(doc: &BaseDocument, node_id: NodeId) -> DecorationStackEntry {
    let Some(styles) = doc.get_node(node_id).and_then(|node| node.primary_styles()) else {
        return DecorationStackEntry {
            node_id,
            fill_color: Color::BLACK,
            decoration: None,
            shadows: Vec::new(),
            shift: Vec2::ZERO,
            group: None,
        };
    };

    let itext = styles.get_inherited_text();
    let text = styles.get_text();
    let text_color = itext.color.as_color_color();
    let fill_color = itext
        ._webkit_text_fill_color
        .resolve_to_absolute(&itext.color)
        .as_srgb_color();

    let drawn_lines = TextDecorationLine::UNDERLINE
        | TextDecorationLine::OVERLINE
        | TextDecorationLine::LINE_THROUGH;
    let line = text.text_decoration_line;
    // Decorations propagate through the box tree, and a `display: contents` element
    // generates no box, so its decorations have no effect on descendants.
    let is_contents = styles.clone_display().is_contents();
    let decoration = (!is_contents && line.intersects(drawn_lines)).then(|| {
        // `text-decoration-color: currentColor` (the initial value) resolves against
        // the decorating box's own colour, not the descendant run's.
        let color = text
            .text_decoration_color
            .as_absolute()
            .map(ToColorColor::as_color_color)
            .unwrap_or(text_color);

        ResolvedDecoration {
            line,
            style: text.text_decoration_style,
            color,
            thickness: text.text_decoration_thickness.clone(),
            underline_offset: itext.text_underline_offset.non_auto(),
            underline_under: itext
                .text_underline_position
                .contains(TextUnderlinePosition::UNDER),
            underline_from_font: itext
                .text_underline_position
                .contains(TextUnderlinePosition::FROM_FONT),
            inset: text.text_decoration_inset.clone(),
            skips_ink: !matches!(
                itext.text_decoration_skip_ink,
                style::values::computed::TextDecorationSkipInk::None
            ),
        }
    });

    let shadows = itext
        .text_shadow
        .0
        .iter()
        .map(|shadow| ResolvedTextShadow {
            color: shadow
                .color
                .resolve_to_absolute(&itext.color)
                .as_srgb_color(),
            offset: kurbo::Vec2::new(shadow.horizontal.px() as f64, shadow.vertical.px() as f64),
            blur: shadow.blur.px() as f64,
        })
        .collect();

    let is_inline = styles.get_box().display == Display::Inline;
    DecorationStackEntry {
        node_id,
        fill_color,
        decoration,
        shadows,
        shift: match doc.get_node(node_id) {
            Some(node) if is_inline => relative_shift(node),
            _ => Vec2::ZERO,
        },
        group: starts_group(&styles).then_some(node_id),
    }
}

fn starts_group(styles: &style::properties::ComputedValues) -> bool {
    styles.get_box().display == Display::Inline
        && (styles.get_effects().opacity < 1.0 || is_positioned(styles))
}

pub(crate) fn run_group(
    doc: &BaseDocument,
    node_id: NodeId,
    inline_root_id: NodeId,
) -> Option<NodeId> {
    let mut next_id = Some(node_id);
    while let Some(node) = next_id
        .filter(|node_id| *node_id != inline_root_id)
        .and_then(|node_id| doc.get_node(node_id))
    {
        if node
            .primary_styles()
            .is_some_and(|styles| starts_group(&styles))
        {
            return Some(node.id);
        }
        next_id = node.parent;
    }
    None
}

pub(crate) fn parent_group(
    doc: &BaseDocument,
    group: NodeId,
    inline_root_id: NodeId,
) -> Option<NodeId> {
    let mut next_id = doc.get_node(group).and_then(|node| node.parent);
    while let Some(node) = next_id
        .filter(|node_id| *node_id != inline_root_id)
        .and_then(|node_id| doc.get_node(node_id))
    {
        next_id = node.parent;
        if node
            .primary_styles()
            .is_some_and(|styles| starts_group(&styles))
        {
            return Some(node.id);
        }
    }
    None
}

fn is_positioned(styles: &style::properties::ComputedValues) -> bool {
    styles.get_box().position != style::computed_values::position::T::Static
}

/// The run-dependent geometry used to position and size a decoration, captured from a
/// single glyph run. Firefox draws a decoration once for the whole decorating box using
/// the box's own font, so we prefer the values from a run whose innermost node *is* the
/// decorating box (its own text), falling back to the first run it covers.
#[derive(Clone)]
struct DecorationRunGeometry {
    /// The baseline the decoration is positioned from (the run's own, `vertical-align`
    /// shifted, baseline).
    baseline: f32,
    ascent: f32,
    descent: f32,
    underline_offset: f32,
    underline_size: f32,
    strikethrough_size: f32,
    font: parley::FontData,
    font_size: f32,
    css_font_size: f64,
}

/// A decoration accumulated across the runs of a single line for one decorating box.
///
/// The `text-decoration-*` properties describe the whole box, so we gather the horizontal
/// extent it covers on the line (`min_x`/`max_x`) and the font geometry to draw with, then
/// paint a single line spanning the box rather than one segment per run.
struct LineDecoration {
    node_id: NodeId,
    deco: ResolvedDecoration,
    min_x: f64,
    max_x: f64,
    /// Geometry from a run whose innermost node is the decorating box itself.
    own: Option<DecorationRunGeometry>,
    /// Geometry from the first run the box covers (fallback when it has no own text on
    /// this line, e.g. it only wraps differently-sized descendants).
    first: Option<DecorationRunGeometry>,
    runs: SmallVec<[usize; 4]>,
    shift: Vec2,
}

/// Reusable scratch storage for drawing text across all inline formatting contexts in a
/// document. The vectors are cleared between uses without releasing their allocations.
#[derive(Default)]
pub(crate) struct DrawTextContext {
    stack: Vec<DecorationStackEntry>,
    path_scratch: Vec<NodeId>,
    deco_boxes: Vec<LineDecoration>,
    win_ascent_ratios: WinAscentCache,
    ink: ink::InkCache,
    upright: upright::UprightCache,
    upright_x: Vec<f32>,
    sampled: sampled::SampledCache,
    sampled_paths: Vec<Arc<BezPath>>,
}

#[derive(Clone, Copy)]
pub(crate) struct TextPaintOptions<'a> {
    pub scale: f64,
    pub brush_override: Option<Color>,
    pub ellipsis: Option<LineEllipsis>,
    pub text: Option<&'a str>,
    pub group: Option<NodeId>,
    pub flat_ancestors: bool,
}

/// Where `text-overflow: ellipsis` and `line-clamp` cut lines short with an ellipsis.
#[derive(Clone, Copy)]
pub(crate) struct LineEllipsis {
    /// The inline-axis edge (in layout units) that text must not paint past.
    pub width: f32,
    /// Whether lines that overflow `width` are cut (`text-overflow: ellipsis`).
    pub on_overflow: bool,
    /// A line that always ends in an ellipsis (the last line kept by `line-clamp`).
    pub on_line: Option<usize>,
    /// Whether lines end on the left (`direction: rtl`), so they are cut from the left edge.
    pub rtl: bool,
}

/// How one line is cut short: glyphs outside `min_x..max_x` are dropped and the ellipsis
/// (`count` copies of glyph `id`, `advance` apart from `x`) is drawn in the run at
/// `run_index`. What remains of the line spans `start..end`.
#[derive(Clone, Copy)]
struct LineCut {
    run_index: usize,
    min_x: f32,
    max_x: f32,
    start: f32,
    end: f32,
    id: u32,
    x: f32,
    y: f32,
    advance: f32,
    count: usize,
}

impl LineCut {
    fn keeps(self, glyph: &parley::layout::Glyph) -> bool {
        glyph.x >= self.min_x && glyph.x + glyph.advance <= self.max_x
    }

    fn glyphs(self) -> impl Iterator<Item = anyrender::Glyph> + Clone {
        (0..self.count).map(move |index| anyrender::Glyph {
            id: self.id,
            x: self.x + self.advance * index as f32,
            y: self.y,
        })
    }
}

fn line_cut(
    line: &Line<'_, TextBrush>,
    ellipsis: LineEllipsis,
    is_clamped: bool,
) -> Option<LineCut> {
    let width = ellipsis.width;
    let mut runs = line
        .items()
        .enumerate()
        .filter_map(|(index, item)| match item {
            PositionedLayoutItem::GlyphRun(run) => Some((index, run)),
            PositionedLayoutItem::InlineBox(_) => None,
        });
    let (run_index, run) = if ellipsis.rtl {
        runs.find(|(_, run)| run.offset() + run.advance() > 0.0)?
    } else {
        runs.take_while(|(_, run)| run.offset() < width).last()?
    };
    let (id, advance, count) = ellipsis_glyph(run.run())?;
    let length = advance * count as f32;
    let metrics = line.metrics();
    let origin = metrics.offset + metrics.inline_min_coord;
    let (text_start, text_end) = if ellipsis.rtl {
        (origin + metrics.hanging_advance, origin + metrics.advance)
    } else {
        (origin, origin + metrics.advance - metrics.hanging_advance)
    };
    let line_start = metrics.inline_min_coord.max(0.0);
    let line_end = metrics.inline_max_coord.min(width);
    let (line_start, line_end) = if line_end > line_start {
        (line_start, line_end)
    } else {
        (0.0, width)
    };
    let fits = is_clamped && text_end - text_start + length <= line_end - line_start;
    let (min_x, max_x) = if fits {
        (f32::NEG_INFINITY, f32::INFINITY)
    } else if ellipsis.rtl {
        let shift = (metrics.inline_max_coord - text_end - metrics.indent).max(0.0);
        (line_start + length - shift, f32::INFINITY)
    } else {
        let shift = (metrics.offset - metrics.indent).max(0.0);
        (f32::NEG_INFINITY, line_end - length + shift)
    };
    let (mut kept_start, mut kept_end) = (f32::INFINITY, f32::NEG_INFINITY);
    for item in line.items() {
        if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
            for glyph in glyph_run.positioned_glyphs() {
                if glyph.x >= min_x && glyph.x + glyph.advance <= max_x {
                    kept_start = kept_start.min(glyph.x);
                    kept_end = kept_end.max(glyph.x + glyph.advance);
                }
            }
        }
    }
    let (start, end, x) = if ellipsis.rtl {
        let kept_start = kept_start.max(text_start).min(origin + metrics.advance);
        (kept_start - length, f32::INFINITY, kept_start - length)
    } else {
        let kept_end = kept_end.min(text_end).max(origin);
        (f32::NEG_INFINITY, kept_end + length, kept_end)
    };
    Some(LineCut {
        run_index,
        min_x,
        max_x,
        start,
        end,
        id,
        x,
        y: run.baseline(),
        advance,
        count,
    })
}

/// The glyph used to draw an ellipsis in a run's font, its advance and how many times it
/// is repeated: U+2026 HORIZONTAL ELLIPSIS once, or three full stops when the font lacks it.
fn ellipsis_glyph(run: &parley::Run<'_, TextBrush>) -> Option<(u32, f32, usize)> {
    use skrifa::MetadataProvider as _;
    use skrifa::instance::{LocationRef, Size};
    use skrifa::raw::FontRef;
    use skrifa::raw::types::F2Dot14;

    let font = run.font();
    let font_ref = FontRef::from_index(font.data.as_ref(), font.index).ok()?;
    let charmap = font_ref.charmap();
    let (id, count) = match charmap.map('\u{2026}') {
        Some(id) => (id, 1),
        None => (charmap.map('.')?, 3),
    };
    let normalized_coords: &[i16] = bytemuck::cast_slice(run.normalized_coords());
    let coords: Vec<F2Dot14> = normalized_coords
        .iter()
        .map(|&coord| F2Dot14::from_bits(coord))
        .collect();
    let advance = font_ref
        .glyph_metrics(Size::new(run.font_size()), LocationRef::new(&coords))
        .advance_width(id)?;
    Some((id.to_u32(), advance, count))
}

const MAX_INK_OUTSET: f64 = 13.0;

const UPRIGHT_GLYPH: Affine = Affine::new([0.0, 1.0, -1.0, 0.0, 0.0, 0.0]);

struct LineInk<'a, 'l> {
    line: &'a Line<'l, TextBrush>,
    cut: Option<LineCut>,
    cache: &'a mut ink::InkCache,
    holes: SmallVec<[(f64, f64); 2]>,
}

impl LineInk<'_, '_> {
    fn skips(
        &mut self,
        decoration: &LineDecoration,
        transform: Affine,
        top: f64,
        thickness: f64,
    ) -> Vec<(f64, f64)> {
        let mut skips = self.holes.to_vec();
        if !decoration.deco.skips_ink {
            return skips;
        }
        let origin_x = transform.translation().x;
        let thickness = thickness.max(1.0);
        let band_top = top + 0.5;
        let band_bottom = (top + thickness - 0.5).max(band_top);
        let outset = thickness.min(MAX_INK_OUTSET);
        for (index, item) in self.line.items().enumerate() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };
            if !decoration.runs.contains(&index) {
                continue;
            }
            let run = glyph_run.run();
            let ink_run = ink::InkRun::new(
                run.font(),
                run.font_size(),
                bytemuck::cast_slice(run.normalized_coords()),
            );
            for glyph in glyph_run.positioned_glyphs() {
                if self.cut.is_some_and(|cut| !cut.keeps(&glyph)) {
                    continue;
                }
                let y = f64::from(glyph.y);
                let (Some(low), Some(high)) = ((y - band_bottom).to_f32(), (y - band_top).to_f32())
                else {
                    continue;
                };
                if let Some((left, right)) = self.cache.extent(&ink_run, glyph.id, low, high) {
                    let x = f64::from(glyph.x) + origin_x;
                    skips.push((
                        (x + f64::from(left) - outset).round() - origin_x,
                        (x + f64::from(right) + outset).round() - origin_x,
                    ));
                }
            }
        }
        skips.sort_by(|a, b| a.0.total_cmp(&b.0));
        skips
    }
}

pub(crate) fn is_invisible(doc: &BaseDocument, id: NodeId) -> bool {
    doc.get_node(id)
        .and_then(|node| node.primary_styles())
        .is_some_and(|style| style.get_inherited_box().visibility != Visibility::Visible)
}

struct RunFace<'a> {
    #[cfg(target_os = "windows")]
    installed: Option<&'a BaseDocument>,
    font: &'a parley::FontData,
    font_size: f32,
    coords: &'a [i16],
    embolden: Vec2,
    glyph_xform: Option<Affine>,
}

fn fill_glyphs<I: Iterator<Item = anyrender::Glyph> + Clone>(
    scene: &mut impl PaintScene,
    sampled: &mut sampled::SampledCache,
    sampled_paths: &mut Vec<Arc<BezPath>>,
    face: &RunFace<'_>,
    (transform, color): (Affine, Color),
    glyphs: impl Fn() -> I,
) {
    #[cfg(target_os = "windows")]
    if windows::draw(scene, face, transform, color, glyphs()) {
        return;
    }
    let frame = (!FONT_EMBOLDEN_ENABLED && face.glyph_xform.is_none())
        .then(|| sampled::SampledFrame::new(transform))
        .flatten()
        .filter(|&frame| {
            frame.ppem(face.font_size).is_some_and(|ppem| {
                sampled.outlines(
                    frame,
                    &upright::SizedFace::new(face.font, ppem, face.coords),
                    glyphs().map(|glyph| glyph.id),
                    sampled_paths,
                )
            })
        });
    let Some(frame) = frame else {
        scene.draw_glyphs(
            face.font,
            face.font_size,
            !FONT_EMBOLDEN_ENABLED,
            face.coords,
            face.embolden,
            Fill::NonZero,
            &anyrender::Paint::from(color),
            1.0,
            transform,
            face.glyph_xform,
            quarter::snap(transform, glyphs()),
        );
        return;
    };
    for (glyph, path) in glyphs().zip(sampled_paths.iter()) {
        scene.fill(
            Fill::NonZero,
            frame.place(glyph.x, glyph.y),
            color,
            None,
            path.as_ref(),
        );
    }
}
