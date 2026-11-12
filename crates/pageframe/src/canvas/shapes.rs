use std::f64::consts::TAU;

use anyrender::PaintScene;
use kurbo::{Affine, Arc, Point, Vec2};
use peniko::Fill;

use super::{ARC_TOLERANCE, Canvas, MAX_DASHES, finite};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Turn {
    Clockwise,
    Counterclockwise,
}

fn sweep(start: f64, end: f64, turn: Turn) -> f64 {
    match turn {
        Turn::Clockwise if end - start >= TAU => TAU,
        Turn::Clockwise => (end - start).rem_euclid(TAU),
        Turn::Counterclockwise if start - end >= TAU => -TAU,
        Turn::Counterclockwise => -(start - end).rem_euclid(TAU),
    }
}

impl Canvas {
    pub fn arc(&mut self, center: (f64, f64), radius: f64, start: f64, end: f64) -> &mut Self {
        self.add_arc(
            center,
            (radius, radius),
            0.0,
            sweep_from(start, end, Turn::Clockwise),
        )
    }

    pub fn arc_counterclockwise(
        &mut self,
        center: (f64, f64),
        radius: f64,
        start: f64,
        end: f64,
    ) -> &mut Self {
        let span = sweep_from(start, end, Turn::Counterclockwise);
        self.add_arc(center, (radius, radius), 0.0, span)
    }

    pub fn ellipse(
        &mut self,
        center: (f64, f64),
        radii: (f64, f64),
        rotation: f64,
        start: f64,
        end: f64,
    ) -> &mut Self {
        self.add_arc(
            center,
            radii,
            rotation,
            sweep_from(start, end, Turn::Clockwise),
        )
    }

    fn add_arc(
        &mut self,
        center: (f64, f64),
        radii: (f64, f64),
        rotation: f64,
        span: Option<(f64, f64)>,
    ) -> &mut Self {
        let Some((start, sweep)) = span else {
            return self;
        };
        if !finite(&[center.0, center.1, radii.0, radii.1, rotation]) {
            return self;
        }
        if radii.0 < 0.0 || radii.1 < 0.0 {
            return self;
        }
        let local = Point::new(radii.0 * start.cos(), radii.1 * start.sin());
        let from =
            Affine::translate(Vec2::new(center.0, center.1)) * Affine::rotate(rotation) * local;
        self.line_to(from.x, from.y);
        let arc = Arc::new(center, radii, start, sweep, rotation);
        let transform = self.state.transform;
        for element in arc.append_iter(ARC_TOLERANCE) {
            self.path.push(transform * element);
        }
        self
    }

    pub fn set_line_dash(&mut self, pattern: &[f64]) -> &mut Self {
        let bad = |length: &f64| !length.is_finite() || *length < 0.0;
        if pattern.len() > MAX_DASHES || pattern.iter().any(bad) {
            return self;
        }
        self.state.dashes = if pattern.len() % 2 == 1 {
            [pattern, pattern].concat()
        } else {
            pattern.to_vec()
        };
        self
    }

    pub fn set_line_dash_offset(&mut self, offset: f64) -> &mut Self {
        if offset.is_finite() {
            self.state.dash_offset = offset;
        }
        self
    }

    pub fn fill_even_odd(&mut self) -> &mut Self {
        self.fill_path(Fill::EvenOdd)
    }

    pub fn clip_even_odd(&mut self) -> &mut Self {
        self.scene
            .push_clip_layer(Fill::EvenOdd, Affine::IDENTITY, &self.path);
        self.state.clips += 1;
        self
    }
}

fn sweep_from(start: f64, end: f64, turn: Turn) -> Option<(f64, f64)> {
    finite(&[start, end]).then(|| (start, sweep(start, end, turn)))
}
