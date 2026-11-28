use std::rc::Rc;

use kurbo::{Affine, BezPath, Rect, Shape, Stroke, StrokeOpts};
use parley::FontContext;

use super::Canvas2D;
use super::paint::{Paint, Source, Target};
use super::raster::{self, FillRule, Mask, rasterize};
use super::shadow::shadow_mask;
use super::text;

const STROKE_TOLERANCE: f64 = 0.05;
const MAX_DASHES: f64 = 1_000_000.0;

impl Canvas2D {
    fn size(&self) -> (usize, usize) {
        (self.width as usize, self.height as usize)
    }

    pub(crate) fn target(&mut self) -> Target<'_> {
        Target {
            pixels: &mut self.pixels,
            width: self.width as usize,
            clip: self.state.clip.as_deref().map(Vec::as_slice),
        }
    }

    pub(super) fn shadow_active(&self) -> bool {
        let s = &self.state;
        s.shadow_color.a > 0.0
            && (s.shadow_blur > 0.0 || s.shadow_offset_x != 0.0 || s.shadow_offset_y != 0.0)
    }

    pub(crate) fn draw_mask(&mut self, mask: &Mask, source: &Source<'_>) {
        let alpha = self.state.global_alpha as f32;
        let op = self.state.composite;
        if self.shadow_active() {
            let (w, h) = self.size();
            let offset = (self.state.shadow_offset_x, self.state.shadow_offset_y);
            if let Some(shadow) = shadow_mask(mask, self.state.shadow_blur, offset, w, h) {
                let color =
                    Source::Solid(self.state.shadow_color.premultiplied(source.alpha_hint()));
                self.target().composite(&shadow, &color, alpha, op);
            }
        }
        self.target().composite(mask, source, alpha, op);
        self.touch();
    }

    fn paint_path(&mut self, device: &BezPath, rule: FillRule, paint: &Paint) {
        let (w, h) = self.size();
        let Some(mask) = rasterize(device, rule, w, h) else {
            return;
        };
        let Some(source) = Source::from_paint(paint, self.state.transform) else {
            return;
        };
        self.draw_mask(&mask, &source);
    }

    fn stroke_outline(&self, device: &BezPath) -> Option<BezPath> {
        let t = self.state.transform;
        let det = t.determinant();
        if det.abs() < 1e-12 || device.is_empty() {
            return None;
        }
        let user = t.inverse() * device.clone();
        let s = &self.state;
        let mut style = Stroke::new(s.line_width)
            .with_join(s.line_join)
            .with_miter_limit(s.miter_limit)
            .with_caps(s.line_cap);
        let tolerance = raster::tolerance_for(&user, STROKE_TOLERANCE / det.abs().sqrt())?;
        let period = s.dash.iter().sum::<f64>();
        if period > 0.0 {
            let dashes = user.perimeter(tolerance) / period * s.dash.len() as f64;
            if dashes <= MAX_DASHES {
                style = style.with_dashes(s.dash_offset, s.dash.iter().copied());
            }
        }
        let outline = kurbo::stroke(user.iter(), &style, &StrokeOpts::default(), tolerance);
        Some(t * outline)
    }

    fn stroke_device(&mut self, device: &BezPath) {
        if let Some(outline) = self.stroke_outline(device) {
            let paint = self.state.stroke.clone();
            self.paint_path(&outline, FillRule::NonZero, &paint);
        }
    }

    pub fn fill(&mut self, rule: FillRule) {
        let path = self.path.bez.clone();
        let paint = self.state.fill.clone();
        self.paint_path(&path, rule, &paint);
    }

    pub fn stroke(&mut self) {
        let path = self.path.bez.clone();
        self.stroke_device(&path);
    }

    pub fn clip(&mut self, rule: FillRule) {
        let (w, h) = self.size();
        let mask = rasterize(&self.path.bez, rule, w, h);
        let mut clip = match &self.state.clip {
            Some(old) => old.as_ref().clone(),
            None => vec![1.0; w * h],
        };
        for y in 0..h {
            for x in 0..w {
                let cov = mask.as_ref().map_or(0.0, |m| m.at(x, y));
                clip[y * w + x] *= cov;
            }
        }
        self.state.clip = Some(Rc::new(clip));
    }

    fn rect_path(&self, x: f64, y: f64, w: f64, h: f64) -> Option<BezPath> {
        [x, y, w, h]
            .iter()
            .all(|v| v.is_finite())
            .then(|| self.state.transform * Rect::new(x, y, x + w, y + h).to_path(0.1))
    }

    pub fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        if let Some(path) = self.rect_path(x, y, w, h) {
            let paint = self.state.fill.clone();
            self.paint_path(&path, FillRule::NonZero, &paint);
        }
    }

    pub fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        if w == 0.0 && h == 0.0 {
            return;
        }
        if let Some(path) = self.rect_path(x, y, w, h) {
            self.stroke_device(&path);
        }
    }

    pub fn clear_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let (cw, ch) = self.size();
        let Some(path) = self.rect_path(x, y, w, h) else {
            return;
        };
        if let Some(mask) = rasterize(&path, FillRule::NonZero, cw, ch) {
            self.target().erase(&mask);
            self.touch();
        }
    }

    pub fn set_line_dash(&mut self, segments: &[f64]) -> bool {
        if segments.iter().any(|v| !v.is_finite() || *v < 0.0) {
            return false;
        }
        let mut dash = segments.to_vec();
        if dash.len() % 2 == 1 {
            dash.extend_from_slice(segments);
        }
        self.state.dash = dash.into();
        true
    }

    fn text_path(
        &self,
        fonts: &mut FontContext,
        text: &str,
        (x, y): (f64, f64),
        max_width: Option<f64>,
    ) -> Option<BezPath> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        if let Some(max) = max_width {
            if !max.is_finite() || max <= 0.0 {
                return None;
            }
        }
        let shaped = text::shape(fonts, &self.state.font, &text::prepare(text));
        if shaped.outline.is_empty() {
            return None;
        }
        let width = shaped.width;
        let squeeze = match max_width {
            Some(max) if width > max && width > 0.0 => max / width,
            _ => 1.0,
        };
        let dx = text::align_offset(self.state.text_align, width);
        let dy = shaped.metrics.baseline(self.state.text_baseline);
        let snap = baseline_snap(self.state.transform, x, y + dy);
        let place = Affine::translate((x, y + dy + snap))
            * Affine::scale_non_uniform(squeeze, 1.0)
            * Affine::translate((dx, 0.0));
        Some(self.state.transform * place * shaped.outline.clone())
    }

    pub fn fill_text(
        &mut self,
        fonts: &mut FontContext,
        text: &str,
        at: (f64, f64),
        max_width: Option<f64>,
    ) {
        let Some(path) = self.text_path(fonts, text, at, max_width) else {
            return;
        };
        let (w, h) = self.size();
        let Some(mask) = rasterize(&path, FillRule::NonZero, w, h) else {
            return;
        };
        let paint = self.state.fill.clone();
        let Some(source) = Source::from_paint(&paint, self.state.transform) else {
            return;
        };
        self.draw_mask(&mask.text_contrast(w), &source);
    }

    pub fn stroke_text(
        &mut self,
        fonts: &mut FontContext,
        text: &str,
        at: (f64, f64),
        max_width: Option<f64>,
    ) {
        if let Some(path) = self.text_path(fonts, text, at, max_width) {
            self.stroke_device(&path);
        }
    }
}

fn baseline_snap(transform: Affine, x: f64, y: f64) -> f64 {
    let [a, b, c, d, _, _] = transform.as_coeffs();
    if b != 0.0 || c != 0.0 || d == 0.0 || a == 0.0 {
        return 0.0;
    }
    let device = (transform * kurbo::Point::new(x, y)).y;
    (device.round() - device) / d
}
