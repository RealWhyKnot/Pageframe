use std::io::Cursor;
use std::sync::Arc;

use anyrender::PaintScene;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder as _};
use kurbo::{Affine, Rect, Vec2};
use peniko::{
    Blob, Extend, Fill, ImageAlphaType, ImageBrush, ImageData, ImageFormat, ImageQuality,
    ImageSampler,
};

use super::{Canvas, finite};

const MAX_SIDE: u32 = 16_384;
const MAX_PIXELS: u64 = 67_108_864;
const MAX_SOURCE_BYTES: u64 = 268_435_456;

#[derive(Clone, Debug)]
pub struct Image {
    data: Blob<u8>,
    width: u32,
    height: u32,
}

impl Image {
    pub fn from_rgba(width: u32, height: u32, pixels: Vec<u8>) -> Option<Self> {
        let sized = (1..=MAX_SIDE).contains(&width) && (1..=MAX_SIDE).contains(&height);
        if !sized {
            return None;
        }
        let wanted = usize::try_from(u64::from(width) * u64::from(height) * 4).ok()?;
        (pixels.len() == wanted).then(|| Self {
            data: Blob::new(Arc::new(pixels)),
            width,
            height,
        })
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let mut reader = image::ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .ok()?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_SIDE);
        limits.max_image_height = Some(MAX_SIDE);
        reader.limits(limits);
        let mut decoder = reader.into_decoder().ok()?;
        let (width, height) = decoder.dimensions();
        let small = u64::from(width) * u64::from(height) <= MAX_PIXELS
            && decoder.total_bytes() <= MAX_SOURCE_BYTES;
        if !small {
            return None;
        }
        let turn = decoder.orientation().unwrap_or(Orientation::NoTransforms);
        let mut decoded = DynamicImage::from_decoder(decoder).ok()?;
        decoded.apply_orientation(turn);
        Self::from_rgba(
            decoded.width(),
            decoded.height(),
            decoded.into_rgba8().into_raw(),
        )
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    fn bounds(&self) -> Rect {
        Rect::new(0.0, 0.0, f64::from(self.width), f64::from(self.height))
    }

    fn brush(&self, alpha: f32) -> ImageBrush {
        ImageBrush {
            image: ImageData {
                data: self.data.clone(),
                format: ImageFormat::Rgba8,
                width: self.width,
                height: self.height,
                alpha_type: ImageAlphaType::Alpha,
            },
            sampler: ImageSampler {
                x_extend: Extend::Pad,
                y_extend: Extend::Pad,
                quality: ImageQuality::Medium,
                alpha,
            },
        }
    }
}

impl Canvas {
    pub fn draw_image(&mut self, image: &Image, x: f64, y: f64) -> &mut Self {
        let bounds = image.bounds();
        self.place_image(image, bounds, bounds + Vec2::new(x, y))
    }

    pub fn draw_image_scaled(
        &mut self,
        image: &Image,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> &mut Self {
        let target = Rect::new(x, y, x + width, y + height);
        self.place_image(image, image.bounds(), target)
    }

    pub fn draw_image_part(
        &mut self,
        image: &Image,
        source: (f64, f64, f64, f64),
        target: (f64, f64, f64, f64),
    ) -> &mut Self {
        let rect =
            |(x, y, width, height): (f64, f64, f64, f64)| Rect::new(x, y, x + width, y + height);
        self.place_image(image, rect(source), rect(target))
    }

    fn place_image(&mut self, image: &Image, source: Rect, target: Rect) -> &mut Self {
        let edges = [
            source.x0, source.y0, source.x1, source.y1, target.x0, target.y0, target.x1, target.y1,
        ];
        if !finite(&edges) {
            return self;
        }
        let (source, target) = (source.abs(), target.abs());
        let shown = source.intersect(image.bounds());
        if shown.width() <= 0.0 || shown.height() <= 0.0 {
            return self;
        }
        let stretch = (
            target.width() / source.width(),
            target.height() / source.height(),
        );
        if !finite(&[stretch.0, stretch.1]) {
            return self;
        }
        let place = Affine::translate((target.x0, target.y0))
            * Affine::scale_non_uniform(stretch.0, stretch.1)
            * Affine::translate((-source.x0, -source.y0));
        let brush = image.brush(self.state.alpha);
        let whole = self.state.transform * place;
        if !finite(&whole.as_coeffs()) {
            return self;
        }
        let [_, lean, tilt, ..] = whole.as_coeffs();
        if lean == 0.0 && tilt == 0.0 {
            let edge = |value: f64| (value + 0.5).floor();
            let covered = whole.transform_rect_bbox(shown);
            if !finite(&[covered.x0, covered.y0, covered.x1, covered.y1]) {
                return self;
            }
            let snapped = Rect::new(
                edge(covered.x0),
                edge(covered.y0),
                edge(covered.x1),
                edge(covered.y1),
            );
            self.scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                brush.as_ref(),
                Some(whole),
                &snapped,
            );
        } else {
            self.scene
                .fill(Fill::NonZero, whole, brush.as_ref(), None, &shown);
        }
        self
    }
}
