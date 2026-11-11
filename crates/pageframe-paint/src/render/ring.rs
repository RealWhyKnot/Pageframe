use kurbo::{BezPath, Point, Rect, RoundedRectRadii, Vec2};

const TOLERANCE: f64 = 0.25;
const MAX_SPLITS: u32 = 4;
const MAX_SHIFT: u32 = 6;
const FIXED_SCALE: f64 = 256.0;

pub(super) fn ring_path(rect: Rect, radii: RoundedRectRadii, offset: f64, width: f64) -> BezPath {
    let half = width / 2.0;
    let mut path = BezPath::new();
    push_rounded_rect(
        &mut path,
        rect.inflate(offset + half, offset + half),
        grow(radii, half),
    );
    push_rounded_rect(
        &mut path,
        rect.inflate(offset - half, offset - half),
        grow(radii, -half),
    );
    path
}

fn grow(radii: RoundedRectRadii, by: f64) -> RoundedRectRadii {
    let grow = |radius: f64| {
        if radius > 0.0 {
            (radius + by).max(0.0)
        } else {
            0.0
        }
    };
    RoundedRectRadii::new(
        grow(radii.top_left),
        grow(radii.top_right),
        grow(radii.bottom_right),
        grow(radii.bottom_left),
    )
}

fn fit(rect: Rect, radii: RoundedRectRadii) -> RoundedRectRadii {
    let ratio = |side: f64, a: f64, b: f64| if a + b > side { side / (a + b) } else { 1.0 };
    let scale = ratio(rect.width(), radii.top_left, radii.top_right)
        .min(ratio(rect.width(), radii.bottom_left, radii.bottom_right))
        .min(ratio(rect.height(), radii.top_left, radii.bottom_left))
        .min(ratio(rect.height(), radii.top_right, radii.bottom_right));
    RoundedRectRadii::new(
        radii.top_left * scale,
        radii.top_right * scale,
        radii.bottom_right * scale,
        radii.bottom_left * scale,
    )
}

fn push_rounded_rect(path: &mut BezPath, rect: Rect, radii: RoundedRectRadii) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let radii = fit(rect, radii);
    let left = Vec2::new(-1.0, 0.0);
    let up = Vec2::new(0.0, -1.0);
    let right = Vec2::new(1.0, 0.0);
    let down = Vec2::new(0.0, 1.0);
    let corners = [
        (Point::new(rect.x0, rect.y0), radii.top_left, left, up),
        (Point::new(rect.x1, rect.y0), radii.top_right, up, right),
        (
            Point::new(rect.x1, rect.y1),
            radii.bottom_right,
            right,
            down,
        ),
        (Point::new(rect.x0, rect.y1), radii.bottom_left, down, left),
    ];
    let mut points = Vec::new();
    for (corner, radius, from, to) in corners {
        if radius > 0.0 {
            let center = corner - (from + to) * radius;
            push_arc(&mut points, center, radius, from, to);
        } else {
            points.push(corner);
        }
    }
    for (index, point) in points.into_iter().enumerate() {
        let point = Point::new(point.x, snap_y(point.y));
        if index == 0 {
            path.move_to(point);
        } else {
            path.line_to(point);
        }
    }
    path.close_path();
}

fn push_arc(points: &mut Vec<Point>, center: Point, radius: f64, from: Vec2, to: Vec2) {
    let mut quads = Vec::new();
    split_arc(&mut quads, center, radius, (from, to), (0.0, 1.0), 0);
    points.push(center + from * radius);
    for (start, control, end) in quads {
        let segments = segment_count(start, control, end);
        for step in 1..=segments {
            let t = f64::from(step) / f64::from(segments);
            let mt = 1.0 - t;
            points.push(
                (start.to_vec2() * (mt * mt)
                    + control.to_vec2() * (2.0 * mt * t)
                    + end.to_vec2() * (t * t))
                    .to_point(),
            );
        }
    }
}

fn split_arc(
    quads: &mut Vec<(Point, Point, Point)>,
    center: Point,
    radius: f64,
    arc: (Vec2, Vec2),
    range: (f64, f64),
    depth: u32,
) {
    let first = conic_direction(arc, range.0);
    let last = conic_direction(arc, range.1);
    let start = center + first * radius;
    let end = center + last * radius;
    let control = center + (first + last) * (radius / (1.0 + first.dot(last)));
    let middle = (range.0 + range.1) / 2.0;
    let target = center + conic_direction(arc, middle) * radius;
    let quad_middle =
        ((start.to_vec2() + control.to_vec2() * 2.0 + end.to_vec2()) / 4.0).to_point();
    if depth < MAX_SPLITS && target.distance(quad_middle) > TOLERANCE {
        split_arc(quads, center, radius, arc, (range.0, middle), depth + 1);
        split_arc(quads, center, radius, arc, (middle, range.1), depth + 1);
    } else {
        quads.push((start, control, end));
    }
}

fn conic_direction((from, to): (Vec2, Vec2), t: f64) -> Vec2 {
    let weight = std::f64::consts::FRAC_1_SQRT_2;
    let mt = 1.0 - t;
    let point = from * (mt * mt) + (from + to) * (2.0 * weight * mt * t) + to * (t * t);
    point.normalize()
}

fn segment_count(start: Point, control: Point, end: Point) -> u32 {
    let fixed = |value: f64| (value * FIXED_SCALE).trunc();
    let deviation =
        |a: f64, b: f64, c: f64| ((2.0 * fixed(b) - fixed(a) - fixed(c)) / 4.0).floor().abs();
    let dx = deviation(start.x, control.x, end.x);
    let dy = deviation(start.y, control.y, end.y);
    let distance = dx.max(dy) + (dx.min(dy) / 2.0).floor();
    let mut remaining = ((distance + 16.0) / 32.0).floor();
    let mut bits = 0;
    while remaining >= 1.0 {
        remaining = (remaining / 2.0).floor();
        bits += 1;
    }
    2_u32.pow((bits / 2).clamp(1, MAX_SHIFT))
}

fn snap_y(y: f64) -> f64 {
    ((y + 0.125) * 4.0).floor() / 4.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corner_quads(radius: f64) -> Vec<(Point, Point, Point)> {
        let mut quads = Vec::new();
        let arc = (Vec2::new(-1.0, 0.0), Vec2::new(0.0, -1.0));
        split_arc(
            &mut quads,
            Point::new(100.0, 100.0),
            radius,
            arc,
            (0.0, 1.0),
            0,
        );
        quads
    }

    fn lines_per_quad(radius: f64) -> Vec<u32> {
        corner_quads(radius)
            .into_iter()
            .map(|(start, control, end)| segment_count(start, control, end))
            .collect()
    }

    #[test]
    fn small_corners_stay_one_quad() {
        assert_eq!(lines_per_quad(1.0), [2]);
        assert_eq!(lines_per_quad(2.0), [2]);
        assert_eq!(lines_per_quad(3.0), [4]);
    }

    #[test]
    fn larger_corners_split_like_skia() {
        assert_eq!(lines_per_quad(6.0), [2, 2]);
    }

    #[test]
    fn quads_meet_the_arc_ends() {
        let quads = corner_quads(6.0);
        let first = quads.first().unwrap();
        let last = quads.last().unwrap();
        assert!(first.0.distance(Point::new(94.0, 100.0)) < 1e-9);
        assert!(last.2.distance(Point::new(100.0, 94.0)) < 1e-9);
        for pair in quads.windows(2) {
            assert!(pair[0].2.distance(pair[1].0) < 1e-9);
        }
    }

    #[test]
    fn y_snaps_to_quarter_pixels() {
        assert_eq!(snap_y(10.0), 10.0);
        assert_eq!(snap_y(10.124), 10.0);
        assert_eq!(snap_y(10.125), 10.25);
        assert_eq!(snap_y(10.6), 10.5);
        assert_eq!(snap_y(-0.2), -0.25);
    }

    #[test]
    fn ring_spans_both_edges_of_the_stroke() {
        let rect = Rect::new(10.0, 20.0, 110.0, 50.0);
        let ring = ring_path(rect, RoundedRectRadii::from_single_radius(2.0), 1.0, 2.0);
        let bounds = kurbo::Shape::bounding_box(&ring);
        assert_eq!(bounds, Rect::new(8.0, 18.0, 112.0, 52.0));
        let moves = ring
            .elements()
            .iter()
            .filter(|element| matches!(element, kurbo::PathEl::MoveTo(_)))
            .count();
        assert_eq!(moves, 2);
    }

    #[test]
    fn oversized_radii_shrink_to_fit() {
        let rect = Rect::new(0.0, 0.0, 10.0, 4.0);
        let radii = fit(rect, RoundedRectRadii::from_single_radius(5.0));
        assert_eq!(radii.top_left, 2.0);
        assert_eq!(radii.bottom_right, 2.0);
    }

    #[test]
    fn empty_inner_edge_is_skipped() {
        let rect = Rect::new(0.0, 0.0, 0.0, 0.0);
        let ring = ring_path(rect, RoundedRectRadii::from_single_radius(0.0), 0.0, 2.0);
        let moves = ring
            .elements()
            .iter()
            .filter(|element| matches!(element, kurbo::PathEl::MoveTo(_)))
            .count();
        assert_eq!(moves, 1);
    }
}
