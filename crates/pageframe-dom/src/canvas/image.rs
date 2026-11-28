use kurbo::{Affine, Point, Rect, Shape};

use super::Canvas2D;
use super::paint::{CompositeOp, Source};
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

const SAMPLE_ONE: u32 = 256;

struct LinearSample {
    first: usize,
    second: usize,
    fraction: u32,
}

fn linear_samples(
    pixels: std::ops::Range<usize>,
    scale: f64,
    offset: f64,
    bounds: std::ops::Range<i64>,
) -> Vec<LinearSample> {
    pixels
        .map(|pixel| {
            let source = scale * (pixel as f64 + 0.5) + offset - 0.5;
            let first = source.floor();
            let fraction = ((source - first) * f64::from(SAMPLE_ONE)).round() as u32;
            let first = first as i64;
            LinearSample {
                first: first.clamp(bounds.start, bounds.end - 1) as usize,
                second: first.saturating_add(1).clamp(bounds.start, bounds.end - 1) as usize,
                fraction,
            }
        })
        .collect()
}

fn nearest_samples(
    pixels: std::ops::Range<usize>,
    scale: f64,
    offset: f64,
    bounds: std::ops::Range<i64>,
) -> Vec<usize> {
    pixels
        .map(|pixel| {
            let source = scale * (pixel as f64 + 0.5) + offset;
            (source.floor() as i64).clamp(bounds.start, bounds.end - 1) as usize
        })
        .collect()
}

fn downsample_half_row(top: &[u8], bottom: &[u8], destination: &mut [u8]) {
    const CHANNELS: u64 = 0x00ff_00ff_00ff_00ff;
    const CHANNEL_SUMS: u64 = 0x01ff_01ff;
    const ROUND: u64 = 0x0002_0002_0002_0002;
    for ((top, bottom), pixel) in top
        .as_chunks::<8>()
        .0
        .iter()
        .zip(bottom.as_chunks::<8>().0)
        .zip(destination.as_chunks_mut::<4>().0)
    {
        let top = u64::from_le_bytes(*top);
        let bottom = u64::from_le_bytes(*bottom);
        let even = (top & CHANNELS) + (bottom & CHANNELS);
        let odd = ((top >> 8) & CHANNELS) + ((bottom >> 8) & CHANNELS);
        let even = ((even + (even >> 32 & CHANNEL_SUMS) + ROUND) >> 2) & CHANNELS;
        let odd = ((odd + (odd >> 32 & CHANNEL_SUMS) + ROUND) >> 2) & CHANNELS;
        let rgba = (even | (odd << 8)).to_le_bytes();
        *pixel = [rgba[0], rgba[1], rgba[2], rgba[3]];
    }
}

impl Canvas2D {
    pub fn draw_image(&mut self, image: &ImageSource<'_>, src: Rect, dst: Rect) {
        self.draw_image_inner(image, src, dst, false);
    }

    pub fn draw_opaque_image(&mut self, image: &ImageSource<'_>, src: Rect, dst: Rect) {
        self.draw_image_inner(image, src, dst, true);
    }

    fn draw_image_inner(&mut self, image: &ImageSource<'_>, src: Rect, dst: Rect, opaque: bool) {
        let values = [
            src.x0, src.y0, src.x1, src.y1, dst.x0, dst.y0, dst.x1, dst.y1,
        ];
        if values.iter().any(|v| !v.is_finite()) || image.width == 0 || image.height == 0 {
            return;
        }
        let byte_count = u64::from(image.width) * u64::from(image.height);
        if usize::try_from(byte_count).map_or(true, |pixels| pixels > image.premultiplied.len() / 4)
        {
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
        let [a, b, c, d, x, y] = map.as_coeffs();
        let can_replace_pixels = opaque
            && b == 0.0
            && c == 0.0
            && self.state.global_alpha == 1.0
            && self.state.composite == CompositeOp::SourceOver
            && self.state.clip.is_none()
            && !self.shadow_active();
        if can_replace_pixels
            && [a, d] == [1.0, 1.0]
            && [x, y, visible.x0, visible.y0, visible.x1, visible.y1]
                .iter()
                .all(|coordinate| coordinate.fract() == 0.0)
        {
            self.copy_image_rows(image, visible, x as i64, y as i64);
            return;
        }
        let inverse = map.inverse();
        let texels = (
            visible.x0.floor() as i64,
            visible.y0.floor() as i64,
            visible.x1.ceil() as i64,
            visible.y1.ceil() as i64,
        );
        if can_replace_pixels {
            let device = normalize(Rect::from_points(
                self.state.transform * Point::new(dst.x0, dst.y0),
                self.state.transform * Point::new(dst.x1, dst.y1),
            ));
            if [device.x0, device.y0, device.x1, device.y1]
                .iter()
                .all(|coordinate| coordinate.fract() == 0.0)
                && inverse.as_coeffs().iter().all(|value| value.is_finite())
            {
                let half_size = [a, d] == [0.5, 0.5]
                    && [x, y, visible.x0, visible.y0, visible.x1, visible.y1]
                        .iter()
                        .all(|coordinate| coordinate.fract() == 0.0)
                    && device == map.transform_rect_bbox(visible);
                self.scale_opaque_image(image, device, inverse, texels, half_size);
                return;
            }
        }
        let device = self.state.transform * dst.to_path(0.1);
        let (w, h) = (self.width as usize, self.height as usize);
        let Some(mask) = rasterize(&device, FillRule::NonZero, w, h) else {
            return;
        };
        let smooth = self.state.image_smoothing;
        let sampler = |p: Point| image.sample(inverse * p, texels, smooth);
        self.draw_mask(&mask, &Source::Image(&sampler));
    }

    fn scale_opaque_image(
        &mut self,
        image: &ImageSource<'_>,
        device: Rect,
        inverse: Affine,
        texels: (i64, i64, i64, i64),
        half_size: bool,
    ) {
        let device = device.intersect(Rect::new(
            0.0,
            0.0,
            f64::from(self.width),
            f64::from(self.height),
        ));
        if device.width() <= 0.0 || device.height() <= 0.0 {
            return;
        }
        let columns = device.x0 as usize..device.x1 as usize;
        let rows = device.y0 as usize..device.y1 as usize;
        let [x_scale, _, _, y_scale, x_offset, y_offset] = inverse.as_coeffs();
        let source_stride = image.width as usize * 4;
        let stride = self.width as usize * 4;
        let row_start = columns.start * 4;
        let row_end = columns.end * 4;
        let destination = self.pixels[rows.start * stride..rows.end * stride]
            .chunks_exact_mut(stride)
            .map(|row| &mut row[row_start..row_end]);
        if self.state.image_smoothing && half_size {
            let source_x = (x_scale * columns.start as f64 + x_offset) as usize * 4;
            let source_y = (y_scale * rows.start as f64 + y_offset) as usize;
            let source_end = source_x + (row_end - row_start) * 2;
            for (row, out) in destination.enumerate() {
                let top = (source_y + row * 2) * source_stride;
                let bottom = top + source_stride;
                downsample_half_row(
                    &image.premultiplied[top + source_x..top + source_end],
                    &image.premultiplied[bottom + source_x..bottom + source_end],
                    out,
                );
            }
        } else if self.state.image_smoothing {
            let xs = linear_samples(columns, x_scale, x_offset, texels.0..texels.2);
            let ys = linear_samples(rows, y_scale, y_offset, texels.1..texels.3);
            for (out, y) in destination.zip(ys) {
                let top = &image.premultiplied[y.first * source_stride..][..source_stride];
                let bottom = &image.premultiplied[y.second * source_stride..][..source_stride];
                let top = top.as_chunks::<4>().0;
                let bottom = bottom.as_chunks::<4>().0;
                for (pixel, x) in out.as_chunks_mut::<4>().0.iter_mut().zip(&xs) {
                    let left = SAMPLE_ONE - x.fraction;
                    let upper = SAMPLE_ONE - y.fraction;
                    let top_left = top[x.first];
                    let top_right = top[x.second];
                    let bottom_left = bottom[x.first];
                    let bottom_right = bottom[x.second];
                    for channel in 0..3 {
                        let upper_sample = u32::from(top_left[channel]) * left
                            + u32::from(top_right[channel]) * x.fraction;
                        let lower_sample = u32::from(bottom_left[channel]) * left
                            + u32::from(bottom_right[channel]) * x.fraction;
                        pixel[channel] = ((upper_sample * upper
                            + lower_sample * y.fraction
                            + SAMPLE_ONE * SAMPLE_ONE / 2)
                            / (SAMPLE_ONE * SAMPLE_ONE))
                            as u8;
                    }
                    pixel[3] = 255;
                }
            }
        } else {
            let xs = nearest_samples(columns, x_scale, x_offset, texels.0..texels.2);
            let ys = nearest_samples(rows, y_scale, y_offset, texels.1..texels.3);
            for (out, y) in destination.zip(ys) {
                let source = &image.premultiplied[y * source_stride..][..source_stride];
                let source = source.as_chunks::<4>().0;
                for (pixel, &x) in out.as_chunks_mut::<4>().0.iter_mut().zip(&xs) {
                    *pixel = source[x];
                }
            }
        }
        self.touch();
    }

    fn copy_image_rows(&mut self, image: &ImageSource<'_>, src: Rect, x: i64, y: i64) {
        let columns = clip_span(src.x0 as i64..src.x1 as i64, x, self.width);
        let rows = clip_span(src.y0 as i64..src.y1 as i64, y, self.height);
        if columns.is_empty() || rows.is_empty() {
            return;
        }
        let bytes = (columns.end - columns.start) as usize * 4;
        for row in rows {
            let from = (row as usize * image.width as usize + columns.start as usize) * 4;
            let to = ((row + y) as usize * self.width as usize + (columns.start + x) as usize) * 4;
            self.pixels[to..to + bytes].copy_from_slice(&image.premultiplied[from..from + bytes]);
        }
        self.touch();
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
