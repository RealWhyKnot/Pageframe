use kurbo::{BezPath, Insets, Point, Rect, Vec2};
use num_traits::ToPrimitive as _;

use super::dashed::dashed_ratios;
use crate::kurbo_css::CssBox;

const CONIC_WEIGHT: f32 = std::f32::consts::FRAC_1_SQRT_2;
const MAX_T: u32 = 0x3fff_ffff;

fn scalar(value: f64) -> f32 {
    value.to_f32().unwrap_or(0.0)
}

struct Segment {
    points: [Point; 3],
    curved: bool,
    distances: Vec<(f32, f32)>,
    start: f32,
    end: f32,
}

impl Segment {
    fn point(&self, t: f32) -> Point {
        let [first, control, last] = self.points;
        let x = |p: Point| scalar(p.x);
        let y = |p: Point| scalar(p.y);
        let coordinate = |a: f32, b: f32, c: f32| {
            if self.curved {
                let middle = 2.0 * CONIC_WEIGHT * t * (1.0 - t);
                let first = (1.0 - t) * (1.0 - t);
                let last = t * t;
                (first * a + middle * b + last * c) / (first + middle + last)
            } else {
                a + (c - a) * t
            }
        };
        Point::new(
            f64::from(coordinate(x(first), x(control), x(last))),
            f64::from(coordinate(y(first), y(control), y(last))),
        )
    }

    fn measure(&mut self, low: u32, high: u32, from: Point, to: Point, depth: u32) {
        let middle = (low + high) >> 1;
        let t = scalar(f64::from(middle)) / scalar(f64::from(MAX_T));
        let halfway = self.point(t);
        let dx = scalar(halfway.x) - (scalar(from.x) + scalar(to.x)) * 0.5;
        let dy = scalar(halfway.y) - (scalar(from.y) + scalar(to.y)) * 0.5;
        if self.curved && depth < 8 && (high - low) >> 10 > 0 && dx.abs().max(dy.abs()) > 0.5 {
            self.measure(low, middle, from, halfway, depth + 1);
            self.measure(middle, high, halfway, to, depth + 1);
            return;
        }
        let dx = scalar(to.x) - scalar(from.x);
        let dy = scalar(to.y) - scalar(from.y);
        let next = self.end + (dx * dx + dy * dy).sqrt();
        if next > self.end {
            self.end = next;
            self.distances
                .push((next, scalar(f64::from(high)) / scalar(f64::from(MAX_T))));
        }
    }

    fn parameter(&self, distance: f32) -> f32 {
        let mut start = (self.start, 0.0);
        for &(end, t) in &self.distances {
            if distance <= end {
                return start.1 + (t - start.1) * (distance - start.0) / (end - start.0);
            }
            start = (end, t);
        }
        1.0
    }

    fn append(&self, path: &mut BezPath, from: f32, to: f32, move_to: bool) {
        let start = self.point(from);
        let end = self.point(to);
        if move_to {
            path.move_to(start);
        }
        if !self.curved {
            path.line_to(end);
            return;
        }
        let [first, corner, last] = self.points;
        let center = first + (last - corner);
        let radius = Vec2::new((first.x - last.x).abs(), (first.y - last.y).abs());
        let unit = |p: Point| Vec2::new((p.x - center.x) / radius.x, (p.y - center.y) / radius.y);
        let a = unit(start);
        let b = unit(end);
        let angle = a.cross(b).atan2(a.dot(b));
        let handle = 4.0 / 3.0 * (angle / 4.0).tan();
        let tangent = |v: Vec2| Vec2::new(-v.y * radius.x, v.x * radius.y);
        path.curve_to(start + tangent(a) * handle, end - tangent(b) * handle, end);
    }
}

fn contour(frame: &CssBox, offset: Vec2) -> Vec<Segment> {
    let widths = frame.border_width;
    let half = Insets::new(
        (widths.x0 / 2.0).trunc(),
        (widths.y0 / 2.0).trunc(),
        (widths.x1 / 2.0).trunc(),
        (widths.y1 / 2.0).trunc(),
    );
    let rect = frame.border_box;
    let bounds = Rect::new(
        (rect.x0 + offset.x).round() - offset.x,
        (rect.y0 + offset.y).round() - offset.y,
        (rect.x1 + offset.x).round() - offset.x,
        (rect.y1 + offset.y).round() - offset.y,
    ) - half;
    let reduce = |radius: Vec2, x: f64, y: f64| {
        let reduced = Vec2::new((radius.x - x).max(0.0), (radius.y - y).max(0.0));
        if reduced.x == 0.0 || reduced.y == 0.0 {
            Vec2::ZERO
        } else {
            reduced
        }
    };
    let radii = frame.border_radii;
    let tl = reduce(radii.top_left, half.x0, half.y0);
    let tr = reduce(radii.top_right, half.x1, half.y0);
    let br = reduce(radii.bottom_right, half.x1, half.y1);
    let bl = reduce(radii.bottom_left, half.x0, half.y1);
    let corners = [
        (
            Point::new(bounds.x1 - tr.x, bounds.y0),
            Point::new(bounds.x1, bounds.y0),
            Point::new(bounds.x1, bounds.y0 + tr.y),
        ),
        (
            Point::new(bounds.x1, bounds.y1 - br.y),
            Point::new(bounds.x1, bounds.y1),
            Point::new(bounds.x1 - br.x, bounds.y1),
        ),
        (
            Point::new(bounds.x0 + bl.x, bounds.y1),
            Point::new(bounds.x0, bounds.y1),
            Point::new(bounds.x0, bounds.y1 - bl.y),
        ),
        (
            Point::new(bounds.x0, bounds.y0 + tl.y),
            Point::new(bounds.x0, bounds.y0),
            Point::new(bounds.x0 + tl.x, bounds.y0),
        ),
    ];
    let mut current = corners[3].2;
    let mut distance = 0.0;
    let mut segments = Vec::new();
    for (first, control, last) in corners {
        for (points, curved) in [
            ([current, current, first], false),
            ([first, control, last], true),
        ] {
            let mut segment = Segment {
                points,
                curved,
                distances: Vec::new(),
                start: distance,
                end: distance,
            };
            segment.measure(0, MAX_T, points[0], points[2], 0);
            distance = segment.end;
            if segment.end > segment.start {
                segments.push(segment);
            }
        }
        current = last;
    }
    segments
}

fn append_span(path: &mut BezPath, segments: &[Segment], from: f32, to: f32, mut move_to: bool) {
    for segment in segments {
        if segment.end <= from || segment.start >= to {
            continue;
        }
        let start = segment.parameter(from.max(segment.start));
        let end = segment.parameter(to.min(segment.end));
        segment.append(path, start, end, move_to);
        move_to = false;
    }
}

fn pattern(length: f32, thickness: f32) -> (f32, f32) {
    let (dash_ratio, gap_ratio) = dashed_ratios(f64::from(thickness));
    let dash = scalar(dash_ratio) * thickness;
    let gap = scalar(gap_ratio) * thickness;
    if length <= 2.0 * dash {
        return (dash, 0.0);
    }
    let two = 2.0 * (dash + gap);
    if length <= two {
        let scale = length / two;
        return (dash * scale, gap * scale);
    }
    let fewer = (length / (dash + gap)).floor();
    let more = fewer + 1.0;
    let fewer_gap = (length - fewer * dash) / fewer;
    let more_gap = (length - more * dash) / more;
    let gap = if more_gap <= 0.0 || (fewer_gap - gap).abs() < (more_gap - gap).abs() {
        fewer_gap
    } else {
        more_gap
    };
    (dash, gap)
}

pub(super) fn dashed_contour(frame: &CssBox, thickness: f64, offset: Vec2) -> Option<BezPath> {
    let segments = contour(frame, offset);
    let length = segments.last()?.end;
    let (dash, gap) = pattern(length.trunc(), scalar(thickness));
    let mut path = BezPath::new();
    if gap <= 0.0 {
        append_span(&mut path, &segments, 0.0, length, true);
        path.close_path();
        return Some(path);
    }
    if length / (dash + gap) > 1_000_000.0 {
        return None;
    }
    let step = f64::from(dash) + f64::from(gap);
    let mut distance = step;
    let mut last_reaches_end = false;
    while distance < f64::from(length) {
        let end = (distance + f64::from(dash)).min(f64::from(length));
        append_span(&mut path, &segments, scalar(distance), scalar(end), true);
        last_reaches_end = end >= f64::from(length);
        distance += step;
    }
    append_span(
        &mut path,
        &segments,
        0.0,
        dash.min(length),
        !last_reaches_end,
    );
    Some(path)
}
