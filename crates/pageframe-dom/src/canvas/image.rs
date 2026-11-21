use kurbo::{Affine, Point, Rect, Shape};

use super::Canvas2D;
use super::paint::Source;
use super::raster::{FillRule, rasterize};

#[derive(Clone, Copy, Debug)]
pub struct ImageSource<'a> {
    pub width: u32,
    pub height: u32,
    pub premultiplied: &'a [u8],
}

impl ImageSource<'_> {
    pub fn premultiply(straight: &[u8]) -> Vec<u8> {
        let mut out = straight.to_vec();
        for px in out.as_chunks_mut::<4>().0 {
            let a = u32::from(px[3]);
            if a != 255 {
                for c in &mut px[..3] {
                    *c = ((u32::from(*c) * a + 127) / 255) as u8;
                }
            }
        }
        out
    }

    fn texel(&self, x: i64, y: i64, bounds: (i64, i64, i64, i64)) -> [f32; 4] {
        let x = x.clamp(bounds.0, bounds.2 - 1) as usize;
        let y = y.clamp(bounds.1, bounds.3 - 1) as usize;
        let i = (y * self.width as usize + x) * 4;
        std::array::from_fn(|c| f32::from(self.premultiplied[i + c]) / 255.0)
    }

    fn sample(&self, p: Point, bounds: (i64, i64, i64, i64), smooth: bool) -> [f32; 4] {
        if !smooth {
            return self.texel(p.x.floor() as i64, p.y.floor() as i64, bounds);
        }
        let (fx, fy) = (p.x - 0.5, p.y - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = ((fx - x0) as f32, (fy - y0) as f32);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let a = self.texel(x0, y0, bounds);
        let b = self.texel(x0 + 1, y0, bounds);
        let c = self.texel(x0, y0 + 1, bounds);
        let d = self.texel(x0 + 1, y0 + 1, bounds);
        std::array::from_fn(|i| {
            let top = a[i] + (b[i] - a[i]) * tx;
            let bottom = c[i] + (d[i] - c[i]) * tx;
            top + (bottom - top) * ty
        })
    }
}

fn overlap(start: i64, len: u32, size: u32) -> std::ops::Range<i64> {
    clip_span(0..i64::from(len), start, size)
}

fn clip_span(span: std::ops::Range<i64>, offset: i64, size: u32) -> std::ops::Range<i64> {
    let lo = span.start.max(0i64.saturating_sub(offset));
    let hi = span.end.min(i64::from(size).saturating_sub(offset));
    lo..hi.max(lo)
}

fn normalize(r: Rect) -> Rect {
    Rect::new(
        r.x0.min(r.x1),
        r.y0.min(r.y1),
        r.x0.max(r.x1),
        r.y0.max(r.y1),
    )
}

impl Canvas2D {
    pub fn draw_image(&mut self, image: &ImageSource<'_>, src: Rect, dst: Rect) {
        let values = [
            src.x0, src.y0, src.x1, src.y1, dst.x0, dst.y0, dst.x1, dst.y1,
        ];
        if values.iter().any(|v| !v.is_finite()) || image.width == 0 || image.height == 0 {
            return;
        }
        let (src, dst) = (normalize(src), normalize(dst));
        if src.width() == 0.0 || src.height() == 0.0 || dst.width() == 0.0 || dst.height() == 0.0 {
            return;
        }
        let bounds = Rect::new(0.0, 0.0, f64::from(image.width), f64::from(image.height));
        let visible = src.intersect(bounds);
        if visible.width() <= 0.0 || visible.height() <= 0.0 {
            return;
        }
        let (sx, sy) = (dst.width() / src.width(), dst.height() / src.height());
        let dst = Rect::new(
            dst.x0 + (visible.x0 - src.x0) * sx,
            dst.y0 + (visible.y0 - src.y0) * sy,
            dst.x0 + (visible.x1 - src.x0) * sx,
            dst.y0 + (visible.y1 - src.y0) * sy,
        );
        let map = self.state.transform
            * Affine::translate((dst.x0, dst.y0))
            * Affine::scale_non_uniform(sx, sy)
            * Affine::translate((-visible.x0, -visible.y0));
        if map.determinant().abs() < 1e-12 {
            return;
        }
        let inverse = map.inverse();
        let texels = (
            visible.x0.floor() as i64,
            visible.y0.floor() as i64,
            visible.x1.ceil() as i64,
            visible.y1.ceil() as i64,
        );
        let device = self.state.transform * dst.to_path(0.1);
        let (w, h) = (self.width as usize, self.height as usize);
        let Some(mask) = rasterize(&device, FillRule::NonZero, w, h) else {
            return;
        };
        let smooth = self.state.image_smoothing;
        let sampler = |p: Point| image.sample(inverse * p, texels, smooth);
        self.draw_mask(&mask, &Source::Image(&sampler));
    }

    pub fn get_image_data(&self, x: i64, y: i64, w: u32, h: u32) -> Vec<u8> {
        let mut out = vec![0u8; w as usize * h as usize * 4];
        let rows = overlap(y, h, self.height);
        let cols = overlap(x, w, self.width);
        for row in rows.clone() {
            let sy = y + row;
            for col in cols.clone() {
                let sx = x + col;
                let i = (sy as usize * self.width as usize + sx as usize) * 4;
                let o = (row as usize * w as usize + col as usize) * 4;
                let px = &self.pixels[i..i + 4];
                let a = u32::from(px[3]);
                out[o + 3] = px[3];
                for c in 0..3 {
                    if let Some(v) = (u32::from(px[c]) * 255 + a / 2).checked_div(a) {
                        out[o + c] = v.min(255) as u8;
                    }
                }
            }
        }
        out
    }

    pub fn put_image_data(
        &mut self,
        data: &[u8],
        (w, h): (u32, u32),
        (dx, dy): (i64, i64),
        dirty: Rect,
    ) {
        let dirty = normalize(dirty).intersect(Rect::new(0.0, 0.0, f64::from(w), f64::from(h)));
        let (x0, y0) = (
            dirty.x0.floor().max(0.0) as i64,
            dirty.y0.floor().max(0.0) as i64,
        );
        let (x1, y1) = (dirty.x1.ceil() as i64, dirty.y1.ceil() as i64);
        let rows = clip_span(y0..y1, dy, self.height);
        let cols = clip_span(x0..x1, dx, self.width);
        for row in rows {
            let ty = dy + row;
            for col in cols.clone() {
                let tx = dx + col;
                let s = (row as usize * w as usize + col as usize) * 4;
                let t = (ty as usize * self.width as usize + tx as usize) * 4;
                let Some(px) = data.get(s..s + 4) else {
                    continue;
                };
                let a = u32::from(px[3]);
                for c in 0..3 {
                    self.pixels[t + c] = ((u32::from(px[c]) * a + 127) / 255) as u8;
                }
                self.pixels[t + 3] = px[3];
            }
        }
        self.touch();
    }
}
