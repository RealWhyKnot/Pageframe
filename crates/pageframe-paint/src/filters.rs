use anyrender::PaintScene as _;
use anyrender::recording::{RenderCommand, Scene};
use kurbo::{Affine, Rect, Shape as _, Vec2};
use num_traits::ToPrimitive as _;
use pageframe_dom::util::ToColorColor as _;
use std::sync::Arc;
use style::color::AbsoluteColor;
use style::computed_values::filter::single_value::T as StyloFilter;

use crate::text::gaussian_blur;

const MAX_FILTER_IMAGE_SIDE: f64 = 8192.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PixelFilter {
    Matrix([f32; 20]),
    Blur(f64),
    DropShadow {
        offset: Vec2,
        std_deviation: f64,
        color: [f32; 4],
    },
}

impl PixelFilter {
    fn grows_by(&self) -> Rect {
        match self {
            Self::Matrix(_) => Rect::ZERO,
            Self::Blur(std_deviation) => {
                let reach = 3.0 * std_deviation;
                Rect::new(-reach, -reach, reach, reach)
            }
            Self::DropShadow {
                offset,
                std_deviation,
                ..
            } => {
                let reach = 3.0 * std_deviation;
                Rect::new(
                    (offset.x - reach).min(0.0),
                    (offset.y - reach).min(0.0),
                    (offset.x + reach).max(0.0),
                    (offset.y + reach).max(0.0),
                )
            }
        }
    }
}

pub(crate) fn pixel_filters(
    filters: &[StyloFilter],
    current_color: &AbsoluteColor,
    scale: f64,
) -> Vec<PixelFilter> {
    filters
        .iter()
        .filter_map(|filter| pixel_filter(filter, current_color, scale))
        .collect()
}

fn pixel_filter(
    filter: &StyloFilter,
    current_color: &AbsoluteColor,
    scale: f64,
) -> Option<PixelFilter> {
    Some(match filter {
        StyloFilter::Blur(radius) => PixelFilter::Blur(f64::from(radius.px()) * scale),
        StyloFilter::Brightness(amount) => PixelFilter::Matrix(linear(amount.0, 0.0)),
        StyloFilter::Contrast(amount) => {
            PixelFilter::Matrix(linear(amount.0, 0.5 - 0.5 * amount.0))
        }
        StyloFilter::Grayscale(amount) => PixelFilter::Matrix(grayscale(amount.0.min(1.0))),
        StyloFilter::HueRotate(angle) => PixelFilter::Matrix(hue_rotate(angle.radians())),
        StyloFilter::Invert(amount) => {
            let amount = amount.0.min(1.0);
            PixelFilter::Matrix(linear(1.0 - 2.0 * amount, amount))
        }
        StyloFilter::Opacity(amount) => PixelFilter::Matrix(opacity(amount.0.min(1.0))),
        StyloFilter::Saturate(amount) => PixelFilter::Matrix(saturate(amount.0)),
        StyloFilter::Sepia(amount) => PixelFilter::Matrix(sepia(amount.0.min(1.0))),
        StyloFilter::DropShadow(shadow) => {
            let [r, g, b, alpha] = shadow
                .color
                .resolve_to_absolute(current_color)
                .as_color_color()
                .components;
            PixelFilter::DropShadow {
                offset: Vec2::new(
                    f64::from(shadow.horizontal.px()) * scale,
                    f64::from(shadow.vertical.px()) * scale,
                ),
                std_deviation: f64::from(shadow.blur.px()) * scale,
                color: [r * alpha, g * alpha, b * alpha, alpha],
            }
        }
        StyloFilter::Url(_) => return None,
    })
}

fn rgb_matrix(rows: [[f32; 3]; 3]) -> [f32; 20] {
    let [r, g, b] = rows;
    [
        r[0], r[1], r[2], 0.0, 0.0, g[0], g[1], g[2], 0.0, 0.0, b[0], b[1], b[2], 0.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
    ]
}

fn linear(slope: f32, intercept: f32) -> [f32; 20] {
    [
        slope, 0.0, 0.0, 0.0, intercept, 0.0, slope, 0.0, 0.0, intercept, 0.0, 0.0, slope, 0.0,
        intercept, 0.0, 0.0, 0.0, 1.0, 0.0,
    ]
}

fn opacity(amount: f32) -> [f32; 20] {
    let mut matrix = linear(1.0, 0.0);
    matrix[18] = amount;
    matrix
}

fn grayscale(amount: f32) -> [f32; 20] {
    let s = 1.0 - amount;
    rgb_matrix([
        [
            0.2126 + 0.7874 * s,
            0.7152 - 0.7152 * s,
            0.0722 - 0.0722 * s,
        ],
        [
            0.2126 - 0.2126 * s,
            0.7152 + 0.2848 * s,
            0.0722 - 0.0722 * s,
        ],
        [
            0.2126 - 0.2126 * s,
            0.7152 - 0.7152 * s,
            0.0722 + 0.9278 * s,
        ],
    ])
}

fn sepia(amount: f32) -> [f32; 20] {
    let s = 1.0 - amount;
    rgb_matrix([
        [0.393 + 0.607 * s, 0.769 - 0.769 * s, 0.189 - 0.189 * s],
        [0.349 - 0.349 * s, 0.686 + 0.314 * s, 0.168 - 0.168 * s],
        [0.272 - 0.272 * s, 0.534 - 0.534 * s, 0.131 + 0.869 * s],
    ])
}

fn saturate(s: f32) -> [f32; 20] {
    rgb_matrix([
        [0.213 + 0.787 * s, 0.715 - 0.715 * s, 0.072 - 0.072 * s],
        [0.213 - 0.213 * s, 0.715 + 0.285 * s, 0.072 - 0.072 * s],
        [0.213 - 0.213 * s, 0.715 - 0.715 * s, 0.072 + 0.928 * s],
    ])
}

fn hue_rotate(radians: f32) -> [f32; 20] {
    let (sin, cos) = radians.sin_cos();
    rgb_matrix([
        [
            0.213 + cos * 0.787 - sin * 0.213,
            0.715 - cos * 0.715 - sin * 0.715,
            0.072 - cos * 0.072 + sin * 0.928,
        ],
        [
            0.213 - cos * 0.213 + sin * 0.143,
            0.715 + cos * 0.285 + sin * 0.140,
            0.072 - cos * 0.072 - sin * 0.283,
        ],
        [
            0.213 - cos * 0.213 - sin * 0.787,
            0.715 - cos * 0.715 + sin * 0.715,
            0.072 + cos * 0.928 + sin * 0.072,
        ],
    ])
}

pub(crate) fn filter_reach(filters: &[PixelFilter]) -> Rect {
    filters.iter().fold(Rect::ZERO, |reach, filter| {
        let grow = filter.grows_by();
        Rect::new(
            reach.x0 + grow.x0,
            reach.y0 + grow.y0,
            reach.x1 + grow.x1,
            reach.y1 + grow.y1,
        )
    })
}

fn filter_region(scene: &Scene, filters: &[PixelFilter], limit: Rect) -> Option<Rect> {
    let reach = filter_reach(filters);
    let ink = ink_bounds(scene)?;
    let region = Rect::new(
        ink.x0 + reach.x0,
        ink.y0 + reach.y0,
        ink.x1 + reach.x1,
        ink.y1 + reach.y1,
    );
    let limit = Rect::new(
        limit.x0 + reach.x0 - reach.x1,
        limit.y0 + reach.y0 - reach.y1,
        limit.x1 + reach.x1 - reach.x0,
        limit.y1 + reach.y1 - reach.y0,
    );
    let region = region.intersect(limit).expand();
    let fits = |side: f64| (1.0..=MAX_FILTER_IMAGE_SIDE).contains(&side);
    (fits(region.width()) && fits(region.height())).then_some(region)
}

fn ink_bounds(scene: &Scene) -> Option<Rect> {
    let mut clips: Vec<Option<Rect>> = Vec::new();
    let mut ink: Option<Rect> = None;
    for command in &scene.commands {
        let bounds = match command {
            RenderCommand::PushLayer(layer) => {
                clips.push(Some(
                    layer
                        .transform
                        .transform_rect_bbox(layer.clip.bounding_box()),
                ));
                continue;
            }
            RenderCommand::PushClipLayer(clip) => {
                clips.push(Some(
                    clip.transform.transform_rect_bbox(clip.clip.bounding_box()),
                ));
                continue;
            }
            RenderCommand::PopLayer => {
                clips.pop();
                continue;
            }
            RenderCommand::Fill(fill) => fill
                .transform
                .transform_rect_bbox(fill.shape.bounding_box()),
            RenderCommand::Stroke(stroke) => {
                let half = stroke.style.width * stroke.style.miter_limit.max(1.0) / 2.0;
                stroke
                    .transform
                    .transform_rect_bbox(stroke.shape.bounding_box().inflate(half, half))
            }
            RenderCommand::GlyphRun(run) => {
                let size = f64::from(run.font_size);
                let Some(glyphs) = run
                    .glyphs
                    .iter()
                    .map(|glyph| {
                        let (x, y) = (f64::from(glyph.x), f64::from(glyph.y));
                        Rect::new(x - size, y - 1.5 * size, x + 2.0 * size, y + size)
                    })
                    .reduce(|a, b| a.union(b))
                else {
                    continue;
                };
                let glyphs = run
                    .glyph_transform
                    .map_or(glyphs, |transform| transform.transform_rect_bbox(glyphs));
                run.transform.transform_rect_bbox(glyphs)
            }
            RenderCommand::BoxShadow(shadow) => {
                let reach = 3.0 * shadow.std_dev;
                shadow
                    .transform
                    .transform_rect_bbox(shadow.rect.inflate(reach, reach))
            }
        };
        let bounds = clips
            .iter()
            .flatten()
            .fold(bounds, |bounds, clip| bounds.intersect(*clip));
        if bounds.width() > 0.0 && bounds.height() > 0.0 {
            ink = Some(ink.map_or(bounds, |ink| ink.union(bounds)));
        }
    }
    ink
}

pub(crate) enum FilterOutput {
    Image(peniko::ImageBrush, Rect),
    Unfiltered(Scene),
}

pub(crate) fn filtered_image(
    recording: Scene,
    filters: &[PixelFilter],
    limit: Rect,
) -> FilterOutput {
    let Some(region) = filter_region(&recording, filters, limit) else {
        return FilterOutput::Unfiltered(recording);
    };
    let sizes = region.width().to_u32().zip(region.height().to_u32());
    let Some((width, height)) = sizes else {
        return FilterOutput::Unfiltered(recording);
    };
    let (Ok(columns), Ok(rows)) = (usize::try_from(width), usize::try_from(height)) else {
        return FilterOutput::Unfiltered(recording);
    };
    let pixels = anyrender::render_to_buffer::<anyrender_vello_cpu::VelloCpuImageRenderer, _>(
        |scene| scene.append_scene(recording, Affine::translate((-region.x0, -region.y0))),
        width,
        height,
    );
    let mut image = FilterImage::from_rgba8(&pixels, columns, rows);
    for filter in filters {
        image.apply(filter);
    }
    let image = peniko::ImageBrush {
        image: peniko::ImageData {
            data: peniko::Blob::new(Arc::new(image.to_rgba8())),
            format: peniko::ImageFormat::Rgba8,
            alpha_type: peniko::ImageAlphaType::Alpha,
            width,
            height,
        },
        sampler: peniko::ImageSampler::default(),
    };
    FilterOutput::Image(image, region)
}

pub(crate) fn filtered_backdrop(
    recording: Scene,
    area: Rect,
    filters: &[PixelFilter],
) -> Option<(peniko::ImageBrush, Rect)> {
    let area = area.expand();
    let reach = filter_reach(filters);
    let pad = [-reach.x0, -reach.y0, reach.x1, reach.y1]
        .into_iter()
        .fold(0.0_f64, f64::max)
        .ceil();
    let fits = |side: f64| (1.0..=MAX_FILTER_IMAGE_SIDE).contains(&side);
    if !fits(area.width() + 2.0 * pad) || !fits(area.height() + 2.0 * pad) {
        return None;
    }
    let width = area.width().to_u32()?;
    let height = area.height().to_u32()?;
    let pixels = anyrender::render_to_buffer::<anyrender_vello_cpu::VelloCpuImageRenderer, _>(
        |scene| scene.append_scene(recording, Affine::translate((-area.x0, -area.y0))),
        width,
        height,
    );
    let margin = pad.to_usize()?;
    let mut image = FilterImage::from_rgba8(
        &pixels,
        usize::try_from(width).ok()?,
        usize::try_from(height).ok()?,
    )
    .mirrored(margin);
    for filter in filters {
        image.apply(filter);
    }
    let image = image.cropped(margin);
    let image = peniko::ImageBrush {
        image: peniko::ImageData {
            data: peniko::Blob::new(Arc::new(image.to_rgba8())),
            format: peniko::ImageFormat::Rgba8,
            alpha_type: peniko::ImageAlphaType::Alpha,
            width: u32::try_from(image.width).ok()?,
            height: u32::try_from(image.height).ok()?,
        },
        sampler: peniko::ImageSampler::default(),
    };
    Some((image, area))
}

fn mirror_index(index: isize, length: usize) -> usize {
    let Ok(length) = isize::try_from(length) else {
        return 0;
    };
    if length == 0 {
        return 0;
    }
    let folded = index.rem_euclid(2 * length);
    let mirrored = if folded < length {
        folded
    } else {
        2 * length - 1 - folded
    };
    usize::try_from(mirrored).unwrap_or(0)
}

struct FilterImage {
    width: usize,
    height: usize,
    pixels: Vec<[f32; 4]>,
}

impl FilterImage {
    fn mirrored(&self, margin: usize) -> Self {
        let width = self.width + 2 * margin;
        let height = self.height + 2 * margin;
        let offset = isize::try_from(margin).unwrap_or(0);
        let mut pixels = Vec::with_capacity(width * height);
        for padded_row in 0..height {
            let row = isize::try_from(padded_row).unwrap_or(0) - offset;
            let source_row = mirror_index(row, self.height);
            for padded_column in 0..width {
                let column = isize::try_from(padded_column).unwrap_or(0) - offset;
                let source_column = mirror_index(column, self.width);
                pixels.push(self.pixels[source_row * self.width + source_column]);
            }
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    fn cropped(&self, margin: usize) -> Self {
        let width = self.width.saturating_sub(2 * margin);
        let height = self.height.saturating_sub(2 * margin);
        let mut pixels = Vec::with_capacity(width * height);
        for row in margin..margin + height {
            let start = row * self.width + margin;
            pixels.extend_from_slice(&self.pixels[start..start + width]);
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    fn from_rgba8(data: &[u8], width: usize, height: usize) -> Self {
        let pixels = data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|px| px.map(|channel| f32::from(channel) / 255.0))
            .collect();
        Self {
            width,
            height,
            pixels,
        }
    }

    fn to_rgba8(&self) -> Vec<u8> {
        self.pixels
            .iter()
            .flat_map(|px| {
                let alpha = px[3].clamp(0.0, 1.0);
                let straight = |channel: f32| {
                    if alpha > 0.0 {
                        (channel / alpha).clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                };
                [straight(px[0]), straight(px[1]), straight(px[2]), alpha]
                    .map(|channel| (channel * 255.0).round().to_u8().unwrap_or(0))
            })
            .collect()
    }

    fn apply(&mut self, filter: &PixelFilter) {
        match filter {
            PixelFilter::Matrix(matrix) => self.apply_matrix(matrix),
            PixelFilter::Blur(std_deviation) => {
                for channel in 0..4 {
                    self.blur_channel(channel, *std_deviation);
                }
            }
            PixelFilter::DropShadow {
                offset,
                std_deviation,
                color,
            } => self.apply_drop_shadow(*offset, *std_deviation, color),
        }
    }

    fn apply_matrix(&mut self, m: &[f32; 20]) {
        for px in &mut self.pixels {
            let alpha = px[3];
            if alpha <= 0.0 {
                continue;
            }
            let [r, g, b] = [px[0] / alpha, px[1] / alpha, px[2] / alpha];
            let row = |i: usize| {
                (m[i] * r + m[i + 1] * g + m[i + 2] * b + m[i + 3] * alpha + m[i + 4])
                    .clamp(0.0, 1.0)
            };
            let out_alpha = row(15);
            *px = [
                row(0) * out_alpha,
                row(5) * out_alpha,
                row(10) * out_alpha,
                out_alpha,
            ];
        }
    }

    fn blur_channel(&mut self, channel: usize, std_deviation: f64) {
        let mut values: Vec<f32> = self.pixels.iter().map(|px| px[channel]).collect();
        gaussian_blur(&mut values, self.width, self.height, std_deviation);
        for (px, value) in self.pixels.iter_mut().zip(values) {
            px[channel] = value;
        }
    }

    fn apply_drop_shadow(&mut self, offset: Vec2, std_deviation: f64, color: &[f32; 4]) {
        let dx = offset.x.round().to_isize().unwrap_or(0);
        let dy = offset.y.round().to_isize().unwrap_or(0);
        let mut alpha = vec![0.0; self.pixels.len()];
        for y in 0..self.height {
            let Some(source_y) = y.checked_add_signed(-dy).filter(|&sy| sy < self.height) else {
                continue;
            };
            for x in 0..self.width {
                if let Some(source_x) = x.checked_add_signed(-dx).filter(|&sx| sx < self.width) {
                    alpha[y * self.width + x] = self.pixels[source_y * self.width + source_x][3];
                }
            }
        }
        gaussian_blur(&mut alpha, self.width, self.height, std_deviation);
        for y in 0..self.height {
            for x in 0..self.width {
                let shadow = alpha[y * self.width + x];
                let px = &mut self.pixels[y * self.width + x];
                let under = 1.0 - px[3];
                for channel in 0..4 {
                    px[channel] += color[channel] * shadow * under;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_pixel(rgba: [u8; 4]) -> FilterImage {
        FilterImage::from_rgba8(&rgba, 1, 1)
    }

    #[test]
    fn grayscale_uses_rec709_luma() {
        let mut image = one_pixel([255, 0, 0, 255]);
        image.apply(&PixelFilter::Matrix(grayscale(1.0)));
        assert_eq!(image.to_rgba8(), vec![54, 54, 54, 255]);
    }

    #[test]
    fn invert_works_on_unpremultiplied_colour() {
        let mut image = one_pixel([100, 0, 0, 128]);
        image.apply(&PixelFilter::Matrix(linear(-1.0, 1.0)));
        assert_eq!(image.to_rgba8(), vec![56, 255, 255, 128]);
    }

    #[test]
    fn brightness_clamps_each_step() {
        let mut image = one_pixel([200, 100, 0, 255]);
        image.apply(&PixelFilter::Matrix(linear(2.0, 0.0)));
        image.apply(&PixelFilter::Matrix(linear(0.5, 0.0)));
        assert_eq!(image.to_rgba8(), vec![128, 100, 0, 255]);
    }

    #[test]
    fn drop_shadow_sits_under_the_source() {
        let mut data = vec![0; 3 * 3 * 4];
        data[..4].copy_from_slice(&[0, 0, 255, 255]);
        let mut image = FilterImage::from_rgba8(&data, 3, 3);
        image.apply(&PixelFilter::DropShadow {
            offset: Vec2::new(1.0, 1.0),
            std_deviation: 0.0,
            color: [1.0, 0.0, 0.0, 1.0],
        });
        let out = image.to_rgba8();
        assert_eq!(&out[..4], &[0, 0, 255, 255]);
        assert_eq!(&out[16..20], &[255, 0, 0, 255]);
        assert_eq!(&out[4..8], &[0, 0, 0, 0]);
    }

    #[test]
    fn reach_adds_up_through_the_chain() {
        let reach = filter_reach(&[
            PixelFilter::Blur(2.0),
            PixelFilter::DropShadow {
                offset: Vec2::new(4.0, -1.0),
                std_deviation: 1.0,
                color: [0.0; 4],
            },
        ]);
        assert_eq!(reach, Rect::new(-6.0, -10.0, 13.0, 8.0));
    }
}
