use kurbo::{BezPath, PathEl, Point};

const TOLERANCE: f64 = 0.05;
const TEXT_SPREAD: f32 = 0.2;
const TEXT_GAMMA: f32 = 1.0 / 1.2;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}

#[derive(Clone, Debug)]
pub(crate) struct Mask {
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) w: usize,
    pub(crate) h: usize,
    pub(crate) cov: Vec<f32>,
}

impl Mask {
    pub(crate) fn at(&self, x: usize, y: usize) -> f32 {
        if x < self.x || y < self.y || x >= self.x + self.w || y >= self.y + self.h {
            return 0.0;
        }
        self.cov[(y - self.y) * self.w + (x - self.x)]
    }

    pub(crate) fn text_contrast(&self, width: usize) -> Mask {
        let x = self.x.saturating_sub(1);
        let x1 = (self.x + self.w + 1).min(width);
        let w = x1 - x;
        let mut cov = Vec::with_capacity(w * self.h);
        for row in self.y..self.y + self.h {
            for col in x..x1 {
                let left = col.checked_sub(1).map_or(0.0, |c| self.at(c, row));
                let spread = self.at(col, row) + TEXT_SPREAD * (left + self.at(col + 1, row));
                cov.push(spread.min(1.0).powf(TEXT_GAMMA));
            }
        }
        Mask {
            x,
            y: self.y,
            w,
            h: self.h,
            cov,
        }
    }
}

pub(crate) fn rasterize(
    path: &BezPath,
    rule: FillRule,
    width: usize,
    height: usize,
) -> Option<Mask> {
    if width == 0 || height == 0 {
        return None;
    }
    let lines = flatten_lines(path, tolerance_for(path, TOLERANCE)?);
    if lines.is_empty() {
        return None;
    }
    let mut min = Point::new(f64::INFINITY, f64::INFINITY);
    let mut max = Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for (a, b) in &lines {
        for p in [a, b] {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }
    }
    if !(min.x.is_finite() && min.y.is_finite() && max.x.is_finite() && max.y.is_finite()) {
        return None;
    }
    let x0 = min.x.floor().clamp(0.0, width as f64) as usize;
    let y0 = min.y.floor().clamp(0.0, height as f64) as usize;
    let x1 = max.x.ceil().clamp(0.0, width as f64) as usize;
    let y1 = max.y.ceil().clamp(0.0, height as f64) as usize;
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let w = x1 - x0;
    let h = y1 - y0;
    let mut acc = Accumulator::new(w, h);
    let origin = Point::new(x0 as f64, y0 as f64).to_vec2();
    for (a, b) in lines {
        acc.add_clipped(a - origin, b - origin);
    }
    Some(Mask {
        x: x0,
        y: y0,
        w,
        h,
        cov: acc.coverage(rule),
    })
}

pub(crate) fn tolerance_for(path: &BezPath, base: f64) -> Option<f64> {
    let bounds = path.control_box();
    let coords = [bounds.x0, bounds.y0, bounds.x1, bounds.y1];
    if !coords.iter().all(|c| c.is_finite()) {
        return None;
    }
    let extent = coords.iter().fold(0.0f64, |acc, c| acc.max(c.abs()));
    Some(base.max(extent * 1e-9))
}

fn flatten_lines(path: &BezPath, tolerance: f64) -> Vec<(Point, Point)> {
    let mut lines = Vec::new();
    let mut start = Point::ZERO;
    let mut current: Option<Point> = None;
    kurbo::flatten(path.iter(), tolerance, |el| match el {
        PathEl::MoveTo(p) => {
            if let Some(c) = current {
                lines.push((c, start));
            }
            start = p;
            current = Some(p);
        }
        PathEl::LineTo(p) => {
            if let Some(c) = current {
                lines.push((c, p));
            }
            current = Some(p);
        }
        PathEl::ClosePath => {
            if let Some(c) = current {
                lines.push((c, start));
            }
            current = Some(start);
        }
        PathEl::QuadTo(..) | PathEl::CurveTo(..) => {}
    });
    if let Some(c) = current {
        lines.push((c, start));
    }
    lines.retain(|(a, b)| a.y != b.y);
    lines
}

struct Accumulator {
    w: usize,
    h: usize,
    stride: usize,
    cells: Vec<f32>,
}

impl Accumulator {
    fn new(w: usize, h: usize) -> Self {
        let stride = w + 2;
        Self {
            w,
            h,
            stride,
            cells: vec![0.0; stride * h],
        }
    }

    fn add_clipped(&mut self, a: Point, b: Point) {
        let right = self.w as f64;
        let mut cuts = [0.0, 1.0, 1.0, 1.0];
        let mut count = 2;
        let dx = b.x - a.x;
        if dx != 0.0 {
            for edge in [0.0, right] {
                let t = (edge - a.x) / dx;
                if t > 0.0 && t < 1.0 {
                    cuts[count] = t;
                    count += 1;
                }
            }
        }
        let cuts = &mut cuts[..count];
        cuts.sort_by(f64::total_cmp);
        let clamp = |pt: Point| Point::new(pt.x.clamp(0.0, right), pt.y);
        for pair in cuts.windows(2) {
            self.line(clamp(a.lerp(b, pair[0])), clamp(a.lerp(b, pair[1])));
        }
    }

    fn line(&mut self, p0: Point, p1: Point) {
        if p0.y == p1.y {
            return;
        }
        let (dir, p0, p1) = if p0.y < p1.y {
            (1.0f32, p0, p1)
        } else {
            (-1.0f32, p1, p0)
        };
        let height = self.h as f64;
        if p1.y <= 0.0 || p0.y >= height {
            return;
        }
        let dxdy = (p1.x - p0.x) / (p1.y - p0.y);
        let top = p0.y.max(0.0);
        let limit = self.w as f64;
        let mut x = (p0.x + (top - p0.y) * dxdy).clamp(0.0, limit);
        let row_start = top.floor() as usize;
        let row_end = p1.y.ceil().min(height) as usize;
        for row in row_start..row_end {
            let dy = ((row + 1) as f64).min(p1.y) - (row as f64).max(p0.y);
            let xnext = (x + dxdy * dy).clamp(0.0, limit);
            self.span(row * self.stride, x, xnext, dy as f32 * dir);
            x = xnext;
        }
    }

    fn span(&mut self, line: usize, x: f64, xnext: f64, d: f32) {
        let (x0, x1) = if x < xnext { (x, xnext) } else { (xnext, x) };
        let x0floor = x0.floor();
        let x0i = x0floor as usize;
        let x1ceil = x1.ceil();
        let x1i = x1ceil as usize;
        let cells = &mut self.cells;
        if x1i <= x0i + 1 {
            let xmf = (0.5 * (x + xnext) - x0floor) as f32;
            cells[line + x0i] += d - d * xmf;
            cells[line + x0i + 1] += d * xmf;
            return;
        }
        let s = (1.0 / (x1 - x0)) as f32;
        let x0f = (x0 - x0floor) as f32;
        let a0 = 0.5 * s * (1.0 - x0f) * (1.0 - x0f);
        let x1f = (x1 - x1ceil + 1.0) as f32;
        let am = 0.5 * s * x1f * x1f;
        cells[line + x0i] += d * a0;
        if x1i == x0i + 2 {
            cells[line + x0i + 1] += d * (1.0 - a0 - am);
        } else {
            let a1 = s * (1.5 - x0f);
            cells[line + x0i + 1] += d * (a1 - a0);
            for xi in x0i + 2..x1i - 1 {
                cells[line + xi] += d * s;
            }
            let a2 = a1 + (x1i - x0i - 3) as f32 * s;
            cells[line + x1i - 1] += d * (1.0 - a2 - am);
        }
        cells[line + x1i] += d * am;
    }

    fn coverage(&self, rule: FillRule) -> Vec<f32> {
        let mut out = vec![0.0; self.w * self.h];
        for row in 0..self.h {
            let mut sum = 0.0f32;
            let cells = &self.cells[row * self.stride..row * self.stride + self.w];
            let dest = &mut out[row * self.w..(row + 1) * self.w];
            for (slot, cell) in dest.iter_mut().zip(cells) {
                sum += cell;
                *slot = coverage(sum, rule);
            }
        }
        out
    }
}

fn coverage(sum: f32, rule: FillRule) -> f32 {
    let a = sum.abs();
    match rule {
        FillRule::NonZero => a.min(1.0),
        FillRule::EvenOdd => {
            let m = a % 2.0;
            if m > 1.0 { 2.0 - m } else { m }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape;
    use std::f64::consts::{PI, TAU};

    fn total(mask: &Mask) -> f32 {
        mask.cov.iter().sum()
    }

    #[test]
    fn aligned_rect_has_exact_coverage() {
        let rect = kurbo::Rect::new(2.0, 3.0, 12.0, 8.0).to_path(0.1);
        let mask = rasterize(&rect, FillRule::NonZero, 20, 20).unwrap();
        assert_eq!((mask.x, mask.y, mask.w, mask.h), (2, 3, 10, 5));
        assert!(mask.cov.iter().all(|c| (*c - 1.0).abs() < 1e-5));
    }

    #[test]
    fn half_pixel_rect_has_half_coverage() {
        let rect = kurbo::Rect::new(0.5, 0.0, 4.5, 2.0).to_path(0.1);
        let mask = rasterize(&rect, FillRule::NonZero, 10, 10).unwrap();
        assert!((mask.at(0, 0) - 0.5).abs() < 1e-5);
        assert!((mask.at(2, 1) - 1.0).abs() < 1e-5);
        assert!((mask.at(4, 1) - 0.5).abs() < 1e-5);
        assert!((total(&mask) - 8.0).abs() < 1e-4);
    }

    #[test]
    fn circle_area_matches() {
        let circle = kurbo::Circle::new((50.0, 50.0), 20.0).to_path(0.01);
        let mask = rasterize(&circle, FillRule::NonZero, 100, 100).unwrap();
        let area = PI * 400.0;
        let got = f64::from(total(&mask));
        assert!(
            (got - area).abs() < TAU * 20.0 * TOLERANCE,
            "{got} vs {area}"
        );
    }

    #[test]
    fn offscreen_edges_still_fill() {
        let rect = kurbo::Rect::new(-10.0, -3.0, 15.0, 4.0).to_path(0.1);
        let mask = rasterize(&rect, FillRule::NonZero, 10, 10).unwrap();
        assert!((total(&mask) - 40.0).abs() < 1e-4);
    }

    #[test]
    fn even_odd_cuts_hole() {
        let mut path = kurbo::Rect::new(0.0, 0.0, 10.0, 10.0).to_path(0.1);
        path.extend(kurbo::Rect::new(2.0, 2.0, 8.0, 8.0).to_path(0.1));
        let mask = rasterize(&path, FillRule::EvenOdd, 10, 10).unwrap();
        assert!(mask.at(5, 5) < 1e-5);
        assert!((mask.at(1, 1) - 1.0).abs() < 1e-5);
        let mask = rasterize(&path, FillRule::NonZero, 10, 10).unwrap();
        assert!((mask.at(5, 5) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn text_contrast_spreads_stems_within_the_canvas() {
        let stem = kurbo::Rect::new(0.0, 0.0, 1.0, 4.0).to_path(0.1);
        let mask = rasterize(&stem, FillRule::NonZero, 3, 4).unwrap();
        let bold = mask.text_contrast(3);
        assert_eq!((bold.x, bold.w, bold.h), (0, 2, 4));
        assert!((bold.at(0, 2) - 1.0).abs() < 1e-5);
        assert!((bold.at(1, 2) - TEXT_SPREAD.powf(TEXT_GAMMA)).abs() < 1e-5);
        let half = rasterize(
            &kurbo::Rect::new(1.0, 0.0, 2.0, 1.0).to_path(0.1),
            FillRule::NonZero,
            3,
            1,
        )
        .unwrap()
        .text_contrast(3);
        assert_eq!((half.x, half.w), (0, 3));
        assert!(half.at(0, 0) > 0.2 && half.at(2, 0) > 0.2);
    }
}
