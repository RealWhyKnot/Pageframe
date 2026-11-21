use std::cell::RefCell;
use std::f64::consts::TAU;
use std::rc::Rc;

use kurbo::{Affine, Point};

use super::color::Rgba;
use super::raster::Mask;

const RAMP: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GradientKind {
    Linear {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
    },
    Radial {
        x0: f64,
        y0: f64,
        r0: f64,
        x1: f64,
        y1: f64,
        r1: f64,
    },
    Conic {
        angle: f64,
        x: f64,
        y: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    pub kind: GradientKind,
    stops: Vec<(f32, Rgba)>,
}

impl Gradient {
    pub fn new(kind: GradientKind) -> Self {
        Self {
            kind,
            stops: Vec::new(),
        }
    }

    pub fn add_stop(&mut self, offset: f64, color: Rgba) {
        let offset = offset.clamp(0.0, 1.0) as f32;
        let index = self.stops.partition_point(|(o, _)| *o <= offset);
        self.stops.insert(index, (offset, color));
    }

    fn ramp(&self) -> Option<Box<[[f32; 4]]>> {
        let first = self.stops.first()?;
        let last = self.stops.last()?;
        let mut lut = vec![[0.0; 4]; RAMP].into_boxed_slice();
        for (i, slot) in lut.iter_mut().enumerate() {
            let t = i as f32 / (RAMP - 1) as f32;
            let color = if t <= first.0 {
                first.1
            } else if t >= last.0 {
                last.1
            } else {
                let next = self.stops.partition_point(|(o, _)| *o <= t);
                let (o0, c0) = self.stops[next - 1];
                let (o1, c1) = self.stops[next];
                let f = if o1 > o0 { (t - o0) / (o1 - o0) } else { 1.0 };
                let mix = |a: f32, b: f32| a + (b - a) * f;
                Rgba {
                    r: mix(c0.r, c1.r),
                    g: mix(c0.g, c1.g),
                    b: mix(c0.b, c1.b),
                    a: mix(c0.a, c1.a),
                }
            };
            *slot = color.premultiplied(1.0);
        }
        Some(lut)
    }
}

#[derive(Clone, Debug)]
pub enum Paint {
    Color(Rgba),
    Gradient(Rc<RefCell<Gradient>>),
}

impl Default for Paint {
    fn default() -> Self {
        Self::Color(Rgba::BLACK)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CompositeOp {
    #[default]
    SourceOver,
    SourceAtop,
    DestinationOver,
    DestinationOut,
    Lighter,
    Xor,
}

impl CompositeOp {
    pub const NAMES: [&'static str; 26] = [
        "source-over",
        "source-in",
        "source-out",
        "source-atop",
        "destination-over",
        "destination-in",
        "destination-out",
        "destination-atop",
        "lighter",
        "copy",
        "xor",
        "multiply",
        "screen",
        "overlay",
        "darken",
        "lighten",
        "color-dodge",
        "color-burn",
        "hard-light",
        "soft-light",
        "difference",
        "exclusion",
        "hue",
        "saturation",
        "color",
        "luminosity",
    ];

    pub fn from_name(name: &str) -> Self {
        match name {
            "source-atop" => Self::SourceAtop,
            "destination-over" => Self::DestinationOver,
            "destination-out" => Self::DestinationOut,
            "lighter" => Self::Lighter,
            "xor" => Self::Xor,
            _ => Self::SourceOver,
        }
    }

    fn blend(self, s: [f32; 4], d: [f32; 4]) -> [f32; 4] {
        let (sa, da) = (s[3], d[3]);
        let mix = |fs: f32, fd: f32| std::array::from_fn(|i| s[i] * fs + d[i] * fd);
        match self {
            Self::SourceOver => mix(1.0, 1.0 - sa),
            Self::SourceAtop => mix(da, 1.0 - sa),
            Self::DestinationOver => mix(1.0 - da, 1.0),
            Self::DestinationOut => mix(0.0, 1.0 - sa),
            Self::Lighter => std::array::from_fn(|i| (s[i] + d[i]).min(1.0)),
            Self::Xor => mix(1.0 - da, 1.0 - sa),
        }
    }
}

pub(crate) enum Source<'a> {
    Solid([f32; 4]),
    Ramp {
        lut: Box<[[f32; 4]]>,
        inverse: Affine,
        kind: GradientKind,
    },
    Image(&'a dyn Fn(Point) -> [f32; 4]),
}

impl Source<'_> {
    pub(crate) fn from_paint(paint: &Paint, transform: Affine) -> Option<Self> {
        match paint {
            Paint::Color(color) => Some(Source::Solid(color.premultiplied(1.0))),
            Paint::Gradient(gradient) => {
                let gradient = gradient.borrow();
                if transform.determinant().abs() < 1e-12 {
                    return None;
                }
                Some(Source::Ramp {
                    lut: gradient.ramp()?,
                    inverse: transform.inverse(),
                    kind: gradient.kind,
                })
            }
        }
    }

    pub(crate) fn alpha_hint(&self) -> f32 {
        match self {
            Source::Solid(color) => color[3],
            Source::Ramp { .. } | Source::Image(_) => 1.0,
        }
    }

    fn sample(&self, x: f64, y: f64) -> [f32; 4] {
        match self {
            Source::Solid(color) => *color,
            Source::Image(sample) => sample(Point::new(x, y)),
            Source::Ramp { lut, inverse, kind } => {
                let p = *inverse * Point::new(x, y);
                match ramp_position(*kind, p) {
                    Some(t) => lut[(t.clamp(0.0, 1.0) * (RAMP - 1) as f64).round() as usize],
                    None => [0.0; 4],
                }
            }
        }
    }
}

fn ramp_position(kind: GradientKind, p: Point) -> Option<f64> {
    match kind {
        GradientKind::Linear { x0, y0, x1, y1 } => {
            let (dx, dy) = (x1 - x0, y1 - y0);
            let len = dx * dx + dy * dy;
            if len == 0.0 {
                return None;
            }
            Some(((p.x - x0) * dx + (p.y - y0) * dy) / len)
        }
        GradientKind::Conic { angle, x, y } => {
            let a = (p.y - y).atan2(p.x - x) - angle;
            Some(a.rem_euclid(TAU) / TAU)
        }
        GradientKind::Radial {
            x0,
            y0,
            r0,
            x1,
            y1,
            r1,
        } => {
            if x0 == x1 && y0 == y1 && r0 == r1 {
                return None;
            }
            let (cdx, cdy, dr) = (x1 - x0, y1 - y0, r1 - r0);
            let (pdx, pdy) = (p.x - x0, p.y - y0);
            let a = cdx * cdx + cdy * cdy - dr * dr;
            let b = pdx * cdx + pdy * cdy + r0 * dr;
            let c = pdx * pdx + pdy * pdy - r0 * r0;
            let valid = |w: f64| r0 + w * dr >= 0.0;
            if a.abs() < 1e-9 {
                if b == 0.0 {
                    return None;
                }
                let w = c / (2.0 * b);
                return valid(w).then_some(w);
            }
            let disc = b * b - a * c;
            if disc < 0.0 {
                return None;
            }
            let root = disc.sqrt();
            let (w1, w2) = ((b + root) / a, (b - root) / a);
            let (hi, lo) = if w1 > w2 { (w1, w2) } else { (w2, w1) };
            if valid(hi) {
                Some(hi)
            } else if valid(lo) {
                Some(lo)
            } else {
                None
            }
        }
    }
}

pub(crate) struct Target<'a> {
    pub(crate) pixels: &'a mut [u8],
    pub(crate) width: usize,
    pub(crate) clip: Option<&'a [f32]>,
}

impl Target<'_> {
    pub(crate) fn composite(
        &mut self,
        mask: &Mask,
        source: &Source<'_>,
        alpha: f32,
        op: CompositeOp,
    ) {
        for row in 0..mask.h {
            let y = mask.y + row;
            for col in 0..mask.w {
                let x = mask.x + col;
                let mut cov = mask.cov[row * mask.w + col];
                if let Some(clip) = self.clip {
                    cov *= clip[y * self.width + x];
                }
                if cov <= 0.0 {
                    continue;
                }
                let k = cov * alpha;
                let s = source.sample(x as f64 + 0.5, y as f64 + 0.5).map(|c| c * k);
                let i = (y * self.width + x) * 4;
                let px = &mut self.pixels[i..i + 4];
                let d: [f32; 4] = std::array::from_fn(|c| f32::from(px[c]) / 255.0);
                let out = op.blend(s, d);
                for (slot, value) in px.iter_mut().zip(out) {
                    *slot = (value.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                }
            }
        }
    }

    pub(crate) fn erase(&mut self, mask: &Mask) {
        for row in 0..mask.h {
            let y = mask.y + row;
            for col in 0..mask.w {
                let x = mask.x + col;
                let mut cov = mask.cov[row * mask.w + col];
                if let Some(clip) = self.clip {
                    cov *= clip[y * self.width + x];
                }
                if cov <= 0.0 {
                    continue;
                }
                let i = (y * self.width + x) * 4;
                for slot in &mut self.pixels[i..i + 4] {
                    *slot = (f32::from(*slot) * (1.0 - cov.min(1.0)) + 0.5) as u8;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_with_equal_offsets_keep_insertion_order() {
        let mut gradient = Gradient::new(GradientKind::Linear {
            x0: 0.0,
            y0: 0.0,
            x1: 10.0,
            y1: 0.0,
        });
        gradient.add_stop(0.5, Rgba::BLACK);
        gradient.add_stop(0.5, Rgba::TRANSPARENT);
        gradient.add_stop(0.0, Rgba::BLACK);
        assert_eq!(gradient.stops.len(), 3);
        assert_eq!(gradient.stops[1].1, Rgba::BLACK);
        assert_eq!(gradient.stops[2].1, Rgba::TRANSPARENT);
    }

    #[test]
    fn radial_gradient_centers_at_zero() {
        let kind = GradientKind::Radial {
            x0: 0.0,
            y0: 0.0,
            r0: 0.0,
            x1: 0.0,
            y1: 0.0,
            r1: 10.0,
        };
        let t = ramp_position(kind, Point::new(5.0, 0.0)).unwrap();
        assert!((t - 0.5).abs() < 1e-9);
    }
}
