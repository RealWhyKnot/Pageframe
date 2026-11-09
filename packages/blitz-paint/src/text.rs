use anyrender::PaintScene;
use blitz_dom::CopyStyleGetters;
use blitz_dom::{BaseDocument, NodeId, node::TextBrush, util::ToColorColor};
use kurbo::{Affine, BezPath, Cap, Circle, Rect, Stroke};
use parley::{Affinity, Cursor, Layout, Line, PositionedLayoutItem, Selection};
use peniko::Fill;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use style::properties::generated::longhands::text_decoration_style::computed_value::T as TextDecorationStyle;
use style::values::computed::{
    Length, LengthPercentage, TextDecorationLength, TextDecorationLine, TextUnderlinePosition,
};
use style::values::generics::text::{GenericTextDecorationInset, GenericTextDecorationLength};

use crate::color::{Color, ToColorColor as _};
use crate::{FONT_EMBOLDEN_ENABLED, SELECTION_COLOR};

/// Draw the backgrounds of inline elements (e.g. `<span style="background: ...">`).
///
/// Each glyph run carries the node id of the innermost inline element it belongs to
/// (via its brush). We look up that node's `background-color` and, if non-transparent,
/// fill a rectangle covering the run's advance and its font's ascent/descent so that the
/// background sits behind the text.
///
/// The inline root's own background is painted separately (as a normal block box), so
/// runs belonging to the root are skipped to avoid drawing it twice.
pub(crate) fn draw_inline_backgrounds<'a>(
    scene: &mut impl PaintScene,
    lines: impl Iterator<Item = Line<'a, TextBrush>>,
    doc: &BaseDocument,
    transform: Affine,
    inline_root_id: NodeId,
) {
    for line in lines {
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };

            let node_id = glyph_run.style().brush.id;
            if node_id == inline_root_id {
                continue;
            }

            let Some(styles) = doc.get_node(node_id).and_then(|node| node.primary_styles()) else {
                continue;
            };

            let current_color = styles.clone_color();
            let bg_color = styles
                .get_background()
                .background_color
                .resolve_to_absolute(&current_color)
                .as_srgb_color();
            if bg_color == Color::TRANSPARENT {
                continue;
            }

            let metrics = glyph_run.run().font_metrics();
            let x = glyph_run.offset() as f64;
            let w = glyph_run.advance() as f64;
            let baseline = glyph_run.baseline() as f64;
            let y0 = baseline - metrics.ascent as f64;
            let y1 = baseline + metrics.descent as f64;
            let rect = Rect::new(x, y0, x + w, y1);

            scene.fill(Fill::NonZero, transform, bg_color, None, &rect);
        }
    }
}

/// Per-font-face cache of the OS/2 `usWinAscent / unitsPerEm` ratio, keyed by the font
/// blob's unique id and face index. `None` is cached for fonts without a usable OS/2 or
/// head table so they aren't re-parsed either.
type WinAscentCache = HashMap<(u64, u32), Option<f32>>;

/// The font's OS/2 `usWinAscent`, in the same (device) pixels as `font_size`.
///
/// This is the ascent browsers use as the top of the "em box" when positioning overlines. It
/// is typically taller than the hhea ascent Parley exposes via [`parley::layout::run::RunMetrics`].
/// Returns `None` when the font has no OS/2 table.
fn win_ascent(cache: &mut WinAscentCache, font: &parley::FontData, font_size: f32) -> Option<f32> {
    let ratio = *cache
        .entry((font.data.id(), font.index))
        .or_insert_with(|| win_ascent_ratio(font));
    Some(ratio? * font_size)
}

/// The unitless `usWinAscent / unitsPerEm` ratio for a font face, or `None` when the font
/// has no usable head/OS/2 table.
fn win_ascent_ratio(font: &parley::FontData) -> Option<f32> {
    use skrifa::raw::{FontRef, TableProvider as _};
    let font_ref = FontRef::from_index(font.data.as_ref(), font.index).ok()?;
    let units_per_em = font_ref.head().ok()?.units_per_em();
    let win_ascent = font_ref.os2().ok()?.us_win_ascent();
    Some(win_ascent as f32 / units_per_em as f32)
}

/// Mirrors Blink's `SelectBestDashGap`: choose the gap length (as close as possible to
/// `gap_length`) that fits a whole number of `dash_length` dashes across `stroke_length`,
/// so dashes are evenly distributed and a dash lands at each end of the line.
fn select_best_dash_gap(stroke_length: f64, dash_length: f64, gap_length: f64) -> f64 {
    let available_length = stroke_length + gap_length;
    let min_num_dashes = (available_length / (dash_length + gap_length)).floor();
    let max_num_dashes = min_num_dashes + 1.0;
    let min_num_gaps = min_num_dashes - 1.0;
    let max_num_gaps = max_num_dashes - 1.0;
    if min_num_gaps < 1.0 {
        return gap_length;
    }
    let min_gap = (stroke_length - min_num_dashes * dash_length) / min_num_gaps;
    let max_gap = (stroke_length - max_num_dashes * dash_length) / max_num_gaps;
    if max_gap <= 0.0 || (min_gap - gap_length).abs() < (max_gap - gap_length).abs() {
        min_gap
    } else {
        max_gap
    }
}

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

    DecorationStackEntry {
        node_id,
        fill_color,
        decoration,
        shadows,
    }
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
}

/// Reusable scratch storage for drawing text across all inline formatting contexts in a
/// document. The vectors are cleared between uses without releasing their allocations.
#[derive(Default)]
pub(crate) struct DrawTextContext {
    stack: Vec<DecorationStackEntry>,
    path_scratch: Vec<NodeId>,
    deco_boxes: Vec<LineDecoration>,
    win_ascent_ratios: WinAscentCache,
}

/// Resolve the CSS `text-decoration-thickness` to a device-pixel size.
///
/// - Percentages are resolved against the font size,
/// - `from-font` uses the metrics from Parley,
/// - `auto` uses a thickness of `font-size / 10` (minimum: 1px).
///
/// The result is floored to a whole number of device pixels (minimum 1), matching
/// browsers, which snap decoration thickness to whole pixels.
fn decoration_size(
    thickness: &TextDecorationLength,
    metric_size: f32,
    css_font_size: f64,
    scale: f64,
) -> f32 {
    (match thickness {
        GenericTextDecorationLength::LengthPercentage(lp) => {
            lp.resolve(Length::new(css_font_size as f32)).px() as f64 * scale
        }
        GenericTextDecorationLength::FromFont => metric_size as f64,
        GenericTextDecorationLength::Auto => (css_font_size / 10.0).max(1.0) * scale,
    })
    .floor()
    .max(1.0) as f32
}

/// Draws a single decoration line of the given `text-decoration-style` spanning
/// `[x0, x0 + width]`. `baseline` is the line's baseline and `offset` positions the line
/// relative to it (`y = baseline - offset + size / 2`, growing downward). Geometry is built
/// explicitly (rather than relying on backend dash support) so it renders consistently
/// across renderers.
///
/// `double_dir` points away from the text (`+1` = down, `-1` = up) and is used to place the
/// second line of a `double` decoration.
#[allow(clippy::too_many_arguments)]
fn draw_decoration_line(
    scene: &mut impl PaintScene,
    transform: Affine,
    scale: f64,
    x0: f64,
    width: f64,
    baseline: f32,
    offset: f32,
    size: f32,
    brush: &anyrender::Paint,
    double_dir: f64,
    deco_style: TextDecorationStyle,
    inset_start: f64,
    inset_end: f64,
) {
    // Inset the line from the box's start/end edges (assumes LTR: start is the left edge).
    // Negative insets extend the line past the text.
    let x = x0 + inset_start;
    let w = width - inset_start - inset_end;
    if w <= 0.0 {
        return;
    }
    let y = (baseline - offset + size / 2.0) as f64;
    let size = size as f64;
    let butt_stroke = Stroke::new(size).with_caps(Cap::Butt);

    match deco_style {
        TextDecorationStyle::None => {
            // no line. Equivalent to `text-decoration-line: none`
        }
        // `solid`
        TextDecorationStyle::Solid => {
            let line = kurbo::Line::new((x, y), (x + w, y));
            scene.stroke(&butt_stroke, transform, brush, None, &line);
        }
        // Two lines, each `size` thick, separated by a 1px (CSS) gap. This
        // matches Chrome, where the second line is offset by `thickness + 1px`
        // and sits further from the text (below for underline, above for
        // overline).
        TextDecorationStyle::Double => {
            let one_css_px = scale;
            for cy in [y, y + double_dir * (size + one_css_px)] {
                let line = kurbo::Line::new((x, cy), (x + w, cy));
                scene.stroke(&butt_stroke, transform, brush, None, &line);
            }
        }
        // Round dots (diameter = line thickness) spaced one dot apart.
        TextDecorationStyle::Dotted => {
            let radius = size / 2.0;
            let step = 2.0 * size;
            let mut cx = x + radius;
            while cx <= x + w - radius {
                let dot = Circle::new((cx, y), radius);
                scene.fill(Fill::NonZero, transform, brush, None, &dot);
                cx += step;
            }
        }
        // Dashes as filled rectangles. Dash/gap lengths are relative to the
        // thickness (matching Blink): thinner lines use proportionally longer
        // dashes and gaps so they don't read as dotted or solid.
        TextDecorationStyle::Dashed => {
            let (dash_ratio, gap_ratio) = if size >= 3.0 { (2.0, 1.0) } else { (3.0, 2.0) };
            let dash = size * dash_ratio;
            let nominal_gap = size * gap_ratio;
            // Nudge the gap so a whole number of dashes fits the run evenly.
            let gap = if w > 2.0 * dash {
                select_best_dash_gap(w, dash, nominal_gap)
            } else {
                nominal_gap
            };
            let mut dx = x;
            while dx < x + w {
                let end = (dx + dash).min(x + w);
                let rect = Rect::new(dx, y - size / 2.0, end, y + size / 2.0);
                scene.fill(Fill::NonZero, transform, brush, None, &rect);
                dx += dash + gap;
            }
        }
        // A squiggle built from alternating quadratic Béziers.
        TextDecorationStyle::Wavy => {
            let amplitude = size;
            let half_wave = (2.0 * size).max(1.0);
            let mut path = BezPath::new();
            path.move_to((x, y));
            let mut px = x;
            let mut up = true;
            while px < x + w {
                let nx = (px + half_wave).min(x + w);
                // A quad Bézier reaches half the control-point offset at its
                // midpoint, so use `2 * amplitude` to hit the target peak.
                let ctrl_y = if up {
                    y - 2.0 * amplitude
                } else {
                    y + 2.0 * amplitude
                };
                path.quad_to(((px + nx) / 2.0, ctrl_y), (nx, y));
                px = nx;
                up = !up;
            }
            let stroke = Stroke::new(size).with_caps(Cap::Round);
            scene.stroke(&stroke, transform, brush, None, &path);
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct TextPaintOptions {
    pub scale: f64,
    pub brush_override: Option<Color>,
    pub ellipsis: Option<LineEllipsis>,
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

fn line_cut(line: &Line<'_, TextBrush>, ellipsis: LineEllipsis) -> Option<LineCut> {
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
    let (min_x, max_x) = if ellipsis.rtl {
        (length, f32::INFINITY)
    } else {
        (f32::NEG_INFINITY, width - length)
    };
    let metrics = line.metrics();
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
        let kept_start = kept_start.min(metrics.offset + metrics.advance);
        (kept_start - length, f32::INFINITY, kept_start - length)
    } else {
        let kept_end = kept_end.max(metrics.offset);
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

/// Paint the decorations accumulated for one line, one line per decorating box.
fn flush_line_decorations(
    scene: &mut impl PaintScene,
    transform: Affine,
    options: TextPaintOptions,
    deco_boxes: &[LineDecoration],
    win_ascent_ratios: &mut WinAscentCache,
    inline_root_id: NodeId,
    line_baseline: f32,
) {
    let TextPaintOptions {
        scale,
        brush_override,
        ..
    } = options;
    // Draw innermost boxes first so ancestors' decorations paint on top, matching the
    // per-run drawing order this replaced (`stack.iter().rev()`).
    for acc in deco_boxes.iter().rev() {
        let deco = &acc.deco;
        // Prefer the decorating box's own font; fall back to the first run it covers.
        let geom = match (&acc.own, &acc.first) {
            (Some(own), _) => own.clone(),
            (None, Some(first)) => {
                let mut geom = first.clone();
                // A descendant run's baseline may be shifted by `vertical-align`, but the
                // decoration is positioned from the decorating box's own baseline. For the
                // inline root that is the line's baseline.
                if acc.node_id == inline_root_id {
                    geom.baseline = line_baseline;
                }
                geom
            }
            (None, None) => continue,
        };
        let geom = &geom;
        let width = acc.max_x - acc.min_x;
        if width <= 0.0 {
            continue;
        }
        let brush = anyrender::Paint::from(brush_override.unwrap_or(deco.color));

        // `text-decoration-inset` shortens (or, when negative, extends) the line from the
        // inline-start/end edges. Percentages resolve against the decoration line length
        // (the box's total advance on this line); `auto` is no inset.
        let (inset_start, inset_end) = match &deco.inset {
            GenericTextDecorationInset::LengthPercentage { start, end } => {
                // The extent is in device pixels; convert to CSS pixels so the resolved
                // result can be scaled back.
                let line_length = Length::new((width / scale) as f32);
                (
                    start.resolve(line_length).px() as f64 * scale,
                    end.resolve(line_length).px() as f64 * scale,
                )
            }
            GenericTextDecorationInset::Auto => (0.0, 0.0),
        };

        if deco.line.contains(TextDecorationLine::UNDERLINE) {
            let size = decoration_size(
                &deco.thickness,
                geom.underline_size,
                geom.css_font_size,
                scale,
            );

            // `text-underline-offset` moves the underline away from the text. `auto` keeps
            // the font's suggested position; otherwise it resolves against the font size
            // (percentages relative to 1em).
            let extra_underline_offset = deco
                .underline_offset
                .as_ref()
                .map(|lp| lp.resolve(Length::new(geom.css_font_size as f32)).px() as f64 * scale)
                .unwrap_or(0.0);

            // `text-underline-position: under` places the underline below the glyph box
            // (below descenders) rather than at the font's suggested position near the
            // alphabetic baseline. We anchor the top of the line at the descent so it clears
            // descending glyphs like "gqy".
            // An explicit offset under `auto` is taken from the alphabetic
            // baseline, as CSS and Chrome have it, not from the auto gap.
            let base_offset = if deco.underline_under {
                -geom.descent
            } else if deco.underline_offset.is_some() && !deco.underline_from_font {
                0.0
            } else {
                geom.underline_offset
            };
            let offset = base_offset - extra_underline_offset as f32;

            // A `double` underline extends downward, away from the text.
            draw_decoration_line(
                scene,
                transform,
                scale,
                acc.min_x,
                width,
                geom.baseline,
                offset,
                size,
                &brush,
                1.0,
                deco.style,
                inset_start,
                inset_end,
            );
        }
        if deco.line.contains(TextDecorationLine::OVERLINE) {
            // Fonts don't provide a dedicated overline metric, so reuse the underline
            // thickness. The line sits at the top of the "em box": its lower edge rests on
            // the ascent so it clears the glyphs, and it extends upward from there.
            //
            // Browsers use the OS/2 `usWinAscent` for this edge, which is taller than the
            // hhea-based ascent Parley reports; using the smaller value would draw the
            // overline too low. Fall back to Parley's ascent for fonts without a usable
            // OS/2 table.
            let size = decoration_size(
                &deco.thickness,
                geom.underline_size,
                geom.css_font_size,
                scale,
            );
            let ascent =
                win_ascent(win_ascent_ratios, &geom.font, geom.font_size).unwrap_or(geom.ascent);
            let offset = ascent + size;

            // A `double` overline extends upward, away from the text.
            draw_decoration_line(
                scene,
                transform,
                scale,
                acc.min_x,
                width,
                geom.baseline,
                offset,
                size,
                &brush,
                -1.0,
                deco.style,
                inset_start,
                inset_end,
            );
        }
        if deco.line.contains(TextDecorationLine::LINE_THROUGH) {
            let size = decoration_size(
                &deco.thickness,
                geom.strikethrough_size,
                geom.css_font_size,
                scale,
            );

            // Centre the line-through a third of the ascent above the baseline, matching
            // Chrome (which places it `2/3 * ascent` below the text-top). Parley's
            // `strikethrough_offset` (the font's `yStrikeoutPosition`) sits lower and would
            // draw the line too close to the baseline.
            let ascent =
                win_ascent(win_ascent_ratios, &geom.font, geom.font_size).unwrap_or(geom.ascent);
            let offset = ascent / 3.0 + size / 2.0;

            draw_decoration_line(
                scene,
                transform,
                scale,
                acc.min_x,
                width,
                geom.baseline,
                offset,
                size,
                &brush,
                1.0,
                deco.style,
                inset_start,
                inset_end,
            );
        }
    }
}

pub(crate) fn stroke_text<'a>(
    scene: &mut impl PaintScene,
    lines: impl Iterator<Item = Line<'a, TextBrush>>,
    doc: &BaseDocument,
    transform: Affine,
    options: TextPaintOptions,
    inline_root_id: NodeId,
    context: &mut DrawTextContext,
) {
    let TextPaintOptions {
        scale,
        brush_override,
        ellipsis,
    } = options;
    let DrawTextContext {
        stack,
        path_scratch,
        deco_boxes,
        win_ascent_ratios,
    } = context;
    stack.clear();
    path_scratch.clear();
    deco_boxes.clear();

    // Persistent stack mirroring the ancestor path (inline root -> current run's
    // node) as we walk the runs. The `text-decoration-*` properties are *not*
    // inherited; instead a decoration set on an ancestor is propagated to the
    // descendant text it wraps. Rather than re-resolving styles for the whole
    // ancestor chain on every run, we cache each node's resolved values here and,
    // for each run, only resolve styles for the nodes newly descended into (popping
    // as we ascend). `path_scratch` is a reusable buffer for the run's node path.
    for (line_index, line) in lines.enumerate() {
        let cut = ellipsis.and_then(|ellipsis| {
            let metrics = line.metrics();
            let overflows = metrics.offset < 0.0
                || metrics.offset + metrics.advance - metrics.hanging_advance > ellipsis.width;
            ((ellipsis.on_overflow && overflows) || ellipsis.on_line == Some(line_index))
                .then(|| line_cut(&line, ellipsis))
                .flatten()
        });

        // Decorations accumulated for this line, keyed by decorating box, so each box is
        // painted once (spanning all its runs) using its own font — matching Firefox, which
        // draws one decoration per box rather than one stepped segment per differently-sized
        // run. Clearing preserves the allocation for the next line and inline context.
        deco_boxes.clear();

        for (item_index, item) in line.items().enumerate() {
            if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                let run = glyph_run.run();
                let font = run.font();
                let font_size = run.font_size();
                let metrics = run.font_metrics();
                let style = glyph_run.style();
                let synthesis = run.synthesis();
                let glyph_xform = synthesis
                    .skew()
                    .map(|angle| Affine::skew(angle.to_radians().tan() as f64, 0.0));

                let css_font_size = font_size as f64 / scale;

                // Reconcile the stack with this run's ancestor path. Build the path
                // (inline root -> run node), keep the shared prefix already on the stack,
                // pop the rest, and resolve styles only for the newly-descended nodes.
                path_scratch.clear();
                let mut walk_id = Some(style.brush.id);
                while let Some(node_id) = walk_id {
                    path_scratch.push(node_id);
                    if node_id == inline_root_id {
                        break;
                    }
                    walk_id = doc.get_node(node_id).and_then(|node| node.parent);
                }
                path_scratch.reverse();

                let shared = stack
                    .iter()
                    .zip(path_scratch.iter())
                    .take_while(|(entry, node_id)| entry.node_id == **node_id)
                    .count();
                stack.truncate(shared);
                for &node_id in &path_scratch[shared..] {
                    stack.push(resolve_decoration_entry(doc, node_id));
                }

                let fill_color = brush_override
                    .unwrap_or_else(|| stack.last().map(|e| e.fill_color).unwrap_or(Color::BLACK));

                let embolden = if FONT_EMBOLDEN_ENABLED {
                    font_embolden(font_size as f64, fill_color, transform)
                } else {
                    kurbo::Vec2::default()
                };

                let normalized_coords: &[i16] = bytemuck::cast_slice(run.normalized_coords());
                let glyphs = || {
                    glyph_run
                        .positioned_glyphs()
                        .filter(move |glyph| cut.is_none_or(|cut| cut.keeps(glyph)))
                        .map(|glyph| anyrender::Glyph {
                            id: glyph.id as _,
                            x: glyph.x,
                            y: glyph.y,
                        })
                        .chain(
                            cut.filter(|cut| cut.run_index == item_index)
                                .into_iter()
                                .flat_map(LineCut::glyphs),
                        )
                };

                // Shadows are painted beneath the text, with the first shadow on top.
                // Text painted with a brush override (e.g. as a mask) has no shadow.
                let shadows = match (brush_override, stack.last()) {
                    (None, Some(entry)) if font_size > 0.0 => entry.shadows.as_slice(),
                    _ => &[],
                };
                for shadow in shadows
                    .iter()
                    .rev()
                    .filter(|shadow| shadow.color.components[3] > 0.0)
                {
                    let shadow_transform = transform * Affine::translate(shadow.offset * scale);
                    // A shadow's blur radius is twice the standard deviation of its Gaussian.
                    let std_deviation = shadow.blur * scale / 2.0;
                    let run_x0 = glyph_run.offset() as f64;
                    let baseline = glyph_run.baseline() as f64;
                    let ink_margin = font_size as f64;
                    let bounds = Rect::new(
                        run_x0 - ink_margin,
                        baseline - metrics.ascent.abs() as f64 - ink_margin,
                        run_x0 + glyph_run.advance() as f64 + ink_margin,
                        baseline + metrics.descent.abs() as f64 + ink_margin,
                    );
                    let blurred = (std_deviation > 0.0)
                        .then(|| {
                            blurred_shadow_image(
                                bounds,
                                std_deviation,
                                shadow.color,
                                |hasher, origin| {
                                    font.data.id().hash(hasher);
                                    font.index.hash(hasher);
                                    font_size.to_bits().hash(hasher);
                                    normalized_coords.hash(hasher);
                                    [embolden.x, embolden.y].map(f64::to_bits).hash(hasher);
                                    glyph_xform
                                        .map(|xform| xform.as_coeffs().map(f64::to_bits))
                                        .hash(hasher);
                                    for glyph in glyphs() {
                                        glyph.id.hash(hasher);
                                        (f64::from(glyph.x) - origin.x).to_bits().hash(hasher);
                                        (f64::from(glyph.y) - origin.y).to_bits().hash(hasher);
                                    }
                                },
                                |scene, offset| {
                                    scene.draw_glyphs(
                                        font,
                                        font_size,
                                        false,
                                        normalized_coords,
                                        embolden,
                                        Fill::NonZero,
                                        &anyrender::Paint::from(Color::BLACK),
                                        1.0,
                                        offset,
                                        glyph_xform,
                                        glyphs(),
                                    );
                                },
                            )
                        })
                        .flatten();
                    match blurred {
                        Some((image, origin)) => {
                            scene.draw_image(
                                image.as_ref(),
                                shadow_transform * Affine::translate(origin),
                            );
                        }
                        None => scene.draw_glyphs(
                            font,
                            font_size,
                            !FONT_EMBOLDEN_ENABLED,
                            normalized_coords,
                            embolden,
                            Fill::NonZero,
                            &anyrender::Paint::from(shadow.color),
                            1.0,
                            shadow_transform,
                            glyph_xform,
                            glyphs(),
                        ),
                    }
                }

                // Zero-size text is invisible, but hinting can give its glyphs ink.
                if font_size > 0.0 {
                    scene.draw_glyphs(
                        font,
                        font_size,
                        !FONT_EMBOLDEN_ENABLED, // hint
                        normalized_coords,
                        embolden,
                        Fill::NonZero,
                        &anyrender::Paint::from(fill_color),
                        1.0, // alpha
                        transform,
                        glyph_xform,
                        glyphs(),
                    );
                }

                // Accumulate this run's contribution to each decorating box on its ancestor
                // path. The decoration is drawn once per box after the whole line has been
                // walked (see `flush_line_decorations`), so mixed font sizes within a box
                // produce a single straight line rather than one stepped segment per run.
                let geometry = DecorationRunGeometry {
                    baseline: glyph_run.baseline(),
                    ascent: metrics.ascent,
                    descent: metrics.descent,
                    underline_offset: metrics.underline_offset,
                    underline_size: metrics.underline_size,
                    strikethrough_size: metrics.strikethrough_size,
                    font: font.clone(),
                    font_size,
                    css_font_size,
                };
                let run_node_id = style.brush.id;
                let mut run_x0 = glyph_run.offset() as f64;
                let mut run_x1 = run_x0 + glyph_run.advance() as f64;
                if let Some(cut) = cut {
                    run_x0 = run_x0.max(cut.start as f64).min(cut.end as f64);
                    run_x1 = run_x1.max(cut.start as f64).min(cut.end as f64);
                }

                for entry in stack.iter() {
                    if entry.decoration.is_none() {
                        continue;
                    }
                    let idx = match deco_boxes.iter().position(|d| d.node_id == entry.node_id) {
                        Some(idx) => idx,
                        None => {
                            deco_boxes.push(LineDecoration {
                                node_id: entry.node_id,
                                deco: entry.decoration.clone().unwrap(),
                                min_x: f64::INFINITY,
                                max_x: f64::NEG_INFINITY,
                                own: None,
                                first: None,
                            });
                            deco_boxes.len() - 1
                        }
                    };
                    let acc = &mut deco_boxes[idx];
                    acc.min_x = acc.min_x.min(run_x0);
                    acc.max_x = acc.max_x.max(run_x1);
                    if acc.first.is_none() {
                        acc.first = Some(geometry.clone());
                    }
                    // A run whose innermost node is the box itself is the box's own text, so
                    // its font is the one Firefox positions and sizes the decoration with.
                    if entry.node_id == run_node_id {
                        acc.own = Some(geometry.clone());
                    }
                }
            }
        }

        flush_line_decorations(
            scene,
            transform,
            options,
            deco_boxes,
            win_ascent_ratios,
            inline_root_id,
            line.metrics().baseline,
        );
    }
}

/// Draw selection highlight rectangles for the given byte range in a layout.
/// Uses Parley's Selection type for accurate geometry calculation. Lines from
/// `visible_lines` on (those hidden by `line-clamp`) are not highlighted.
pub(crate) fn draw_text_selection(
    scene: &mut impl PaintScene,
    layout: &Layout<TextBrush>,
    transform: Affine,
    selection_start: usize,
    selection_end: usize,
    visible_lines: usize,
) {
    let anchor = Cursor::from_byte_index(layout, selection_start, Affinity::Downstream);
    let focus = Cursor::from_byte_index(layout, selection_end, Affinity::Downstream);
    let selection = Selection::new(anchor, focus);

    selection.geometry_with(layout, |rect, line_idx| {
        if line_idx >= visible_lines {
            return;
        }
        let rect = kurbo::Rect::new(rect.x0, rect.y0, rect.x1, rect.y1);
        scene.fill(Fill::NonZero, transform, SELECTION_COLOR, None, &rect);
    });
}

/// Approximates macOS font smoothing (stem dilation). CoreGraphics dilates each side by an amount
/// in device pixels that grows with the device ppem up to 0.3px (fitted to Safari's ink at 9-72px
/// and 1x-3x scale), and keeps dilating at large sizes, so there is no ppem cutoff. glifo applies
/// the embolden in user units, so the device amounts are divided by the transform's scale.
fn font_embolden(font_size: f64, color: Color, transform: Affine) -> kurbo::Vec2 {
    const MAX_DEVICE_EMBOLDEN: f64 = 0.3;
    let [a, b, c, d, _, _] = transform.as_coeffs();
    let (scale_x, scale_y) = (a.hypot(b), c.hypot(d));
    let device_ppem = font_size * (a * d - b * c).abs().sqrt();
    if scale_x == 0.0 || scale_y == 0.0 || !device_ppem.is_finite() {
        return kurbo::Vec2::default();
    }
    let fs = device_ppem * embolden_scale(color);
    kurbo::Vec2::new(
        (0.0121 * fs).min(MAX_DEVICE_EMBOLDEN) / scale_x,
        (0.015125 * fs).min(MAX_DEVICE_EMBOLDEN) / scale_y,
    )
}

/// Scales Pathfinder's stem-darkening factors by the text's relative luminance. CoreText
/// dilates light text more than dark text; the points are fitted to Safari's ink for text
/// colours from black to white on black and white backgrounds.
fn embolden_scale(color: Color) -> f64 {
    const POINTS: [(f64, f64); 4] = [(0.1, 0.5), (0.125, 0.72), (0.35, 0.72), (1.0, 1.3)];
    let lum = color.discard_alpha().relative_luminance() as f64;
    let mut prev = POINTS[0];
    if lum <= prev.0 {
        return prev.1;
    }
    for point in &POINTS[1..] {
        if lum <= point.0 {
            return prev.1 + (point.1 - prev.1) * (lum - prev.0) / (point.0 - prev.0);
        }
        prev = *point;
    }
    prev.1
}

/// The largest side, in pixels, of the image a blurred shadow is rendered into. Larger
/// shadows are drawn without blur.
const MAX_SHADOW_IMAGE_SIDE: f64 = 4096.0;

/// How many bytes of blurred shadow images each thread keeps for reuse.
const SHADOW_CACHE_BYTES: usize = 32 << 20;

#[derive(Default)]
struct ShadowCache {
    entries: HashMap<u64, (peniko::ImageBrush, u64)>,
    clock: u64,
    bytes: usize,
}

impl ShadowCache {
    fn get(&mut self, key: u64) -> Option<peniko::ImageBrush> {
        self.clock += 1;
        let (image, last_used) = self.entries.get_mut(&key)?;
        *last_used = self.clock;
        Some(image.clone())
    }

    fn insert(&mut self, key: u64, image: peniko::ImageBrush) {
        self.bytes += image.image.data.len();
        if let Some((old, _)) = self.entries.insert(key, (image, self.clock)) {
            self.bytes -= old.image.data.len();
        }
        if self.bytes > SHADOW_CACHE_BYTES {
            let mut by_age: Vec<(u64, u64)> = self
                .entries
                .iter()
                .map(|(key, (_, last_used))| (*last_used, *key))
                .collect();
            by_age.sort_unstable();
            for (_, key) in by_age {
                if self.bytes <= SHADOW_CACHE_BYTES / 2 {
                    break;
                }
                if let Some((old, _)) = self.entries.remove(&key) {
                    self.bytes -= old.image.data.len();
                }
            }
        }
    }
}

thread_local! {
    static SHADOW_CACHE: std::cell::RefCell<ShadowCache> = std::cell::RefCell::default();
}

/// Renders what `draw` paints into an image, blurs it with a Gaussian of `std_deviation` and
/// tints it with `color`. Renderers without filter support, the GPU one among them, draw the
/// result like any other image. `bounds` must contain the unblurred ink.
///
/// `describe` hashes everything `draw` depends on, with positions taken relative to the image
/// origin it is given. Images with the same hash are reused from a per-thread cache, so a
/// repaint or scroll doesn't render the shadow again.
///
/// Returns the image and the offset to draw it at.
fn blurred_shadow_image(
    bounds: Rect,
    std_deviation: f64,
    color: Color,
    describe: impl FnOnce(&mut DefaultHasher, kurbo::Vec2),
    draw: impl FnOnce(&mut anyrender_vello_cpu::VelloCpuScenePainter, Affine),
) -> Option<(peniko::ImageBrush, kurbo::Vec2)> {
    let bounds = bounds
        .inflate(3.0 * std_deviation, 3.0 * std_deviation)
        .expand();
    if !(1.0..=MAX_SHADOW_IMAGE_SIDE).contains(&bounds.width())
        || !(1.0..=MAX_SHADOW_IMAGE_SIDE).contains(&bounds.height())
    {
        return None;
    }
    let origin = kurbo::Vec2::new(bounds.x0, bounds.y0);
    let (width, height) = (bounds.width() as u32, bounds.height() as u32);

    let mut hasher = DefaultHasher::new();
    describe(&mut hasher, origin);
    (width, height).hash(&mut hasher);
    std_deviation.to_bits().hash(&mut hasher);
    color.to_rgba8().to_u8_array().hash(&mut hasher);
    let key = hasher.finish();
    if let Some(image) = SHADOW_CACHE.with_borrow_mut(|cache| cache.get(key)) {
        return Some((image, origin));
    }

    let coverage = anyrender::render_to_buffer::<anyrender_vello_cpu::VelloCpuImageRenderer, _>(
        |scene| draw(scene, Affine::translate(-origin)),
        width,
        height,
    );
    let mut alpha: Vec<f32> = coverage
        .as_chunks::<4>()
        .0
        .iter()
        .map(|px| f32::from(px[3]) / 255.0)
        .collect();
    gaussian_blur(&mut alpha, width as usize, height as usize, std_deviation);

    let [r, g, b, a] = color.to_rgba8().to_u8_array();
    let data: Vec<u8> = alpha
        .iter()
        .flat_map(|coverage| [r, g, b, (coverage * f32::from(a)).round() as u8])
        .collect();
    let image = peniko::ImageBrush {
        image: peniko::ImageData {
            data: peniko::Blob::new(Arc::new(data)),
            format: peniko::ImageFormat::Rgba8,
            alpha_type: peniko::ImageAlphaType::Alpha,
            width,
            height,
        },
        sampler: peniko::ImageSampler::default(),
    };
    SHADOW_CACHE.with_borrow_mut(|cache| cache.insert(key, image.clone()));
    Some((image, origin))
}

/// Blurs a single-channel image in place by approximating a Gaussian with three box blurs,
/// as the Filter Effects spec describes for `feGaussianBlur`.
pub(crate) fn gaussian_blur(values: &mut [f32], width: usize, height: usize, std_deviation: f64) {
    let size = (std_deviation * 3.0 * (2.0 * std::f64::consts::PI).sqrt() / 4.0 + 0.5) as usize;
    if size < 2 {
        return;
    }
    // Each box is (length, pixels to the left of the output pixel).
    let boxes = if size % 2 == 1 {
        [(size, size / 2); 3]
    } else {
        [(size, size / 2), (size, size / 2 - 1), (size + 1, size / 2)]
    };
    let mut scratch = vec![0.0; width.max(height)];
    for (len, left) in boxes {
        for row in values.chunks_exact_mut(width) {
            box_blur_line(row, 1, &mut scratch[..width], len, left);
        }
    }
    for (len, left) in boxes {
        for x in 0..width {
            box_blur_line(&mut values[x..], width, &mut scratch[..height], len, left);
        }
    }
}

/// Box-blurs the `out.len()` values of `values` that are `stride` apart, treating values past
/// either end as zero.
fn box_blur_line(values: &mut [f32], stride: usize, out: &mut [f32], len: usize, left: usize) {
    let count = out.len();
    let at = |i: isize| -> f32 {
        if i < 0 || i as usize >= count {
            0.0
        } else {
            values[i as usize * stride]
        }
    };
    let mut sum: f32 = (0..len as isize).map(|i| at(i - left as isize)).sum();
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = sum / len as f32;
        let i = i as isize;
        sum += at(i + len as isize - left as isize) - at(i - left as isize);
    }
    for (i, value) in out.iter().enumerate() {
        values[i * stride] = *value;
    }
}
