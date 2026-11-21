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

mod hard_stops;
mod stop_colors;
mod stops;

use stop_colors::{chrome_oklch_stop, interpolated};
use stops::{resolve_angle_color_stops, resolve_length_color_stops};

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
    if matches!(gradient, GenericGradient::Linear { .. }) {
        hard_stops::snap_hard_stops(&mut painted);
    }
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
