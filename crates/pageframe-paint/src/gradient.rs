use crate::color::ToColorColor;
use color::{ColorSpaceTag, DynamicColor, Flags, HueDirection, Missing};
use kurbo::{self, Affine, Point, Rect, Vec2};
use num_traits::ToPrimitive as _;
use peniko::{self, ColorStop, Gradient, LinearGradientPosition, SweepGradientPosition};
use style::color::mix::{ColorInterpolationMethod, HueInterpolationMethod};
use style::color::{AbsoluteColor, ColorFlags, ColorSpace};
use style::{
    OwnedSlice,
    values::{
        computed::{
            Angle, AngleOrPercentage, CSSPixelLength, Gradient as StyloGradient, LengthPercentage,
            LineDirection, Percentage,
        },
        generics::{
            NonNegative,
            color::GenericColor,
            image::{
                EndingShape, GenericCircle, GenericEllipse, GenericEndingShape, GenericGradient,
                GenericGradientItem, GradientFlags, ShapeExtent,
            },
            position::GenericPosition,
        },
        specified::{
            percentage::ToPercentage,
            position::{HorizontalPositionKeyword, VerticalPositionKeyword},
        },
    },
};

const MAX_STOP_GAP: f32 = 1.0 / 64.0;
const MAX_PIECES: u16 = 1024;

type GradientItem<T> = GenericGradientItem<GenericColor<Percentage>, T>;
type LinearGradient<'a> = (
    &'a LineDirection,
    &'a [GradientItem<LengthPercentage>],
    GradientFlags,
);
type RadialGradient<'a> = (
    &'a EndingShape<NonNegative<CSSPixelLength>, NonNegative<LengthPercentage>>,
    &'a GenericPosition<LengthPercentage, LengthPercentage>,
    &'a OwnedSlice<GenericGradientItem<GenericColor<Percentage>, LengthPercentage>>,
    GradientFlags,
);
type ConicGradient<'a> = (
    &'a Angle,
    &'a GenericPosition<LengthPercentage, LengthPercentage>,
    &'a OwnedSlice<GenericGradientItem<GenericColor<Percentage>, AngleOrPercentage>>,
    GradientFlags,
);

pub(crate) fn to_peniko_gradient(
    gradient: &StyloGradient,
    origin_rect: Rect,
    scale: f64,
    current_color: &AbsoluteColor,
) -> (peniko::Gradient, Option<Affine>) {
    let css_rect = Rect::new(
        origin_rect.x0 / scale,
        origin_rect.y0 / scale,
        origin_rect.x1 / scale,
        origin_rect.y1 / scale,
    );
    let to_device = |(gradient, transform): (peniko::Gradient, Option<Affine>)| {
        (
            gradient,
            transform.map(|transform| Affine::scale(scale) * transform),
        )
    };
    let (GenericGradient::Linear {
        color_interpolation_method: method,
        ..
    }
    | GenericGradient::Radial {
        color_interpolation_method: method,
        ..
    }
    | GenericGradient::Conic {
        color_interpolation_method: method,
        ..
    }) = gradient;
    let (mut painted, transform) = match gradient {
        // https://developer.mozilla.org/en-US/docs/Web/CSS/gradient/linear-gradient
        GenericGradient::Linear {
            direction,
            items,
            flags,
            // compat_mode,
            ..
        } => linear_gradient(
            (direction, items, *flags),
            origin_rect,
            scale,
            current_color,
            method,
        ),
        GenericGradient::Radial {
            shape,
            position,
            items,
            flags,
            // compat_mode,
            ..
        } => to_device(radial_gradient(
            (shape, position, items, *flags),
            css_rect,
            current_color,
            method,
        )),
        GenericGradient::Conic {
            angle,
            position,
            items,
            flags,
            ..
        } => to_device(conic_gradient(
            (angle, position, items, *flags),
            css_rect,
            current_color,
            method,
        )),
    };
    densify(&mut painted);
    (painted, transform)
}

fn densify(gradient: &mut peniko::Gradient) {
    if gradient.interpolation_cs == ColorSpaceTag::Srgb {
        return;
    }
    let (space, hue) = (gradient.interpolation_cs, gradient.hue_direction);
    let mut dense: Vec<ColorStop> = Vec::with_capacity(gradient.stops.len());
    for pair in gradient.stops.windows(2) {
        let [from, to] = pair else {
            continue;
        };
        let (start, end) = if space == ColorSpaceTag::Oklch {
            (
                chrome_oklch_stop(from.color, to.color),
                chrome_oklch_stop(to.color, from.color),
            )
        } else {
            (None, None)
        };
        dense.push(ColorStop {
            offset: from.offset,
            color: start.unwrap_or(from.color),
        });
        let gap = to.offset - from.offset;
        if !gap.is_finite() || gap <= 0.0 {
            continue;
        }
        let pieces = (gap / MAX_STOP_GAP)
            .ceil()
            .to_u16()
            .unwrap_or(MAX_PIECES)
            .clamp(2, MAX_PIECES);
        let ramp = start
            .unwrap_or(from.color)
            .interpolate(end.unwrap_or(to.color), space, hue);
        for piece in 1..pieces {
            let fraction = f32::from(piece) / f32::from(pieces);
            dense.push(ColorStop {
                offset: from.offset + gap * fraction,
                color: ramp.eval(fraction),
            });
        }
        if let Some(end) = end {
            dense.push(ColorStop {
                offset: to.offset,
                color: end,
            });
        }
    }
    dense.extend(gradient.stops.last().copied());
    gradient.stops = dense.as_slice().into();
    gradient.hue_direction = HueDirection::Shorter;
}

const BLINK_WHITE_A: f32 = 4.547_163e-5;
const BLINK_WHITE_B: f32 = 2.009_6e-5;
const BLINK_ACHROMATIC_CHROMA: f32 = 0.02;
const SKIA_POWERLESS_CHROMA: f32 = 1e-6;
const NEAR_ACHROMATIC_CHROMA: f32 = 1e-3;

struct BlinkLch {
    lightness: f32,
    chroma: f32,
    hue: f32,
    hue_is_none: bool,
}

fn blink_lch_from_lab(lightness: f32, axis_a: f32, axis_b: f32) -> BlinkLch {
    let chroma = axis_a.hypot(axis_b);
    BlinkLch {
        lightness,
        chroma,
        hue: axis_b.atan2(axis_a).to_degrees().rem_euclid(360.0),
        hue_is_none: chroma <= BLINK_ACHROMATIC_CHROMA,
    }
}

fn blink_lch(color: DynamicColor) -> Option<BlinkLch> {
    let missing = color.flags.missing();
    let [lightness, first, second, _] = color.components;
    match color.cs {
        ColorSpaceTag::Oklch => {
            if missing.contains(0) || missing.contains(1) || missing.contains(3) {
                return None;
            }
            Some(BlinkLch {
                lightness,
                chroma: first,
                hue: second,
                hue_is_none: missing.contains(2),
            })
        }
        ColorSpaceTag::Oklab => missing
            .is_empty()
            .then(|| blink_lch_from_lab(lightness, first, second)),
        _ => {
            if !missing.is_empty() {
                return None;
            }
            let [lightness, axis_a, axis_b, _] = color.convert(ColorSpaceTag::Oklab).components;
            Some(blink_lch_from_lab(
                lightness,
                axis_a + lightness * BLINK_WHITE_A,
                axis_b + lightness * BLINK_WHITE_B,
            ))
        }
    }
}

fn chrome_oklch_stop(color: DynamicColor, neighbour: DynamicColor) -> Option<DynamicColor> {
    let own = blink_lch(color)?;
    if !own.hue_is_none && own.chroma > NEAR_ACHROMATIC_CHROMA {
        return None;
    }
    let hue = if own.hue_is_none {
        blink_lch(neighbour)?.hue
    } else {
        own.hue
    };
    let radians = hue.to_radians();
    let axis_a = own.chroma * radians.cos() - own.lightness * BLINK_WHITE_A;
    let axis_b = own.chroma * radians.sin() - own.lightness * BLINK_WHITE_B;
    let chroma = axis_a.hypot(axis_b);
    if chroma <= SKIA_POWERLESS_CHROMA {
        return None;
    }
    Some(DynamicColor {
        cs: ColorSpaceTag::Oklch,
        flags: Flags::default(),
        components: [
            own.lightness,
            chroma,
            axis_b.atan2(axis_a).to_degrees().rem_euclid(360.0),
            color.components[3],
        ],
    })
}

fn color_space_tag(space: ColorSpace) -> ColorSpaceTag {
    match space {
        ColorSpace::Srgb => ColorSpaceTag::Srgb,
        ColorSpace::Hsl => ColorSpaceTag::Hsl,
        ColorSpace::Hwb => ColorSpaceTag::Hwb,
        ColorSpace::Lab => ColorSpaceTag::Lab,
        ColorSpace::Lch => ColorSpaceTag::Lch,
        ColorSpace::Oklab => ColorSpaceTag::Oklab,
        ColorSpace::Oklch => ColorSpaceTag::Oklch,
        ColorSpace::SrgbLinear => ColorSpaceTag::LinearSrgb,
        ColorSpace::DisplayP3 => ColorSpaceTag::DisplayP3,
        ColorSpace::A98Rgb => ColorSpaceTag::A98Rgb,
        ColorSpace::ProphotoRgb => ColorSpaceTag::ProphotoRgb,
        ColorSpace::Rec2020 => ColorSpaceTag::Rec2020,
        ColorSpace::XyzD50 => ColorSpaceTag::XyzD50,
        ColorSpace::XyzD65 | ColorSpace::DisplayP3Linear => ColorSpaceTag::XyzD65,
    }
}

fn stop_color(color: &AbsoluteColor) -> DynamicColor {
    let srgb = color.as_dynamic_color();
    if color.color_space == ColorSpace::DisplayP3Linear {
        return srgb;
    }
    let missing = [
        ColorFlags::C0_IS_NONE,
        ColorFlags::C1_IS_NONE,
        ColorFlags::C2_IS_NONE,
        ColorFlags::ALPHA_IS_NONE,
    ]
    .into_iter()
    .enumerate()
    .filter(|(_, flag)| color.flags.contains(*flag))
    .fold(Missing::default(), |mut missing, (index, _)| {
        missing.insert(index);
        missing
    });
    if matches!(color.color_space, ColorSpace::Oklch | ColorSpace::Oklab) {
        let mut components = *color.raw_components();
        for (index, component) in components.iter_mut().enumerate() {
            if missing.contains(index) {
                *component = 0.0;
            }
        }
        return DynamicColor {
            cs: color_space_tag(color.color_space),
            flags: Flags::from_missing(missing),
            components,
        };
    }
    if missing.is_empty() {
        return srgb;
    }
    let mut own = srgb.convert(color_space_tag(color.color_space));
    for (index, component) in own.components.iter_mut().enumerate() {
        if missing.contains(index) {
            *component = 0.0;
        }
    }
    own.flags = Flags::from_missing(own.flags.missing() | missing);
    own
}

fn interpolated(gradient: peniko::Gradient, method: &ColorInterpolationMethod) -> peniko::Gradient {
    let space = color_space_tag(method.space);
    let hue = match method.hue {
        HueInterpolationMethod::Shorter | HueInterpolationMethod::Specified => {
            HueDirection::Shorter
        }
        HueInterpolationMethod::Longer => HueDirection::Longer,
        HueInterpolationMethod::Increasing => HueDirection::Increasing,
        HueInterpolationMethod::Decreasing => HueDirection::Decreasing,
    };
    gradient
        .with_interpolation_cs(space)
        .with_hue_direction(hue)
}

fn linear_gradient(
    gradient: LinearGradient,
    rect: Rect,
    scale: f64,
    current_color: &AbsoluteColor,
    method: &ColorInterpolationMethod,
) -> (peniko::Gradient, Option<Affine>) {
    let (direction, items, flags) = gradient;

    let center = rect.center();
    let (start, end) = match direction {
        LineDirection::Angle(angle) => {
            let angle = -angle.radians64() + std::f64::consts::PI;
            let offset_length =
                rect.width() / 2.0 * angle.sin().abs() + rect.height() / 2.0 * angle.cos().abs();
            let offset_vec = Vec2::new(angle.sin(), angle.cos()) * offset_length;
            (center - offset_vec, center + offset_vec)
        }
        LineDirection::Horizontal(horizontal) => {
            let start = Point::new(rect.x0, rect.y0 + rect.height() / 2.0);
            let end = Point::new(rect.x1, rect.y0 + rect.height() / 2.0);
            match horizontal {
                HorizontalPositionKeyword::Right => (start, end),
                HorizontalPositionKeyword::Left => (end, start),
            }
        }
        LineDirection::Vertical(vertical) => {
            let start = Point::new(rect.x0 + rect.width() / 2.0, rect.y0);
            let end = Point::new(rect.x0 + rect.width() / 2.0, rect.y1);
            match vertical {
                VerticalPositionKeyword::Top => (end, start),
                VerticalPositionKeyword::Bottom => (start, end),
            }
        }
        LineDirection::Corner(horizontal, vertical) => {
            let (start_x, end_x) = match horizontal {
                HorizontalPositionKeyword::Right => (rect.x0, rect.x1),
                HorizontalPositionKeyword::Left => (rect.x1, rect.x0),
            };
            let (start_y, end_y) = match vertical {
                VerticalPositionKeyword::Top => (rect.y1, rect.y0),
                VerticalPositionKeyword::Bottom => (rect.y0, rect.y1),
            };
            (Point::new(start_x, start_y), Point::new(end_x, end_y))
        }
    };

    let gradient_length = CSSPixelLength::new((start.distance(end) / scale) as f32);
    let repeating = flags.contains(GradientFlags::REPEATING);

    let mut gradient = interpolated(
        peniko::Gradient::new_linear(start, end).with_extend(if repeating {
            peniko::Extend::Repeat
        } else {
            peniko::Extend::Pad
        }),
        method,
    );

    let (first_offset, last_offset) = resolve_length_color_stops(
        current_color,
        items,
        gradient_length,
        &mut gradient,
        repeating,
    );
    if repeating && gradient.stops.len() > 1 {
        gradient.kind = peniko::GradientKind::Linear(LinearGradientPosition {
            start: start + (end - start) * first_offset as f64,
            end: end + (start - end) * (1.0 - last_offset) as f64,
        });
    }

    (gradient, None)
}

fn radial_gradient(
    gradient: RadialGradient,
    rect: Rect,
    current_color: &AbsoluteColor,
    method: &ColorInterpolationMethod,
) -> (peniko::Gradient, Option<Affine>) {
    let (shape, position, items, flags) = gradient;
    let repeating = flags.contains(GradientFlags::REPEATING);

    let mut gradient = interpolated(
        peniko::Gradient::new_radial((0.0, 0.0), 1.0).with_extend(if repeating {
            peniko::Extend::Repeat
        } else {
            peniko::Extend::Pad
        }),
        method,
    );

    let (width_px, height_px) = (
        resolve_gradient_length(&position.horizontal, rect.width()),
        resolve_gradient_length(&position.vertical, rect.height()),
    );

    let gradient_scale: Option<Vec2> = match shape {
        GenericEndingShape::Circle(circle) => {
            let scale = match circle {
                GenericCircle::Extent(extent) => match extent {
                    ShapeExtent::FarthestSide => width_px
                        .max(rect.width() - width_px)
                        .max(height_px.max(rect.height() - height_px)),
                    ShapeExtent::ClosestSide => width_px
                        .min(rect.width() - width_px)
                        .min(height_px.min(rect.height() - height_px)),
                    ShapeExtent::FarthestCorner => width_px
                        .max(rect.width() - width_px)
                        .hypot(height_px.max(rect.height() - height_px)),
                    ShapeExtent::ClosestCorner => width_px
                        .min(rect.width() - width_px)
                        .hypot(height_px.min(rect.height() - height_px)),
                    _ => 0.0,
                },
                GenericCircle::Radius(radius) => radius.0.px() as f64,
            };
            Some(Vec2::new(scale, scale))
        }
        GenericEndingShape::Ellipse(ellipse) => match ellipse {
            GenericEllipse::Extent(extent) => match extent {
                ShapeExtent::FarthestCorner | ShapeExtent::FarthestSide => {
                    let mut scale = Vec2::new(
                        width_px.max(rect.width() - width_px),
                        height_px.max(rect.height() - height_px),
                    );
                    if *extent == ShapeExtent::FarthestCorner {
                        scale *= 2.0_f64.sqrt();
                    }
                    Some(scale)
                }
                ShapeExtent::ClosestCorner | ShapeExtent::ClosestSide => {
                    let mut scale = Vec2::new(
                        width_px.min(rect.width() - width_px),
                        height_px.min(rect.height() - height_px),
                    );
                    if *extent == ShapeExtent::ClosestCorner {
                        scale *= 2.0_f64.sqrt();
                    }
                    Some(scale)
                }
                _ => None,
            },
            GenericEllipse::Radii(x, y) => Some(Vec2::new(
                resolve_gradient_length(&x.0, rect.width()),
                resolve_gradient_length(&y.0, rect.height()),
            )),
        },
    };

    let gradient_transform = {
        // If the gradient has no valid scale, we don't need to calculate the color stops
        if let Some(gradient_scale) = gradient_scale {
            let (first_offset, last_offset) = resolve_length_color_stops(
                current_color,
                items,
                CSSPixelLength::new(gradient_scale.x as f32),
                &mut gradient,
                repeating,
            );
            let scale = if repeating && gradient.stops.len() >= 2 {
                (last_offset - first_offset) as f64
            } else {
                1.0
            };
            Some(
                Affine::scale_non_uniform(gradient_scale.x * scale, gradient_scale.y * scale)
                    .then_translate(get_translation(position, rect)),
            )
        } else {
            None
        }
    };

    (gradient, gradient_transform)
}

fn conic_gradient(
    gradient: ConicGradient,
    rect: Rect,
    current_color: &AbsoluteColor,
    method: &ColorInterpolationMethod,
) -> (peniko::Gradient, Option<Affine>) {
    let (angle, position, items, flags) = gradient;

    let repeating = flags.contains(GradientFlags::REPEATING);
    let mut gradient = interpolated(
        peniko::Gradient::new_sweep((0.0, 0.0), 0.0, std::f32::consts::PI * 2.0).with_extend(
            if repeating {
                peniko::Extend::Repeat
            } else {
                peniko::Extend::Pad
            },
        ),
        method,
    );

    let (first_offset, last_offset) = resolve_angle_color_stops(
        current_color,
        items,
        CSSPixelLength::new(1.0),
        &mut gradient,
        repeating,
    );
    if repeating && gradient.stops.len() >= 2 {
        gradient.kind = peniko::GradientKind::Sweep(SweepGradientPosition {
            center: Point::new(0.0, 0.0),
            start_angle: std::f32::consts::PI * 2.0 * first_offset,
            end_angle: std::f32::consts::PI * 2.0 * last_offset,
        });
    }

    let gradient_transform = Some(
        Affine::rotate(angle.radians() as f64 - std::f64::consts::PI / 2.0)
            .then_translate(get_translation(position, rect)),
    );

    (gradient, gradient_transform)
}

#[inline]
fn resolve_length_color_stops(
    current_color: &AbsoluteColor,
    items: &[GradientItem<LengthPercentage>],
    gradient_length: CSSPixelLength,
    gradient: &mut Gradient,
    repeating: bool,
) -> (f32, f32) {
    resolve_color_stops(
        current_color,
        items,
        gradient_length,
        gradient,
        repeating,
        |gradient_length: CSSPixelLength, position: &LengthPercentage| -> Option<f32> {
            position
                .to_percentage_of(gradient_length)
                .map(|percentage| percentage.to_percentage().unwrap_or(0.0))
        },
    )
}

#[inline]
fn resolve_color_stops<T>(
    current_color: &AbsoluteColor,
    items: &[GradientItem<T>],
    gradient_length: CSSPixelLength,
    gradient: &mut Gradient,
    repeating: bool,
    item_resolver: impl Fn(CSSPixelLength, &T) -> Option<f32>,
) -> (f32, f32) {
    let mut hint: Option<f32> = None;

    for (idx, item) in items.iter().enumerate() {
        let (color, offset) = match item {
            GenericGradientItem::SimpleColorStop(color) => {
                let step = 1.0 / (items.len() as f32 - 1.0);
                (
                    stop_color(&color.resolve_to_absolute(current_color)),
                    step * idx as f32,
                )
            }
            GenericGradientItem::ComplexColorStop { color, position } => {
                let offset = item_resolver(gradient_length, position);
                if let Some(offset) = offset {
                    (
                        stop_color(&color.resolve_to_absolute(current_color)),
                        offset,
                    )
                } else {
                    continue;
                }
            }
            GenericGradientItem::InterpolationHint(position) => {
                hint = item_resolver(gradient_length, position);
                continue;
            }
        };

        if idx == 0 && !repeating && offset != 0.0 {
            gradient
                .stops
                .push(peniko::ColorStop { color, offset: 0.0 });
        }

        match hint {
            None => gradient.stops.push(peniko::ColorStop { color, offset }),
            Some(hint) => {
                let &last_stop = gradient.stops.last().unwrap();

                if hint <= last_stop.offset {
                    // Upstream code has a bug here, so we're going to do something different
                    match gradient.stops.len() {
                        0 => (),
                        1 => {
                            gradient.stops.pop();
                        }
                        _ => {
                            let prev_stop = gradient.stops[gradient.stops.len() - 2];
                            if prev_stop.offset == hint {
                                gradient.stops.pop();
                            }
                        }
                    }
                    gradient.stops.push(peniko::ColorStop {
                        color,
                        offset: hint,
                    });
                } else if hint >= offset {
                    gradient.stops.push(peniko::ColorStop {
                        color: last_stop.color,
                        offset: hint,
                    });
                    gradient.stops.push(peniko::ColorStop {
                        color,
                        offset: last_stop.offset,
                    });
                } else if hint == (last_stop.offset + offset) / 2.0 {
                    gradient.stops.push(peniko::ColorStop { color, offset });
                } else {
                    let mid_point = (hint - last_stop.offset) / (offset - last_stop.offset);
                    let ramp = last_stop.color.interpolate(
                        color,
                        gradient.interpolation_cs,
                        gradient.hue_direction,
                    );
                    let mut interpolate_stop = |cur_offset: f32| {
                        let relative_offset =
                            (cur_offset - last_stop.offset) / (offset - last_stop.offset);
                        let multiplier = relative_offset.powf(0.5f32.log(mid_point));
                        gradient.stops.push(peniko::ColorStop {
                            color: ramp.eval(multiplier),
                            offset: cur_offset,
                        });
                    };
                    if mid_point > 0.5 {
                        for i in 0..7 {
                            interpolate_stop(
                                last_stop.offset
                                    + (hint - last_stop.offset) * (7.0 + i as f32) / 13.0,
                            );
                        }
                        interpolate_stop(hint + (offset - hint) / 3.0);
                        interpolate_stop(hint + (offset - hint) * 2.0 / 3.0);
                    } else {
                        interpolate_stop(last_stop.offset + (hint - last_stop.offset) / 3.0);
                        interpolate_stop(last_stop.offset + (hint - last_stop.offset) * 2.0 / 3.0);
                        for i in 0..7 {
                            interpolate_stop(hint + (offset - hint) * (i as f32) / 13.0);
                        }
                    }
                    gradient.stops.push(peniko::ColorStop { color, offset });
                }
            }
        }
    }

    // Post-process the stops for repeating gradients
    if repeating && gradient.stops.len() > 1 {
        let first_offset = gradient.stops.first().unwrap().offset;
        let last_offset = gradient.stops.last().unwrap().offset;
        if first_offset != 0.0 || last_offset != 1.0 {
            let scale_inv = 1e-7_f32.max(1.0 / (last_offset - first_offset));
            for stop in &mut *gradient.stops {
                stop.offset = (stop.offset - first_offset) * scale_inv;
            }
        }
        (first_offset, last_offset)
    } else {
        // Ensure that the gradient ends at offset 1.0
        if gradient.stops.len() > 1 {
            let last_stop = &gradient.stops.last().unwrap();
            if last_stop.offset < 1.0 {
                let last_stop = ColorStop {
                    offset: 1.0,
                    ..(**last_stop)
                };
                gradient.stops.push(last_stop);
            }
        }
        (0.0, 1.0)
    }
}

#[inline]
fn resolve_angle_color_stops(
    current_color: &AbsoluteColor,
    items: &[GradientItem<AngleOrPercentage>],
    gradient_length: CSSPixelLength,
    gradient: &mut Gradient,
    repeating: bool,
) -> (f32, f32) {
    resolve_color_stops(
        current_color,
        items,
        gradient_length,
        gradient,
        repeating,
        |_gradient_length: CSSPixelLength, position: &AngleOrPercentage| -> Option<f32> {
            match position {
                AngleOrPercentage::Angle(angle) => {
                    Some(angle.radians() / (std::f64::consts::PI * 2.0) as f32)
                }
                AngleOrPercentage::Percentage(percentage) => {
                    Some(percentage.to_percentage().unwrap_or(0.0))
                }
            }
        },
    )
}

#[inline]
fn resolve_gradient_length(value: &LengthPercentage, basis: f64) -> f64 {
    // Percentage resolution can overflow even though computed lengths are finite.
    value
        .resolve(CSSPixelLength::new(basis as f32))
        .px()
        .clamp(f32::MIN, f32::MAX) as f64
}

#[inline]
fn get_translation(
    position: &GenericPosition<LengthPercentage, LengthPercentage>,
    rect: Rect,
) -> Vec2 {
    Vec2::new(
        rect.x0 + resolve_gradient_length(&position.horizontal, rect.width()),
        rect.y0 + resolve_gradient_length(&position.vertical, rect.height()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_percentage_overflow_is_clamped() {
        for percentage in [f32::MAX, f32::MIN] {
            let value = LengthPercentage::new_percent(Percentage(percentage));
            assert_eq!(resolve_gradient_length(&value, 300.0), percentage as f64);
        }
    }

    #[test]
    fn finite_gradient_lengths_are_preserved() {
        let length = LengthPercentage::new_length(CSSPixelLength::new(-12.0));
        let percentage = LengthPercentage::new_percent(Percentage(-0.5));
        assert_eq!(resolve_gradient_length(&length, 300.0), -12.0);
        assert_eq!(resolve_gradient_length(&percentage, 300.0), -150.0);
    }
}
