use anyrender::PaintScene;
use kurbo::{Affine, Rect};
use pageframe_dom::node::RasterImageData;
use peniko::{
    Blob, Extend, Fill, ImageAlphaType, ImageBrush, ImageData, ImageFormat, ImageQuality,
};
use std::sync::Arc;

const MAX_PIXELS: f64 = 4_194_304.0;
const MAX_COORD: f64 = 16_777_216.0;
const FIXED_ONE: i32 = 1 << 16;
const FRACTIONAL_ONE: f64 = 4_294_967_296.0;
const RASTER_TILE: i32 = 254;
const SHADE_CHUNK: i32 = 127;
const ALIGN_EPSILON: f64 = 1e-4;

struct Axis {
    scale: f64,
    translate: f64,
    one: i32,
    size: i32,
    repeat: bool,
}

impl Axis {
    fn new(scale: f64, translate: f64, size: u32, repeat: bool) -> Self {
        let size = size as i32;
        Self {
            scale,
            translate,
            one: if repeat { FIXED_ONE / size } else { FIXED_ONE },
            size,
            repeat,
        }
    }

    fn map(&self, pixel: i32) -> (f32, f32) {
        let origin = pixel.div_euclid(RASTER_TILE) * RASTER_TILE;
        let inverse = 1.0 / self.scale as f32;
        let mut step = inverse;
        let mut offset = -((self.translate - f64::from(origin)) as f32) * inverse;
        if self.repeat {
            let normalize = 1.0 / self.size as f32;
            step *= normalize;
            offset *= normalize;
        }
        (step, ((pixel - origin) as f32 + 0.5) * step + offset)
    }

    fn bias(&self) -> i32 {
        self.one >> 1
    }

    fn tile(&self, fixed: i32) -> usize {
        if self.repeat {
            (((fixed as u32 & 0xFFFF) * self.size as u32) >> 16) as usize
        } else {
            (fixed >> 16).clamp(0, self.size - 1) as usize
        }
    }

    fn weight(&self, fixed: i32) -> u32 {
        if self.repeat {
            (((fixed as u32 & 0xFFFF) * self.size as u32) >> 12) & 0xF
        } else {
            (fixed >> 12) as u32 & 0xF
        }
    }

    fn sample(&self, fixed: i32) -> (usize, usize, u32) {
        (
            self.tile(fixed),
            self.tile(fixed.wrapping_add(self.one)),
            self.weight(fixed),
        )
    }

    fn column(&self, pixel: i32, span_start: i32) -> (usize, usize, u32) {
        let origin = pixel.div_euclid(RASTER_TILE) * RASTER_TILE;
        let span_start = span_start.max(origin).min(pixel);
        let anchor = span_start + (pixel - span_start) / SHADE_CHUNK * SHADE_CHUNK;
        let (step, point) = self.map(anchor);
        let step = (f64::from(step) * FRACTIONAL_ONE) as i64;
        let fractional = (f64::from(point) * FRACTIONAL_ONE) as i64
            - (i64::from(self.bias()) << 16)
            + i64::from(pixel - anchor) * step;
        self.sample((fractional >> 16) as i32)
    }

    fn row(&self, pixel: i32) -> (usize, usize, u32) {
        let fixed = (self.map(pixel).1 * FIXED_ONE as f32) as i32;
        self.sample(fixed.wrapping_sub(self.bias()))
    }
}

fn premultiply(pixel: &[u8; 4]) -> [u32; 4] {
    let mul = |channel: u8| {
        let product = u32::from(channel) * u32::from(pixel[3]) + 128;
        (product + (product >> 8)) >> 8
    };
    [
        mul(pixel[0]),
        mul(pixel[1]),
        mul(pixel[2]),
        u32::from(pixel[3]),
    ]
}

fn bounded(values: [f64; 4]) -> bool {
    values.iter().all(|value| value.abs() <= MAX_COORD)
}

fn pixel_aligned(scale_x: f64, scale_y: f64, x: f64, y: f64) -> bool {
    let near = |value: f64, target: f64| (value - target).abs() <= ALIGN_EPSILON;
    near(scale_x, 1.0) && near(scale_y, 1.0) && near(x, x.round()) && near(y, y.round())
}

pub(super) fn draw_filtered(
    scene: &mut impl PaintScene,
    image: &RasterImageData,
    transform: Affine,
    tile: Rect,
    dest: Rect,
    extend: Extend,
) -> bool {
    let [a, b, c, d, e, f] = transform.as_coeffs();
    let size_ok =
        image.width > 0 && image.height > 0 && image.width <= 65535 && image.height <= 65535;
    let repeat = match extend {
        Extend::Repeat => true,
        Extend::Pad => false,
        _ => return false,
    };
    if b != 0.0
        || c != 0.0
        || a < 1.0
        || d < 1.0
        || !size_ok
        || !bounded([a, d, e, f])
        || pixel_aligned(a, d, e, f)
    {
        return false;
    }
    let shape = transform.transform_rect_bbox(tile).intersect(dest.expand());
    if !bounded([shape.x0, shape.y0, shape.x1, shape.y1]) {
        return false;
    }
    if shape.width() <= 0.0 || shape.height() <= 0.0 {
        return true;
    }
    let area = shape.expand();
    if area.area() > MAX_PIXELS {
        return false;
    }
    let (width, height) = (area.width() as u32, area.height() as u32);
    let (left, top) = (area.x0 as i32, area.y0 as i32);
    let x = Axis::new(a, e, image.width, repeat);
    let y = Axis::new(d, f, image.height, repeat);
    let stride = image.width as usize;
    let source = image.data.data().as_chunks::<4>().0;
    if source.len() < stride * image.height as usize {
        return false;
    }
    let interior = shape.x0.ceil() as i32;
    let right_edge = (shape.x1.fract() != 0.0).then(|| shape.x1.floor() as i32);
    let columns: Vec<_> = (left..left + width as i32)
        .map(|pixel| {
            let lone = pixel < interior || Some(pixel) == right_edge;
            x.column(pixel, if lone { pixel } else { interior })
        })
        .collect();
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
    for row in 0..height as i32 {
        let (y0, y1, wy) = y.row(top + row);
        for &(x0, x1, wx) in &columns {
            let weights = [
                (16 - wx) * (16 - wy),
                wx * (16 - wy),
                (16 - wx) * wy,
                wx * wy,
            ];
            let texels = [
                premultiply(&source[y0 * stride + x0]),
                premultiply(&source[y0 * stride + x1]),
                premultiply(&source[y1 * stride + x0]),
                premultiply(&source[y1 * stride + x1]),
            ];
            let blend = |channel: usize| {
                let sum: u32 = texels
                    .iter()
                    .zip(weights)
                    .map(|(t, w)| t[channel] * w)
                    .sum();
                (sum >> 8) as u8
            };
            pixels.extend((0..4).map(blend));
        }
    }
    let brush = ImageBrush::new(ImageData {
        data: Blob::new(Arc::new(pixels)),
        format: ImageFormat::Rgba8,
        alpha_type: ImageAlphaType::AlphaPremultiplied,
        width,
        height,
    })
    .with_quality(ImageQuality::Low);
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        brush.as_ref(),
        Some(Affine::translate((area.x0, area.y0))),
        &shape,
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyrender::Scene;

    fn draws(transform: Affine, extend: Extend) -> bool {
        let image = RasterImageData::new(4, 4, Arc::new(vec![200; 64]));
        let mut scene = Scene::new();
        let tile = Rect::new(0.0, 0.0, 4.0, 4.0);
        let dest = Rect::new(0.0, 0.0, 40.0, 40.0);
        let drawn = draw_filtered(&mut scene, &image, transform, tile, dest, extend);
        assert_eq!(drawn, !scene.commands.is_empty());
        drawn
    }

    #[test]
    fn whole_pixel_one_to_one_uses_the_cached_image() {
        for extend in [Extend::Pad, Extend::Repeat] {
            assert!(!draws(Affine::translate((3.0, 7.0)), extend));
            assert!(!draws(Affine::translate((3.00001, -2.0)), extend));
        }
    }

    #[test]
    fn scaled_or_offset_tiles_are_resampled() {
        for extend in [Extend::Pad, Extend::Repeat] {
            assert!(draws(Affine::scale(2.0), extend));
            assert!(draws(Affine::translate((0.5, 0.0)), extend));
            assert!(draws(
                Affine::scale_non_uniform(1.0, 1.5).then_translate((1.0, 1.0).into()),
                extend
            ));
        }
    }
}
