mod gradient;
mod shapes;
mod text;

use std::sync::{Arc as Shared, Mutex};

use anyrender::{PaintScene, RenderContext, Scene};
use kurbo::{Affine, BezPath, Cap, Join, ParamCurveArclen, PathEl, Point, Rect, Stroke, Vec2};
use pageframe_dom::Widget;
use parley::{FontContext, LayoutContext};
use peniko::Fill;
use style::properties::ComputedValues;

use gradient::Ink;

pub use gradient::Gradient;
pub use text::{TextAlign, TextBaseline};

const DEFAULT_WIDTH: u32 = 300;
const DEFAULT_HEIGHT: u32 = 150;
const MITER_LIMIT: f64 = 10.0;
const ARC_TOLERANCE: f64 = 0.05;
const MAX_DASHES: usize = 1024;
const MAX_DASH_RUNS: f64 = 1_000_000.0;
const LENGTH_ACCURACY: f64 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn hex(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('#')?;
        if !digits.is_ascii() {
            return None;
        }
        let pair = |at: usize| u8::from_str_radix(digits.get(at..at + 2)?, 16).ok();
        let single = |at: usize| {
            let value = u8::from_str_radix(digits.get(at..=at)?, 16).ok()?;
            Some(value * 17)
        };
        match digits.len() {
            3 => Some(Self::rgb(single(0)?, single(1)?, single(2)?)),
            4 => Some(Self::rgba(single(0)?, single(1)?, single(2)?, single(3)?)),
            6 => Some(Self::rgb(pair(0)?, pair(2)?, pair(4)?)),
            8 => Some(Self::rgba(pair(0)?, pair(2)?, pair(4)?, pair(6)?)),
            _ => None,
        }
    }

    fn brush(self, alpha: f32) -> peniko::Color {
        peniko::Color::from_rgba8(self.r, self.g, self.b, self.a).multiply_alpha(alpha)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

#[derive(Clone)]
struct State {
    transform: Affine,
    fill: Ink,
    stroke: Ink,
    line_width: f64,
    line_cap: LineCap,
    line_join: LineJoin,
    dashes: Vec<f64>,
    dash_offset: f64,
    alpha: f32,
    clips: usize,
    font_family: String,
    font_size: f32,
    font_weight: f32,
    font_italic: bool,
    text_align: TextAlign,
    text_baseline: TextBaseline,
}

impl Default for State {
    fn default() -> Self {
        Self {
            transform: Affine::IDENTITY,
            fill: Ink::Solid(Color::BLACK),
            stroke: Ink::Solid(Color::BLACK),
            line_width: 1.0,
            line_cap: LineCap::default(),
            line_join: LineJoin::default(),
            dashes: Vec::new(),
            dash_offset: 0.0,
            alpha: 1.0,
            clips: 0,
            font_family: String::from("sans-serif"),
            font_size: 10.0,
            font_weight: 400.0,
            font_italic: false,
            text_align: TextAlign::default(),
            text_baseline: TextBaseline::default(),
        }
    }
}

pub struct Canvas {
    scene: Scene,
    width: u32,
    height: u32,
    state: State,
    saved: Vec<State>,
    path: BezPath,
    fonts: Shared<Mutex<FontContext>>,
    layouts: LayoutContext<()>,
}

impl Canvas {
    pub(crate) fn new(width: u32, height: u32, fonts: Shared<Mutex<FontContext>>) -> Self {
        Self {
            scene: Scene::new(),
            width,
            height,
            state: State::default(),
            saved: Vec::new(),
            path: BezPath::new(),
            fonts,
            layouts: LayoutContext::new(),
        }
    }

    pub(crate) fn size_from_attrs(width: Option<&str>, height: Option<&str>) -> (u32, u32) {
        let parse = |attr: Option<&str>, fallback: u32| {
            let Some(text) = attr else {
                return fallback;
            };
            let text = text.trim_start_matches(|c: char| c.is_ascii_whitespace());
            let text = text.strip_prefix('+').unwrap_or(text);
            let digits = text
                .find(|c: char| !c.is_ascii_digit())
                .map_or(text, |end| &text[..end]);
            digits.parse::<u32>().unwrap_or(fallback)
        };
        (parse(width, DEFAULT_WIDTH), parse(height, DEFAULT_HEIGHT))
    }

    pub(crate) fn into_widget(mut self) -> Box<dyn Widget> {
        for _ in 0..self.state.clips {
            self.scene.pop_layer();
        }
        Box::new(CanvasWidget {
            scene: self.scene,
            width: f64::from(self.width),
            height: f64::from(self.height),
        })
    }

    pub fn width(&self) -> f64 {
        f64::from(self.width)
    }

    pub fn height(&self) -> f64 {
        f64::from(self.height)
    }

    pub fn set_fill(&mut self, color: Color) -> &mut Self {
        self.state.fill = Ink::Solid(color);
        self
    }

    pub fn set_stroke(&mut self, color: Color) -> &mut Self {
        self.state.stroke = Ink::Solid(color);
        self
    }

    pub fn set_line_width(&mut self, width: f64) -> &mut Self {
        if width.is_finite() && width > 0.0 {
            self.state.line_width = width;
        }
        self
    }

    pub fn set_line_cap(&mut self, cap: LineCap) -> &mut Self {
        self.state.line_cap = cap;
        self
    }

    pub fn set_line_join(&mut self, join: LineJoin) -> &mut Self {
        self.state.line_join = join;
        self
    }

    pub fn set_alpha(&mut self, alpha: f32) -> &mut Self {
        if (0.0..=1.0).contains(&alpha) {
            self.state.alpha = alpha;
        }
        self
    }

    pub fn save(&mut self) -> &mut Self {
        self.saved.push(self.state.clone());
        self
    }

    pub fn restore(&mut self) -> &mut Self {
        if let Some(previous) = self.saved.pop() {
            for _ in previous.clips..self.state.clips {
                self.scene.pop_layer();
            }
            self.state = previous;
        }
        self
    }

    pub fn translate(&mut self, x: f64, y: f64) -> &mut Self {
        self.transform(Affine::translate(Vec2::new(x, y)))
    }

    pub fn scale(&mut self, x: f64, y: f64) -> &mut Self {
        self.transform(Affine::scale_non_uniform(x, y))
    }

    pub fn rotate(&mut self, radians: f64) -> &mut Self {
        self.transform(Affine::rotate(radians))
    }

    fn transform(&mut self, change: Affine) -> &mut Self {
        let next = self.state.transform * change;
        if next.as_coeffs().iter().all(|value| value.is_finite()) {
            self.state.transform = next;
        }
        self
    }

    pub fn reset_transform(&mut self) -> &mut Self {
        self.state.transform = Affine::IDENTITY;
        self
    }

    pub fn clear(&mut self) -> &mut Self {
        for _ in 0..self.state.clips {
            self.scene.pop_layer();
        }
        self.scene = Scene::new();
        self.state.clips = 0;
        for state in &mut self.saved {
            state.clips = 0;
        }
        self
    }

    pub fn fill_rect(&mut self, x: f64, y: f64, width: f64, height: f64) -> &mut Self {
        if !finite(&[x, y, width, height]) {
            return self;
        }
        let rect = Rect::new(x, y, x + width, y + height);
        let Some(brush) = self.state.fill.paint(self.state.alpha, Vec2::ZERO) else {
            return self;
        };
        self.scene
            .fill(Fill::NonZero, self.state.transform, &brush, None, &rect);
        self
    }

    pub fn stroke_rect(&mut self, x: f64, y: f64, width: f64, height: f64) -> &mut Self {
        if !finite(&[x, y, width, height]) {
            return self;
        }
        let rect = Rect::new(x, y, x + width, y + height);
        let Some(brush) = self.state.stroke.paint(self.state.alpha, Vec2::ZERO) else {
            return self;
        };
        let style = self.stroke_style(2.0 * (width.abs() + height.abs()));
        self.scene
            .stroke(&style, self.state.transform, &brush, None, &rect);
        self
    }

    pub fn begin_path(&mut self) -> &mut Self {
        self.path = BezPath::new();
        self
    }

    pub fn move_to(&mut self, x: f64, y: f64) -> &mut Self {
        if !finite(&[x, y]) {
            return self;
        }
        let point = self.place(x, y);
        self.path.move_to(point);
        self
    }

    pub fn line_to(&mut self, x: f64, y: f64) -> &mut Self {
        if !finite(&[x, y]) {
            return self;
        }
        let point = self.place(x, y);
        if self.path.elements().is_empty() {
            self.path.move_to(point);
        } else {
            self.path.line_to(point);
        }
        self
    }

    pub fn quad_to(&mut self, cx: f64, cy: f64, x: f64, y: f64) -> &mut Self {
        if !finite(&[cx, cy, x, y]) {
            return self;
        }
        let control = self.place(cx, cy);
        let end = self.place(x, y);
        if self.path.elements().is_empty() {
            self.path.move_to(control);
        }
        self.path.quad_to(control, end);
        self
    }

    pub fn curve_to(&mut self, c1: (f64, f64), c2: (f64, f64), end: (f64, f64)) -> &mut Self {
        if !finite(&[c1.0, c1.1, c2.0, c2.1, end.0, end.1]) {
            return self;
        }
        let first = self.place(c1.0, c1.1);
        let second = self.place(c2.0, c2.1);
        let end = self.place(end.0, end.1);
        if self.path.elements().is_empty() {
            self.path.move_to(first);
        }
        self.path.curve_to(first, second, end);
        self
    }

    pub fn rect(&mut self, x: f64, y: f64, width: f64, height: f64) -> &mut Self {
        if !finite(&[x, y, width, height]) {
            return self;
        }
        self.move_to(x, y)
            .line_to(x + width, y)
            .line_to(x + width, y + height)
            .line_to(x, y + height)
            .close_path()
    }

    pub fn close_path(&mut self) -> &mut Self {
        if !matches!(self.path.elements().last(), None | Some(PathEl::ClosePath)) {
            self.path.close_path();
        }
        self
    }

    pub fn fill(&mut self) -> &mut Self {
        self.fill_path(Fill::NonZero)
    }

    fn fill_path(&mut self, rule: Fill) -> &mut Self {
        let transform = self.state.transform;
        if transform.determinant().abs() < f64::EPSILON {
            return self;
        }
        let Some(brush) = self.state.fill.paint(self.state.alpha, Vec2::ZERO) else {
            return self;
        };
        self.scene
            .fill(rule, Affine::IDENTITY, &brush, Some(transform), &self.path);
        self
    }

    pub fn stroke(&mut self) -> &mut Self {
        let transform = self.state.transform;
        if transform.determinant().abs() < f64::EPSILON {
            return self;
        }
        let local = transform.inverse() * self.path.clone();
        let Some(brush) = self.state.stroke.paint(self.state.alpha, Vec2::ZERO) else {
            return self;
        };
        let length = local
            .segments()
            .map(|segment| segment.arclen(LENGTH_ACCURACY))
            .sum();
        let style = self.stroke_style(length);
        self.scene.stroke(&style, transform, &brush, None, &local);
        self
    }

    pub fn clip(&mut self) -> &mut Self {
        self.scene
            .push_clip_layer(Fill::NonZero, Affine::IDENTITY, &self.path);
        self.state.clips += 1;
        self
    }

    fn place(&self, x: f64, y: f64) -> Point {
        self.state.transform * Point::new(x, y)
    }

    fn stroke_style(&self, length: f64) -> Stroke {
        let cap = match self.state.line_cap {
            LineCap::Butt => Cap::Butt,
            LineCap::Round => Cap::Round,
            LineCap::Square => Cap::Square,
        };
        let join = match self.state.line_join {
            LineJoin::Miter => Join::Miter,
            LineJoin::Round => Join::Round,
            LineJoin::Bevel => Join::Bevel,
        };
        let style = Stroke::new(self.state.line_width)
            .with_caps(cap)
            .with_join(join)
            .with_miter_limit(MITER_LIMIT);
        let period = self.state.dashes.iter().sum::<f64>();
        let runs = length / period * 2.0;
        if period > 0.0 && runs <= MAX_DASH_RUNS {
            let offset = self.state.dash_offset.rem_euclid(period);
            style.with_dashes(offset, &self.state.dashes)
        } else {
            style
        }
    }
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

struct CanvasWidget {
    scene: Scene,
    width: f64,
    height: f64,
}

impl Widget for CanvasWidget {
    fn paint(
        &mut self,
        _render_ctx: &mut dyn RenderContext,
        _styles: &ComputedValues,
        width: u32,
        height: u32,
        _scale: f64,
    ) -> Scene {
        let mut out = Scene::new();
        if self.width <= 0.0 || self.height <= 0.0 {
            return out;
        }
        let fit = Affine::scale_non_uniform(
            f64::from(width) / self.width,
            f64::from(height) / self.height,
        );
        let bounds = Rect::new(0.0, 0.0, self.width, self.height);
        out.push_clip_layer(Fill::NonZero, fit, &bounds);
        out.append_scene(self.scene.clone(), fit);
        out.pop_layer();
        out
    }
}
