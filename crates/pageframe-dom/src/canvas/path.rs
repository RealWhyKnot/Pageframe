use std::f64::consts::{FRAC_PI_2, PI, TAU};

use kurbo::{Affine, Arc, BezPath, PathEl, Point, Vec2};

use super::Canvas2D;

const ARC_TOLERANCE: f64 = 0.01;

fn arc_tolerance(radii: Vec2) -> f64 {
    ARC_TOLERANCE.max(radii.x.abs().max(radii.y.abs()) * 1e-9)
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Path {
    pub(crate) bez: BezPath,
    start: Point,
    current: Option<Point>,
    reopen: bool,
}

impl Path {
    fn move_to(&mut self, p: Point) {
        self.bez.move_to(p);
        self.start = p;
        self.current = Some(p);
        self.reopen = false;
    }

    fn segment_start(&mut self, p: Point) {
        if self.current.is_none() {
            self.move_to(p);
        } else if self.reopen {
            self.bez.move_to(self.start);
            self.reopen = false;
        }
    }

    fn line_to(&mut self, p: Point) {
        if self.current.is_none() {
            self.move_to(p);
            return;
        }
        self.segment_start(p);
        self.bez.line_to(p);
        self.current = Some(p);
    }

    fn quad_to(&mut self, c: Point, p: Point) {
        self.segment_start(c);
        self.bez.quad_to(c, p);
        self.current = Some(p);
    }

    fn curve_to(&mut self, c0: Point, c1: Point, p: Point) {
        self.segment_start(c0);
        self.bez.curve_to(c0, c1, p);
        self.current = Some(p);
    }

    fn close(&mut self) {
        if self.current.is_some() && !self.reopen {
            self.bez.close_path();
            self.current = Some(self.start);
            self.reopen = true;
        }
    }

    fn append(&mut self, elements: impl IntoIterator<Item = PathEl>, transform: Affine) {
        for el in elements {
            match el {
                PathEl::MoveTo(p) => self.move_to(transform * p),
                PathEl::LineTo(p) => self.line_to(transform * p),
                PathEl::QuadTo(c, p) => self.quad_to(transform * c, transform * p),
                PathEl::CurveTo(c0, c1, p) => {
                    self.curve_to(transform * c0, transform * c1, transform * p);
                }
                PathEl::ClosePath => self.close(),
            }
        }
    }
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|v| v.is_finite())
}

fn sweep(start: f64, end: f64, anticlockwise: bool) -> f64 {
    if !anticlockwise && end - start >= TAU {
        TAU
    } else if anticlockwise && start - end >= TAU {
        -TAU
    } else if !anticlockwise && start > end {
        TAU - (start - end) % TAU
    } else if anticlockwise && start < end {
        -(TAU - (end - start) % TAU)
    } else {
        end - start
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Radii {
    pub x: f64,
    pub y: f64,
}

impl Canvas2D {
    pub fn begin_path(&mut self) {
        self.path = Path::default();
    }

    pub fn close_path(&mut self) {
        self.path.close();
    }

    pub fn move_to(&mut self, x: f64, y: f64) {
        if finite(&[x, y]) {
            self.path.move_to(self.state.transform * Point::new(x, y));
        }
    }

    pub fn line_to(&mut self, x: f64, y: f64) {
        if finite(&[x, y]) {
            self.path.line_to(self.state.transform * Point::new(x, y));
        }
    }

    pub fn quadratic_curve_to(&mut self, cx: f64, cy: f64, x: f64, y: f64) {
        if finite(&[cx, cy, x, y]) {
            let t = self.state.transform;
            self.path
                .quad_to(t * Point::new(cx, cy), t * Point::new(x, y));
        }
    }

    pub fn bezier_curve_to(&mut self, c0x: f64, c0y: f64, c1x: f64, c1y: f64, x: f64, y: f64) {
        if finite(&[c0x, c0y, c1x, c1y, x, y]) {
            let t = self.state.transform;
            self.path.curve_to(
                t * Point::new(c0x, c0y),
                t * Point::new(c1x, c1y),
                t * Point::new(x, y),
            );
        }
    }

    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        if !finite(&[x, y, w, h]) {
            return;
        }
        let t = self.state.transform;
        self.path.move_to(t * Point::new(x, y));
        self.path.line_to(t * Point::new(x + w, y));
        self.path.line_to(t * Point::new(x + w, y + h));
        self.path.line_to(t * Point::new(x, y + h));
        self.path.close();
        self.path.move_to(t * Point::new(x, y));
    }

    pub fn arc(
        &mut self,
        x: f64,
        y: f64,
        radius: f64,
        start: f64,
        end: f64,
        anticlockwise: bool,
    ) -> bool {
        self.ellipse(
            Point::new(x, y),
            Vec2::new(radius, radius),
            0.0,
            (start, end),
            anticlockwise,
        )
    }

    pub fn ellipse(
        &mut self,
        center: Point,
        radii: Vec2,
        rotation: f64,
        (start, end): (f64, f64),
        anticlockwise: bool,
    ) -> bool {
        if !finite(&[center.x, center.y, radii.x, radii.y, rotation, start, end]) {
            return true;
        }
        if radii.x < 0.0 || radii.y < 0.0 {
            return false;
        }
        let arc = Arc {
            center,
            radii,
            start_angle: start,
            sweep_angle: sweep(start, end, anticlockwise),
            x_rotation: rotation,
        };
        let t = self.state.transform;
        let at = |angle: f64| {
            let offset = Vec2::new(radii.x * angle.cos(), radii.y * angle.sin());
            center + (Affine::rotate(rotation) * offset.to_point()).to_vec2()
        };
        self.path.line_to(t * at(start));
        if radii.x == 0.0 || radii.y == 0.0 {
            self.path.line_to(t * at(start + arc.sweep_angle));
            return true;
        }
        self.path
            .append(arc.append_iter(arc_tolerance(arc.radii)), t);
        true
    }

    pub fn arc_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, radius: f64) -> bool {
        if !finite(&[x1, y1, x2, y2, radius]) {
            return true;
        }
        if radius < 0.0 {
            return false;
        }
        let t = self.state.transform;
        let p1 = Point::new(x1, y1);
        let p2 = Point::new(x2, y2);
        let Some(current) = self.path.current else {
            self.path.move_to(t * p1);
            return true;
        };
        if t.determinant().abs() < 1e-12 {
            return true;
        }
        let p0 = t.inverse() * current;
        let v1 = p0 - p1;
        let v2 = p2 - p1;
        let cross = v1.x * v2.y - v1.y * v2.x;
        if p0 == p1 || p1 == p2 || radius == 0.0 || cross.abs() < 1e-9 {
            self.path.line_to(t * p1);
            return true;
        }
        let (n1, n2) = (v1.normalize(), v2.normalize());
        let angle = n1.dot(n2).clamp(-1.0, 1.0).acos();
        let dist = radius / (angle / 2.0).tan();
        let t1 = p1 + n1 * dist;
        let t2 = p1 + n2 * dist;
        let center = p1 + (n1 + n2).normalize() * (radius / (angle / 2.0).sin());
        let a0 = (t1 - center).atan2();
        let a1 = (t2 - center).atan2();
        let mut delta = a1 - a0;
        while delta > PI {
            delta -= TAU;
        }
        while delta < -PI {
            delta += TAU;
        }
        self.path.line_to(t * t1);
        let arc = Arc {
            center,
            radii: Vec2::new(radius, radius),
            start_angle: a0,
            sweep_angle: delta,
            x_rotation: 0.0,
        };
        self.path
            .append(arc.append_iter(arc_tolerance(arc.radii)), t);
        true
    }

    pub fn round_rect(&mut self, x: f64, y: f64, w: f64, h: f64, radii: &[Radii]) -> bool {
        if !finite(&[x, y, w, h]) || radii.iter().any(|r| !finite(&[r.x, r.y])) {
            return true;
        }
        if radii.is_empty() || radii.len() > 4 || radii.iter().any(|r| r.x < 0.0 || r.y < 0.0) {
            return false;
        }
        let pick = |i: usize| radii[i];
        let [mut tl, mut tr, mut br, mut bl] = match radii.len() {
            1 => [pick(0); 4],
            2 => [pick(0), pick(1), pick(0), pick(1)],
            3 => [pick(0), pick(1), pick(2), pick(1)],
            _ => [pick(0), pick(1), pick(2), pick(3)],
        };
        if w < 0.0 {
            std::mem::swap(&mut tl, &mut tr);
            std::mem::swap(&mut bl, &mut br);
        }
        if h < 0.0 {
            std::mem::swap(&mut tl, &mut bl);
            std::mem::swap(&mut tr, &mut br);
        }
        let (aw, ah) = (w.abs(), h.abs());
        let scale = [
            aw / (tl.x + tr.x),
            aw / (bl.x + br.x),
            ah / (tl.y + bl.y),
            ah / (tr.y + br.y),
        ]
        .into_iter()
        .filter(|s| s.is_finite())
        .fold(1.0f64, f64::min);
        let s = |r: Radii| Radii {
            x: r.x * scale,
            y: r.y * scale,
        };
        let (tl, tr, br, bl) = (s(tl), s(tr), s(br), s(bl));
        let (left, top) = (x.min(x + w), y.min(y + h));
        let (right, bottom) = (left + aw, top + ah);
        let mut shape = BezPath::new();
        shape.move_to((left + tl.x, top));
        shape.line_to((right - tr.x, top));
        corner(
            &mut shape,
            Point::new(right - tr.x, top + tr.y),
            tr,
            -FRAC_PI_2,
        );
        shape.line_to((right, bottom - br.y));
        corner(&mut shape, Point::new(right - br.x, bottom - br.y), br, 0.0);
        shape.line_to((left + bl.x, bottom));
        corner(
            &mut shape,
            Point::new(left + bl.x, bottom - bl.y),
            bl,
            FRAC_PI_2,
        );
        shape.line_to((left, top + tl.y));
        corner(&mut shape, Point::new(left + tl.x, top + tl.y), tl, PI);
        shape.close_path();
        let t = self.state.transform;
        self.path.append(shape, t);
        self.path.move_to(t * Point::new(x, y));
        true
    }
}

fn corner(path: &mut BezPath, center: Point, radii: Radii, start: f64) {
    if radii.x <= 0.0 || radii.y <= 0.0 {
        return;
    }
    let arc = Arc {
        center,
        radii: Vec2::new(radii.x, radii.y),
        start_angle: start,
        sweep_angle: FRAC_PI_2,
        x_rotation: 0.0,
    };
    path.extend(arc.append_iter(arc_tolerance(arc.radii)));
}
