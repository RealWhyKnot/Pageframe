use kurbo::{Arc, BezPath, PathEl, Point, Vec2};
use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2};

const TOLERANCE: f64 = 0.25;
const MAX_SPLITS: u32 = 5;
const MAX_SHIFT: u32 = 6;
const FIXED_SCALE: f64 = 256.0;
const TALL_STEP: f64 = 2.0;
const ANGLE_SLACK: f64 = 1e-9;
const QUARTER_HANDLE: f64 = 0.552_284_749_830_793_6;
const HANDLE_SLACK: f64 = 0.01;

pub(crate) fn flatten_quarter_arcs(path: &BezPath, phase: f64) -> BezPath {
    let mut flat = BezPath::new();
    let mut start = Point::ZERO;
    let mut current = Point::ZERO;
    for element in path.elements() {
        match *element {
            PathEl::MoveTo(point) => {
                flat.move_to(point);
                start = point;
                current = point;
            }
            PathEl::LineTo(point) => {
                flat.line_to(point);
                current = point;
            }
            PathEl::QuadTo(control, point) => {
                flat.quad_to(control, point);
                current = point;
            }
            PathEl::CurveTo(first, second, point) => {
                match quarter_arc([current, first, second, point]) {
                    Some(arc) => {
                        for along in arc_points(&arc, phase) {
                            flat.line_to(along);
                        }
                    }
                    None => flat.curve_to(first, second, point),
                }
                current = point;
            }
            PathEl::ClosePath => {
                flat.close_path();
                current = start;
            }
        }
    }
    flat
}

fn quarter_arc([from, first, second, to]: [Point; 4]) -> Option<Arc> {
    let center = if first.y == from.y && second.x == to.x {
        Point::new(from.x, to.y)
    } else if first.x == from.x && second.y == to.y {
        Point::new(to.x, from.y)
    } else {
        return None;
    };
    let radii = Vec2::new((to.x - from.x).abs(), (to.y - from.y).abs());
    if radii.x <= 0.0 || radii.y <= 0.0 {
        return None;
    }
    let handle = |near: Point, end: Point| {
        let reach = near - end;
        let span = reach.x.abs() / radii.x + reach.y.abs() / radii.y;
        (span - QUARTER_HANDLE).abs() <= HANDLE_SLACK
    };
    let inward = (first - from).dot(to - from) > 0.0 && (second - to).dot(from - to) > 0.0;
    if !inward || !handle(first, from) || !handle(second, to) {
        return None;
    }
    let unit = |point: Point| {
        let offset = point - center;
        Vec2::new(offset.x / radii.x, offset.y / radii.y)
    };
    let (start, end) = (unit(from), unit(to));
    let sweep = if start.cross(end) > 0.0 {
        FRAC_PI_2
    } else {
        -FRAC_PI_2
    };
    Some(Arc::new(center, radii, start.y.atan2(start.x), sweep, 0.0))
}
pub(super) fn arc_points(arc: &Arc, phase: f64) -> Vec<Point> {
    let end = arc.start_angle + arc.sweep_angle;
    let (low, high) = if arc.sweep_angle < 0.0 {
        (end, arc.start_angle)
    } else {
        (arc.start_angle, end)
    };
    let base = ((low + high) / 2.0 / FRAC_PI_2).floor() * FRAC_PI_2;
    let from = Vec2::new(base.cos().round(), base.sin().round());
    let to = Vec2::new(-from.y, from.x);
    let quarter = Quarter {
        center: arc.center,
        radii: arc.radii,
        from,
        to,
    };

    let mut points = Vec::new();
    for quad in quarter.quads() {
        quad_lines(&mut points, quad, phase);
    }
    let mut points = quarter.trim(points, low - base, high - base);
    if arc.sweep_angle < 0.0 {
        points.reverse();
    }
    points
}

struct Quarter {
    center: Point,
    radii: Vec2,
    from: Vec2,
    to: Vec2,
}

impl Quarter {
    fn place(&self, unit: Vec2) -> Point {
        self.center + Vec2::new(unit.x * self.radii.x, unit.y * self.radii.y)
    }

    fn unit(&self, angle: f64) -> Vec2 {
        let (sin, cos) = angle.sin_cos();
        self.from * cos + self.to * sin
    }

    fn angle(&self, point: Point) -> f64 {
        let offset = point - self.center;
        let unit = Vec2::new(offset.x / self.radii.x, offset.y / self.radii.y);
        unit.dot(self.to).atan2(unit.dot(self.from))
    }

    fn quads(&self) -> Vec<[Point; 3]> {
        let factor = (FRAC_1_SQRT_2 - 1.0) / (4.0 * (1.0 + FRAC_1_SQRT_2));
        let bend = self.place(self.from + self.to) - self.center;
        let mut error = (bend * factor).hypot();
        let mut splits = 0;
        while splits < MAX_SPLITS && error > TOLERANCE {
            error *= 0.25;
            splits += 1;
        }
        let count = 1_u32 << splits;
        let step = FRAC_PI_2 / f64::from(count);
        let reach = (step / 2.0).tan();
        (0..count)
            .map(|index| {
                let turn = step * f64::from(index);
                let first = self.unit(turn);
                let tangent = self.unit(turn + FRAC_PI_2);
                let last = if index + 1 == count {
                    self.to
                } else {
                    self.unit(turn + step)
                };
                [
                    self.place(first),
                    self.place(first + tangent * reach),
                    self.place(last),
                ]
            })
            .collect()
    }

    fn cut(&self, points: &[Point], angle: f64) -> Option<Point> {
        let aim = self.unit(angle);
        points.windows(2).find_map(|pair| {
            if self.angle(pair[1]) < angle {
                return None;
            }
            let first = pair[0] - self.center;
            let first = Vec2::new(first.x / self.radii.x, first.y / self.radii.y);
            let second = pair[1] - self.center;
            let second = Vec2::new(second.x / self.radii.x, second.y / self.radii.y);
            let span = second - first;
            let slant = span.cross(aim);
            if slant == 0.0 {
                return Some(pair[0]);
            }
            let along = (-first.cross(aim) / slant).clamp(0.0, 1.0);
            Some(pair[0] + (pair[1] - pair[0]) * along)
        })
    }

    fn trim(&self, points: Vec<Point>, low: f64, high: f64) -> Vec<Point> {
        let starts_inside = low > ANGLE_SLACK;
        let ends_inside = high < FRAC_PI_2 - ANGLE_SLACK;
        if !starts_inside && !ends_inside {
            return points;
        }
        let mut kept = Vec::with_capacity(points.len() + 2);
        if starts_inside && let Some(point) = self.cut(&points, low) {
            kept.push(point);
        }
        for (index, point) in points.iter().enumerate() {
            let is_first = index == 0;
            let is_last = index + 1 == points.len();
            let angle = self.angle(*point);
            let after_low = if starts_inside {
                !is_first && angle > low
            } else {
                true
            };
            let before_high = if ends_inside {
                !is_last && angle < high
            } else {
                true
            };
            if after_low && before_high {
                kept.push(*point);
            }
        }
        if ends_inside && let Some(point) = self.cut(&points, high) {
            kept.push(point);
        }
        kept
    }
}

fn quad_lines(points: &mut Vec<Point>, [start, control, end]: [Point; 3], phase: f64) {
    let rises = start.y > end.y;
    let (top, bottom) = if rises { (end, start) } else { (start, end) };
    let steps = segment_count(top, control, bottom);
    let last_y = snap_y(bottom.y, phase);
    let mut previous = Point::new(top.x, snap_y(top.y, phase).min(last_y));
    let mut previous_y = top.y;
    let mut run = vec![previous];
    for step in 1..=steps {
        let point = if step == steps {
            Point::new(bottom.x, last_y)
        } else {
            let along = f64::from(step) / f64::from(steps);
            let back = 1.0 - along;
            let raw = (top.to_vec2() * (back * back)
                + control.to_vec2() * (2.0 * back * along)
                + bottom.to_vec2() * (along * along))
                .to_point();
            let is_tall = (raw.y - previous_y).abs() >= TALL_STEP;
            previous_y = raw.y;
            if is_tall {
                let rise = raw.y - previous.y;
                let slope = if rise == 0.0 {
                    0.0
                } else {
                    (raw.x - previous.x) / rise
                };
                let y = whole_y(raw.y, phase).min(last_y);
                Point::new(raw.x - slope * (raw.y - y), y)
            } else {
                Point::new(raw.x, snap_y(raw.y, phase).min(last_y))
            }
        };
        previous = point;
        run.push(point);
    }
    if rises {
        run.reverse();
    }
    for point in run {
        if points.last() != Some(&point) {
            points.push(point);
        }
    }
}

fn segment_count(start: Point, control: Point, end: Point) -> u32 {
    let fixed = |value: f64| (value * FIXED_SCALE).trunc();
    let deviation = |from: f64, middle: f64, to: f64| {
        ((2.0 * fixed(middle) - fixed(from) - fixed(to)) / 4.0)
            .floor()
            .abs()
    };
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

fn snap_y(y: f64, phase: f64) -> f64 {
    ((y + phase + 0.125) * 4.0).floor() / 4.0 - phase
}

fn whole_y(y: f64, phase: f64) -> f64 {
    (y + phase + 0.5).floor() - phase
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quarter(radius: f64) -> Arc {
        Arc::new(
            Point::new(100.0, 100.0),
            Vec2::new(radius, radius),
            std::f64::consts::PI,
            FRAC_PI_2,
            0.0,
        )
    }

    #[test]
    fn a_quarter_runs_between_its_ends() {
        let points = arc_points(&quarter(28.0), 0.0);
        assert_eq!(points.first(), Some(&Point::new(72.0, 100.0)));
        let last = points.last().expect("an arc has points");
        assert!(last.distance(Point::new(100.0, 72.0)) < 1e-9);
        assert_eq!(points.len(), 9);
    }

    #[test]
    fn small_corners_use_two_lines() {
        assert_eq!(arc_points(&quarter(2.0), 0.0).len(), 3);
        assert_eq!(arc_points(&quarter(6.0), 0.0).len(), 5);
    }

    #[test]
    fn points_sit_on_quarter_pixels() {
        for point in arc_points(&quarter(9.5), 0.0) {
            assert_eq!(point.y * 4.0, (point.y * 4.0).round());
        }
        for point in arc_points(&quarter(9.5), 0.4) {
            let device = (point.y + 0.4) * 4.0;
            assert!((device - device.round()).abs() < 1e-9);
        }
    }

    #[test]
    fn a_backwards_sweep_reverses_the_points() {
        let forwards = arc_points(&quarter(12.0), 0.0);
        let mut backwards = arc_points(
            &Arc::new(
                Point::new(100.0, 100.0),
                Vec2::new(12.0, 12.0),
                std::f64::consts::PI + FRAC_PI_2,
                -FRAC_PI_2,
                0.0,
            ),
            0.0,
        );
        backwards.reverse();
        assert_eq!(forwards.len(), backwards.len());
        for (first, second) in forwards.iter().zip(&backwards) {
            assert!(first.distance(*second) < 1e-9);
        }
    }

    #[test]
    fn half_a_corner_stops_on_the_full_outline() {
        let whole = arc_points(&quarter(28.0), 0.0);
        let half = arc_points(
            &Arc::new(
                Point::new(100.0, 100.0),
                Vec2::new(28.0, 28.0),
                std::f64::consts::PI,
                FRAC_PI_2 / 2.0,
                0.0,
            ),
            0.0,
        );
        assert_eq!(half.first(), whole.first());
        let last = half.last().expect("an arc has points");
        assert!((last.x - last.y).abs() < 1e-9);
        assert!(half.len() < whole.len());
        for point in &half[..half.len() - 1] {
            assert!(whole.contains(point));
        }
    }

    #[test]
    fn ellipses_keep_both_radii() {
        let points = arc_points(
            &Arc::new(
                Point::new(60.0, 30.0),
                Vec2::new(60.0, 30.0),
                std::f64::consts::PI,
                FRAC_PI_2,
                0.0,
            ),
            0.0,
        );
        assert_eq!(points.first(), Some(&Point::new(0.0, 30.0)));
        let last = points.last().expect("an arc has points");
        assert!(last.distance(Point::new(60.0, 0.0)) < 1e-9);
    }

    fn quarter_curve(radii: Vec2) -> BezPath {
        let handle = radii * QUARTER_HANDLE;
        let mut path = BezPath::new();
        path.move_to((radii.x, 0.0));
        path.curve_to((radii.x, handle.y), (handle.x, radii.y), (0.0, radii.y));
        path
    }

    fn has_curves(path: &BezPath) -> bool {
        path.elements()
            .iter()
            .any(|element| matches!(element, PathEl::CurveTo(..)))
    }

    #[test]
    fn an_elliptical_quarter_curve_becomes_lines_on_the_ellipse() {
        let radii = Vec2::new(40.0, 25.0);
        let flat = flatten_quarter_arcs(&quarter_curve(radii), 0.0);
        assert!(!has_curves(&flat));
        assert!(flat.elements().len() > 4);
        for element in flat.elements() {
            if let PathEl::LineTo(point) = element {
                let unit = Vec2::new(point.x / radii.x, point.y / radii.y);
                assert!(
                    (unit.hypot() - 1.0).abs() < 0.02,
                    "{point:?} is off the ellipse"
                );
            }
        }
    }

    #[test]
    fn a_curve_running_the_other_way_flattens_onto_the_same_circle() {
        let handle = 20.0 * QUARTER_HANDLE;
        let mut path = BezPath::new();
        path.move_to((0.0, 20.0));
        path.curve_to((handle, 20.0), (20.0, handle), (20.0, 0.0));
        let flat = flatten_quarter_arcs(&path, 0.0);
        assert!(!has_curves(&flat));
        let mut points = Vec::new();
        for element in flat.elements() {
            if let PathEl::LineTo(point) = element {
                assert!(
                    (point.to_vec2().hypot() - 20.0).abs() < 0.2,
                    "{point:?} is off the circle"
                );
                points.push(*point);
            }
        }
        let last = points.last().expect("the arc has points");
        assert!(last.distance(Point::new(20.0, 0.0)) < 0.2);
        assert!(points.first().expect("the arc has points").y > last.y);
    }

    #[test]
    fn other_curves_stay_curves() {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.curve_to((10.0, 0.0), (20.0, 5.0), (30.0, 30.0));
        assert!(has_curves(&flatten_quarter_arcs(&path, 0.0)));
    }

    #[test]
    fn a_curve_with_a_handle_pointing_outwards_stays_a_curve() {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.curve_to((0.0, 5.5), (15.5, 10.0), (10.0, 10.0));
        assert!(has_curves(&flatten_quarter_arcs(&path, 0.0)));
    }
}
