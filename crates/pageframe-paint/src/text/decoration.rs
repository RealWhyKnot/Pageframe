use super::*;

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
    let min_gap = (stroke_length - min_num_dashes * dash_length) / min_num_gaps;
    let max_gap = (stroke_length - max_num_dashes * dash_length) / max_num_gaps;
    if max_gap <= 0.0 || (min_gap - gap_length).abs() < (max_gap - gap_length).abs() {
        min_gap
    } else {
        max_gap
    }
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
    decoration_thickness(thickness, metric_size, css_font_size, scale)
        .floor()
        .max(1.0) as f32
}

fn decoration_thickness(
    thickness: &TextDecorationLength,
    metric_size: f32,
    css_font_size: f64,
    scale: f64,
) -> f64 {
    match thickness {
        GenericTextDecorationLength::LengthPercentage(lp) => {
            (lp.resolve(Length::new(css_font_size as f32)).px() as f64 * scale)
                .round()
                .max(1.0)
        }
        GenericTextDecorationLength::FromFont => f64::from(metric_size),
        GenericTextDecorationLength::Auto => (css_font_size / 10.0).max(1.0) * scale,
    }
}

/// Draws a single decoration line of the given `text-decoration-style` spanning
/// `[x0, x0 + width]`. `baseline` is the line's baseline and `offset` positions the line
/// relative to it (`y = baseline - offset + size / 2`, growing downward). Geometry is built
/// explicitly (rather than relying on backend dash support) so it renders consistently
/// across renderers.
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
    kind: DecorationKind,
    thickness: f64,
    deco_style: TextDecorationStyle,
    inset_start: f64,
    inset_end: f64,
    skips: &[(f64, f64)],
) {
    // Inset the line from the box's start/end edges (assumes LTR: start is the left edge).
    // Negative insets extend the line past the text.
    let x = x0 + inset_start;
    let w = width - inset_start - inset_end;
    if w <= 0.0 {
        return;
    }
    let top = decoration_top(transform, baseline, offset);
    let size = f64::from(size);
    let y = top + size / 2.0;
    let butt_stroke = Stroke::new(size).with_caps(Cap::Butt);
    let spans = visible_spans(x, x + w, skips);
    let is_clipped = matches!(deco_style, TextDecorationStyle::Wavy)
        || (!skips.is_empty()
            && matches!(
                deco_style,
                TextDecorationStyle::Dotted | TextDecorationStyle::Dashed
            ));
    if is_clipped {
        let reach = (4.0 * size).max(6.0 * thickness) + scale + 2.0;
        let mut clip = BezPath::new();
        for &(start, end) in &spans {
            clip.extend(Rect::new(start, y - reach, end, y + reach).path_elements(0.1));
        }
        scene.push_clip_layer(Fill::NonZero, transform, &clip);
    }

    match deco_style {
        TextDecorationStyle::None => {
            // no line. Equivalent to `text-decoration-line: none`
        }
        // `solid`
        TextDecorationStyle::Solid => {
            for &(start, end) in &spans {
                let line = kurbo::Line::new((start, y), (end, y));
                scene.stroke(&butt_stroke, transform, brush, None, &line);
            }
        }
        // Two lines, each `size` thick, separated by a 1px (device) gap. This
        // matches Chrome, where the second line is offset by `size + 1px`
        // and sits further from the text (below for underline, above for
        // overline).
        TextDecorationStyle::Double => {
            let away = if matches!(kind, DecorationKind::Overline) {
                -1.0
            } else {
                1.0
            };
            for cy in [y, y + away * (size + 1.0)] {
                for &(start, end) in &spans {
                    let line = kurbo::Line::new((start, cy), (end, cy));
                    scene.stroke(&butt_stroke, transform, brush, None, &line);
                }
            }
        }
        TextDecorationStyle::Dotted | TextDecorationStyle::Dashed => {
            let dotted = matches!(deco_style, TextDecorationStyle::Dotted);
            let thickness = thickness.max(1.0);
            let whole = thickness.round();
            let origin = transform.translation();
            let start = (x + origin.x).trunc() - origin.x;
            let end = (x + w + origin.x).trunc() - origin.x;
            let length = (x + w + origin.x).trunc() - (x + origin.x).trunc();
            let mut middle = (top + origin.y + (thickness / 2.0).max(0.5)).floor() - origin.y;
            if whole % 2.0 == 1.0 {
                middle += 0.5;
            }
            let quarter = |value: f64| ((value + origin.y) * 4.0).round() / 4.0 - origin.y;
            if dotted && whole > 3.0 {
                let first = start + whole / 2.0;
                let last = end - whole / 2.0;
                let step = if length < 2.0 * whole {
                    2.0 * whole
                } else {
                    select_best_dash_gap(length, whole, whole) + whole - 0.01
                };
                let radius = thickness / 2.0;
                let mut dots = BezPath::new();
                if step.is_finite() {
                    let mut centre = first;
                    while centre <= last || centre == first {
                        let circle = Circle::new((centre, middle), radius);
                        kurbo::flatten(circle.path_elements(0.01), 0.01, |element| {
                            dots.push(element)
                        });
                        centre += step;
                    }
                } else if step.is_infinite() {
                    let capsule = RoundedRect::new(
                        first - radius,
                        middle - radius,
                        last + radius,
                        middle + radius,
                        radius,
                    );
                    kurbo::flatten(capsule.path_elements(0.01), 0.01, |element| {
                        dots.push(element)
                    });
                }
                scene.fill(Fill::NonZero, transform, brush, None, &dots);
            } else {
                let (dash, gap) = match (dotted, whole >= 3.0) {
                    (true, _) => (whole, whole),
                    (false, true) => (2.0 * whole, whole),
                    (false, false) => (3.0 * whole, 2.0 * whole),
                };
                let (dash, gap) = if length <= 2.0 * dash {
                    (length, 0.0)
                } else if length <= 2.0 * dash + gap {
                    let squeeze = length / (2.0 * dash + gap);
                    (dash * squeeze, gap * squeeze)
                } else if dotted {
                    (dash, gap)
                } else {
                    (dash, select_best_dash_gap(length, dash, gap))
                };
                let (upper, lower) = if dotted {
                    (middle - thickness / 2.0, middle + thickness / 2.0)
                } else {
                    (
                        quarter(middle - thickness / 2.0),
                        quarter(middle + thickness / 2.0),
                    )
                };
                let mut from = start;
                while from < end && dash > 0.0 {
                    let rect = Rect::new(from, upper, (from + dash).min(end), lower);
                    scene.fill(Fill::NonZero, transform, brush, None, &rect);
                    from += dash + gap;
                }
            }
        }
        TextDecorationStyle::Wavy => {
            let thickness = thickness.max(1.0);
            let wavelength = 1.0 + 2.0 * (2.0 * thickness + 0.5).round();
            let control = 0.5 + (3.0 * thickness + 0.5).round();
            let shift = match kind {
                DecorationKind::Underline => thickness + 1.0,
                DecorationKind::Overline => -(thickness + 1.0),
                DecorationKind::LineThrough => 0.0,
            };
            let origin_y = transform.translation().y;
            let middle = (top + origin_y + shift - 0.5).ceil() - origin_y + 0.5;
            let mut start = x - wavelength;
            let mut path = BezPath::new();
            path.move_to((start, middle));
            while start < x + w + wavelength {
                let half = start + wavelength / 2.0;
                path.curve_to(
                    (half, middle + control),
                    (half, middle - control),
                    (start + wavelength, middle),
                );
                start += wavelength;
            }
            if thickness <= 1.0 {
                scene.fill(Fill::NonZero, transform, brush, None, &hairline(&path));
            } else {
                let stroke = Stroke::new(thickness).with_caps(Cap::Butt);
                scene.stroke(&stroke, transform, brush, None, &path);
            }
        }
    }
    if is_clipped {
        scene.pop_layer();
    }
}

fn hairline(path: &BezPath) -> BezPath {
    let mut ribbon = BezPath::new();
    let mut previous: Option<Point> = None;
    kurbo::flatten(path, 0.05, |element| match element {
        PathEl::MoveTo(point) => previous = Some(point),
        PathEl::LineTo(point) => {
            if let Some(from) = previous {
                let delta = point - from;
                let (start, end, half) = if delta.x.abs() >= delta.y.abs() {
                    let (start, end) = if from.x <= point.x {
                        (from, point)
                    } else {
                        (point, from)
                    };
                    (start, end, Vec2::new(0.0, 0.5))
                } else {
                    let (start, end) = if from.y <= point.y {
                        (from, point)
                    } else {
                        (point, from)
                    };
                    (start, end, Vec2::new(-0.5, 0.0))
                };
                ribbon.move_to(start - half);
                ribbon.line_to(end - half);
                ribbon.line_to(end + half);
                ribbon.line_to(start + half);
                ribbon.close_path();
            }
            previous = Some(point);
        }
        _ => {}
    });
    ribbon
}

#[derive(Clone, Copy)]
enum DecorationKind {
    Underline,
    Overline,
    LineThrough,
}

fn decoration_top(transform: Affine, baseline: f32, offset: f32) -> f64 {
    let origin_y = transform.translation().y;
    crate::render::device_round(f64::from(baseline - offset) + origin_y) - origin_y
}

fn visible_spans(start: f64, end: f64, skips: &[(f64, f64)]) -> SmallVec<[(f64, f64); 4]> {
    let mut spans = SmallVec::new();
    let mut from = start;
    for &(skip_start, skip_end) in skips {
        if from >= end {
            break;
        }
        if skip_start > from {
            spans.push((from, skip_start.min(end)));
        }
        from = from.max(skip_end);
    }
    if from < end {
        spans.push((from, end));
    }
    spans
}
/// Paint the decorations accumulated for one line, one line per decorating box.
pub(super) fn flush_line_decorations(
    scene: &mut impl PaintScene,
    transform: Affine,
    options: TextPaintOptions<'_>,
    deco_boxes: &[LineDecoration],
    win_ascent_ratios: &mut WinAscentCache,
    ink: &mut LineInk<'_, '_>,
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
        let transform = transform * Affine::translate(acc.shift);
        // Prefer the decorating box's own font; fall back to the first run it covers.
        let geom = match (&acc.own, &acc.first) {
            (Some(own), _) => own.clone(),
            (None, Some(first)) => {
                let mut geom = first.clone();
                // A descendant run's baseline may be shifted by `vertical-align`, but the
                // decoration is positioned from the decorating box's own baseline. For the
                // inline root that is the line's baseline.
                if acc.on_line_baseline {
                    geom.baseline = ink.line.metrics().baseline;
                }
                geom
            }
            (None, None) => continue,
        };
        let geom = &geom;
        let ascent_shift = win_ascent(win_ascent_ratios, &geom.font, geom.font_size)
            .map_or(0.0, |ascent| f64::from(ascent - ascent.round()));
        let ink_thickness = decoration_thickness(
            &deco.thickness,
            geom.underline_size,
            geom.css_font_size,
            scale,
        );
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
            } else if deco.underline_from_font {
                geom.underline_offset
            } else {
                let thickness = decoration_thickness(
                    &deco.thickness,
                    geom.underline_size,
                    geom.css_font_size,
                    scale,
                );
                -(thickness / 2.0).ceil().max(1.0).to_f32().unwrap_or(1.0)
            };
            let offset = base_offset - extra_underline_offset as f32;
            let top = decoration_top(transform, geom.baseline, offset);
            let skips = ink.skips(acc, transform, top + ascent_shift, ink_thickness);

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
                DecorationKind::Underline,
                ink_thickness,
                deco.style,
                inset_start,
                inset_end,
                &skips,
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
            let top = decoration_top(transform, geom.baseline, offset);
            let skips = ink.skips(acc, transform, top + ascent_shift, ink_thickness);

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
                DecorationKind::Overline,
                ink_thickness,
                deco.style,
                inset_start,
                inset_end,
                &skips,
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
            let half_thickness = decoration_thickness(
                &deco.thickness,
                geom.strikethrough_size,
                geom.css_font_size,
                scale,
            ) / 2.0;
            let ascent = ascent.round();
            let offset = ascent
                - (f64::from(ascent) * 2.0 / 3.0 - half_thickness)
                    .round()
                    .to_f32()
                    .unwrap_or(0.0);

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
                DecorationKind::LineThrough,
                2.0 * half_thickness,
                deco.style,
                inset_start,
                inset_end,
                &ink.holes,
            );
        }
    }
}
