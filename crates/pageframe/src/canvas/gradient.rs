use anyrender::Paint;
use kurbo::{Point, Vec2};
use peniko::{ColorStop, InterpolationAlphaSpace};

use super::{Canvas, Color, finite};

const MAX_STOPS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Shape {
    Linear {
        from: Point,
        to: Point,
    },
    Radial {
        start: Point,
        start_radius: f64,
        end: Point,
        end_radius: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    shape: Option<Shape>,
    stops: Vec<(f32, Color)>,
}

impl Gradient {
    pub fn linear(from: (f64, f64), to: (f64, f64)) -> Self {
        let valid = finite(&[from.0, from.1, to.0, to.1]) && from != to;
        let shape = valid.then(|| Shape::Linear {
            from: Point::from(from),
            to: Point::from(to),
        });
        Self {
            shape,
            stops: Vec::new(),
        }
    }

    pub fn radial(start: (f64, f64), start_radius: f64, end: (f64, f64), end_radius: f64) -> Self {
        let valid = finite(&[start.0, start.1, start_radius, end.0, end.1, end_radius])
            && start_radius >= 0.0
            && end_radius >= 0.0
            && (start != end || start_radius != end_radius);
        let shape = valid.then(|| Shape::Radial {
            start: Point::from(start),
            start_radius,
            end: Point::from(end),
            end_radius,
        });
        Self {
            shape,
            stops: Vec::new(),
        }
    }

    #[must_use]
    pub fn stop(mut self, offset: f32, color: Color) -> Self {
        if (0.0..=1.0).contains(&offset) && self.stops.len() < MAX_STOPS {
            let at = self.stops.partition_point(|(other, _)| *other <= offset);
            self.stops.insert(at, (offset, color));
        }
        self
    }

    fn paint(&self, alpha: f32, shift: Vec2) -> Option<Paint> {
        let shape = self.shape?;
        let (_, first) = *self.stops.first()?;
        if self.stops.len() == 1 {
            return Some(Paint::Solid(first.brush(alpha)));
        }
        let gradient = match shape {
            Shape::Linear { from, to } => peniko::Gradient::new_linear(from - shift, to - shift),
            Shape::Radial {
                start,
                start_radius,
                end,
                end_radius,
            } => peniko::Gradient::new_two_point_radial(
                start - shift,
                narrow(start_radius),
                end - shift,
                narrow(end_radius),
            ),
        };
        let stops: Vec<ColorStop> = self
            .stops
            .iter()
            .map(|&(offset, color)| ColorStop::from((offset, color.brush(alpha))))
            .collect();
        let mut gradient = gradient.with_stops(stops.as_slice());
        gradient.interpolation_alpha_space = InterpolationAlphaSpace::Unpremultiplied;
        Some(Paint::Gradient(gradient))
    }
}

fn narrow(value: f64) -> f32 {
    num_traits::ToPrimitive::to_f32(&value).unwrap_or(f32::MAX)
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Ink {
    Solid(Color),
    Gradient(Gradient),
}

impl Ink {
    pub(super) fn paint(&self, alpha: f32, shift: Vec2) -> Option<Paint> {
        match self {
            Self::Solid(color) => Some(Paint::Solid(color.brush(alpha))),
            Self::Gradient(gradient) => gradient.paint(alpha, shift),
        }
    }
}

impl Canvas {
    pub fn set_fill_gradient(&mut self, gradient: &Gradient) -> &mut Self {
        self.state.fill = Ink::Gradient(gradient.clone());
        self
    }

    pub fn set_stroke_gradient(&mut self, gradient: &Gradient) -> &mut Self {
        self.state.stroke = Ink::Gradient(gradient.clone());
        self
    }
}
