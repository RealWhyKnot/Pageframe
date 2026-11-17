use crate::color::Color;
use anyrender::PaintScene;
use kurbo::{Affine, Point, Rect, Vec2};
use peniko::Fill;

pub(super) type Pixels = Vec<Span>;

#[derive(Clone, Copy)]
pub(super) struct Span {
    pub(super) row: f64,
    pub(super) start: f64,
    pub(super) end: f64,
}

const LAYOUT_UNITS: f64 = 64.0;
const UNIT_SLACK: f64 = 1.0 / 32.0;
const NOISE_ULPS: f64 = 4.0;

pub(super) fn device_round(value: f64) -> f64 {
    let units = value * LAYOUT_UNITS;
    let slack = (units.abs() * NOISE_ULPS * f64::from(f32::EPSILON)).max(UNIT_SLACK);
    let truncated = (units.abs() + slack).floor().copysign(units);
    ((truncated + LAYOUT_UNITS / 2.0) / LAYOUT_UNITS).floor()
}

pub(super) fn snap_offset(transform: Affine, origin: Point) -> Vec2 {
    let [a, b, c, d, _, _] = transform.as_coeffs();
    if a != 1.0 || b != 0.0 || c != 0.0 || d != 1.0 {
        return Vec2::ZERO;
    }
    let device = transform * origin;
    Vec2::new(device.x.round() - device.x, device.y.round() - device.y)
}

pub(super) fn device_space(transform: Affine, rect: Rect) -> (Affine, Rect) {
    let [a, b, c, d, e, f] = transform.as_coeffs();
    if a != 1.0 || b != 0.0 || c != 0.0 || d != 1.0 {
        return (transform, rect);
    }
    let device = rect + Vec2::new(e, f);
    let snapped = Rect::new(
        device.x0.round(),
        device.y0.round(),
        device.x1.round(),
        device.y1.round(),
    );
    (Affine::IDENTITY, snapped)
}

pub(super) fn fill_pixels(
    scene: &mut impl PaintScene,
    transform: Affine,
    pixels: &[Span],
    color: Color,
) {
    let mut spans = pixels.to_vec();
    spans.sort_by(|a, b| a.row.total_cmp(&b.row).then(a.start.total_cmp(&b.start)));
    let mut merged: Vec<Span> = Vec::with_capacity(spans.len());
    for span in spans {
        match merged.last_mut() {
            Some(last) if last.row == span.row && span.start <= last.end => {
                last.end = last.end.max(span.end);
            }
            _ => merged.push(span),
        }
    }
    merged.sort_by(|a, b| {
        a.start
            .total_cmp(&b.start)
            .then(a.end.total_cmp(&b.end))
            .then(a.row.total_cmp(&b.row))
    });
    let mut rects: Vec<Rect> = Vec::with_capacity(merged.len());
    for span in merged {
        match rects.last_mut() {
            Some(last) if last.x0 == span.start && last.x1 == span.end && last.y1 == span.row => {
                last.y1 = span.row + 1.0;
            }
            _ => rects.push(Rect::new(span.start, span.row, span.end, span.row + 1.0)),
        }
    }
    for rect in rects {
        scene.fill(Fill::NonZero, transform, color, None, &rect);
    }
}
