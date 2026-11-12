use kurbo::{Arc, Point, Vec2};
use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2};

const TOLERANCE: f64 = 0.25;
const MAX_SPLITS: u32 = 5;
const MAX_SHIFT: u32 = 6;
const FIXED_SCALE: f64 = 256.0;
const TALL_STEP: f64 = 2.0;
const ANGLE_SLACK: f64 = 1e-9;

pub(crate) fn arc_points(arc: &Arc, phase: f64) -> Vec<Point> {
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
            let t = f64::from(step) / f64::from(steps);
            let mt = 1.0 - t;
            let raw = (top.to_vec2() * (mt * mt)
                + control.to_vec2() * (2.0 * mt * t)
                + bottom.to_vec2() * (t * t))
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
}
