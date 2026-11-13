use anyrender::PaintScene;
use kurbo::{Affine, BezPath, Circle, PathEl, Point, Rect, Shape as _};
use peniko::{Color, Fill};

use super::open_dash_gap;
use crate::{kurbo_css::Edge, render::ElementCx};

const DOT_EPSILON: f64 = 0.01;
const DOT_FLATTEN_TOLERANCE: f64 = 0.01;

fn dot_tolerance(transform: Affine) -> f64 {
    let [a, b, c, d, _, _] = transform.as_coeffs();
    let scale = a.hypot(b).max(c.hypot(d));
    if scale.is_finite() && scale > 0.0 {
        DOT_FLATTEN_TOLERANCE / scale
    } else {
        DOT_FLATTEN_TOLERANCE
    }
}

fn push_round_dot(path: &mut BezPath, center: Point, radius: f64, tolerance: f64) {
    kurbo::flatten(
        Circle::new(center, radius).path_elements(tolerance),
        tolerance,
        |element| path.push(element),
    );
}

fn square_dot_spans(start: f64, length: f64, width: f64) -> Vec<(f64, f64)> {
    if length <= 2.0 * width {
        return vec![(start, start + length)];
    }
    let dash = if length <= 3.0 * width {
        length / 3.0
    } else {
        width
    };
    let mod_4 = length.rem_euclid(4.0);
    let mod_6 = length.rem_euclid(6.0);
    let thin = width == 1.0;
    let medium = width == 2.0;
    let thick = width == 3.0;
    let mut spans = Vec::new();
    let mut first = start;
    let mut last = start + length;

    let mut start_dot = None;
    if (thin && length.rem_euclid(2.0) == 0.0) || (thick && mod_6 == 0.0) {
        start_dot = Some((1.0, 1.0));
    }
    if (medium && (mod_4 == 0.0 || mod_4 == 1.0)) || (thick && (mod_6 == 1.0 || mod_6 == 2.0)) {
        start_dot = Some((0.0, -1.0));
    }
    if (medium && mod_4 == 3.0) || (thick && (mod_6 == 4.0 || mod_6 == 5.0)) {
        start_dot = Some((0.0, 1.0));
    }
    let end_growth = if (medium && mod_4 == 0.0) || (thick && (mod_6 == 1.0 || mod_6 == 5.0)) {
        Some(0.0)
    } else if thick && mod_6 == 0.0 {
        Some(1.0)
    } else {
        None
    };

    if let Some((growth, offset)) = start_dot {
        spans.push((first, first + width + growth));
        first += 2.0 * width + offset;
    }
    if let Some(growth) = end_growth {
        spans.push((last - width - growth, last));
        last -= width + growth + 1.0;
    }
    let mut position = first;
    while position < last {
        spans.push((position, (position + dash).min(last)));
        position += 2.0 * dash;
    }
    spans
}

fn round_dot_centers(start: f64, length: f64, width: f64) -> Vec<f64> {
    let first = start + width / 2.0;
    if length < 2.0 * width {
        return vec![first];
    }
    let last = start + length - width / 2.0;
    let step = open_dash_gap(length, width, width) + width - DOT_EPSILON;
    if !step.is_finite() || step <= 0.0 {
        return vec![first];
    }
    let mut centers = Vec::new();
    let mut position = first;
    while position <= last {
        centers.push(position);
        position += step;
    }
    centers
}

fn points_along(path: &BezPath, spacing: f64, count: f64) -> Vec<Point> {
    let mut polyline: Vec<Point> = Vec::new();
    kurbo::flatten(path.iter(), 0.1, |element| match element {
        PathEl::MoveTo(point) | PathEl::LineTo(point) => polyline.push(point),
        PathEl::ClosePath => {
            if let Some(&first) = polyline.first() {
                polyline.push(first);
            }
        }
        _ => {}
    });
    let mut points = Vec::new();
    if polyline.len() < 2 {
        return points;
    }
    let mut segment = 0;
    let mut segment_start = 0.0;
    let mut segment_length = (polyline[1] - polyline[0]).hypot();
    let mut index = 0.0;
    while index < count {
        let target = index * spacing;
        while segment + 2 < polyline.len() && target > segment_start + segment_length {
            segment_start += segment_length;
            segment += 1;
            segment_length = (polyline[segment + 1] - polyline[segment]).hypot();
        }
        let along = if segment_length > 0.0 {
            ((target - segment_start) / segment_length).clamp(0.0, 1.0)
        } else {
            0.0
        };
        points.push(polyline[segment].lerp(polyline[segment + 1], along));
        index += 1.0;
    }
    points
}

impl ElementCx<'_, '_> {
    pub(super) fn draw_dotted_border_edge(
        &self,
        scene: &mut impl PaintScene,
        edge: Edge,
        color: Color,
        clip: &BezPath,
    ) {
        let thickness = self.edge_width(edge);
        if thickness <= 0.0 {
            return;
        }
        if self.frame.has_border_radius() {
            self.draw_round_dotted_border_edge(scene, edge, color, thickness);
            return;
        }
        let bounds = self.frame.border_box;
        let (start, length) = match edge {
            Edge::Top | Edge::Bottom => (bounds.x0, bounds.width().round()),
            Edge::Left | Edge::Right => (bounds.y0, bounds.height().round()),
        };
        if length <= 0.0 {
            return;
        }
        let (across_start, across_end) = match edge {
            Edge::Top => (bounds.y0, bounds.y0 + thickness),
            Edge::Bottom => (bounds.y1 - thickness, bounds.y1),
            Edge::Left => (bounds.x0, bounds.x0 + thickness),
            Edge::Right => (bounds.x1 - thickness, bounds.x1),
        };
        let vertical = matches!(edge, Edge::Left | Edge::Right);
        let mut path = BezPath::new();
        let width = thickness.round();
        if width <= 3.0 {
            for (from, to) in square_dot_spans(start, length, width.max(1.0)) {
                let rect = if vertical {
                    Rect::new(across_start, from, across_end, to)
                } else {
                    Rect::new(from, across_start, to, across_end)
                };
                path.extend(rect.path_elements(0.1));
            }
        } else {
            let middle = (across_start + across_end) / 2.0;
            let tolerance = dot_tolerance(self.transform);
            for along in round_dot_centers(start, length, width) {
                let center = if vertical {
                    Point::new(middle, along)
                } else {
                    Point::new(along, middle)
                };
                push_round_dot(&mut path, center, width / 2.0, tolerance);
            }
        }
        scene.push_clip_layer(Fill::NonZero, self.transform, clip);
        scene.fill(Fill::NonZero, self.transform, color, None, &path);
        scene.pop_layer();
    }

    fn draw_round_dotted_border_edge(
        &self,
        scene: &mut impl PaintScene,
        edge: Edge,
        color: Color,
        thickness: f64,
    ) {
        let mut centerline = self.frame.border_slice(0.0, 0.5).padding_box_path();
        centerline.close_path();
        let perimeter = centerline.perimeter(0.1);
        if perimeter <= 0.0 {
            return;
        }
        let count = (perimeter / (2.0 * thickness)).round().max(1.0);
        let mut path = BezPath::new();
        let tolerance = dot_tolerance(self.transform);
        for center in points_along(&centerline, perimeter / count, count) {
            push_round_dot(&mut path, center, thickness / 2.0, tolerance);
        }
        let clip = self.frame.border_edge_shape(edge);
        scene.push_clip_layer(Fill::NonZero, self.transform, &clip);
        scene.fill(Fill::NonZero, self.transform, color, None, &path);
        scene.pop_layer();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worst_device_error(scale: f64) -> f64 {
        let center = Point::new(10.0, 10.0);
        let radius = 3.0;
        let mut path = BezPath::new();
        push_round_dot(
            &mut path,
            center,
            radius,
            dot_tolerance(Affine::scale(scale)),
        );
        let mut previous = None;
        let mut worst: f64 = 0.0;
        for element in path.elements() {
            if let PathEl::MoveTo(point) | PathEl::LineTo(point) = *element {
                if let Some(start) = previous {
                    let middle = Point::lerp(start, point, 0.5);
                    worst = worst.max(radius - (middle - center).hypot());
                }
                previous = Some(point);
            }
        }
        worst * scale
    }

    #[test]
    fn dots_stay_round_in_device_pixels_under_large_scales() {
        for scale in [1.0, 2.0, 20.0, 100.0] {
            let error = worst_device_error(scale);
            assert!(error < 0.012, "scale {scale}: off by {error} device px");
        }
    }
}
