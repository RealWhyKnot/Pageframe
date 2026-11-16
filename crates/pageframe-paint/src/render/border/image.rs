use std::cell::RefCell;
use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::sync::Arc;

use anyrender::{PaintScene, Scene};
use kurbo::{Affine, Rect, Size};
use num_traits::ToPrimitive as _;
use pageframe_dom::CopyStyleGetters as _;
use pageframe_dom::node::{ImageData, RasterImageData, Status};
use peniko::{Extend, Fill};
use style::values::computed::border::BorderImageRepeat;
use style::values::computed::image::Image;
use style::values::computed::{
    BorderImageSideWidth, CSSPixelLength, Gradient, NonNegativeLengthOrNumber,
    NonNegativeNumberOrPercentage, NumberOrPercentage,
};
use style::values::generics::length::GenericLengthOrNumber;
use style::values::specified::border::BorderImageRepeatKeyword;

use crate::gradient::to_peniko_gradient;
use crate::image_cache::ImageCache;
use crate::render::{ElementCx, to_image_quality, to_peniko_image};

const MAX_TILES: usize = 4096;
const MAX_PATTERN_SIDE: f64 = 4096.0;
const SLICE_CACHE_BYTES: usize = 16 << 20;

thread_local! {
    static SLICE_CACHE: RefCell<ImageCache> = RefCell::new(ImageCache::new(SLICE_CACHE_BYTES));
}

enum Source<'a> {
    Raster(&'a RasterImageData),
    Gradient(&'a Gradient),
    #[cfg(feature = "svg")]
    Svg(&'a pageframe_dom::node::SvgImageData),
}

struct Art<'a> {
    source: Source<'a>,
    gradient: Option<(peniko::Gradient, Affine)>,
    image: Size,
}

#[derive(Clone, Copy)]
struct Tiling {
    scale: f64,
    phase: f64,
    spacing: f64,
    stretch: bool,
}

fn tiling(
    rule: BorderImageRepeatKeyword,
    destination: f64,
    source: f64,
    scale: f64,
) -> Option<Tiling> {
    let tile = source * scale;
    match rule {
        BorderImageRepeatKeyword::Stretch => Some(Tiling {
            scale: destination / source,
            phase: 0.0,
            spacing: 0.0,
            stretch: true,
        }),
        BorderImageRepeatKeyword::Repeat => Some(Tiling {
            scale,
            phase: (destination - tile) / 2.0,
            spacing: 0.0,
            stretch: false,
        }),
        BorderImageRepeatKeyword::Round => {
            let repetitions = (destination / tile).round().max(1.0);
            Some(Tiling {
                scale: destination / (source * repetitions),
                phase: 0.0,
                spacing: 0.0,
                stretch: false,
            })
        }
        BorderImageRepeatKeyword::Space => {
            let repetitions = (destination / tile).floor();
            if repetitions < 1.0 {
                return None;
            }
            let spacing = (destination - tile * repetitions) / (repetitions + 1.0);
            Some(Tiling {
                scale,
                phase: spacing,
                spacing,
                stretch: false,
            })
        }
    }
}

fn first_tile(destination_start: f64, tile: f64, axis: Tiling) -> Option<(f64, f64)> {
    let step = tile + axis.spacing;
    if !step.is_finite() || step <= 0.0 || tile <= 0.0 {
        return None;
    }
    let start = destination_start + axis.phase;
    let skipped = ((start + tile - destination_start) / step).ceil().max(1.0) - 1.0;
    Some((start - skipped * step, step))
}

fn tile_starts(
    destination_start: f64,
    destination_end: f64,
    tile: f64,
    axis: Tiling,
) -> Option<Vec<f64>> {
    if axis.stretch {
        return Some(vec![destination_start]);
    }
    let Some((mut start, step)) = first_tile(destination_start, tile, axis) else {
        return Some(Vec::new());
    };
    let mut starts = Vec::new();
    while start < destination_end {
        if starts.len() == MAX_TILES {
            return None;
        }
        starts.push(start);
        start += step;
    }
    Some(starts)
}

fn straight_alpha(premultiplied: &[u8]) -> Vec<u8> {
    premultiplied
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|&[red, green, blue, alpha]| {
            let straight = |channel: u8| {
                if alpha == 0 {
                    0
                } else {
                    let scaled = u32::from(channel) * 255 + u32::from(alpha) / 2;
                    u8::try_from((scaled / u32::from(alpha)).min(255)).unwrap_or(u8::MAX)
                }
            };
            [straight(red), straight(green), straight(blue), alpha]
        })
        .collect()
}

struct Edge {
    slice: f64,
    width: f64,
}

impl Edge {
    fn drawable(&self) -> bool {
        self.slice > 0.0 && self.width > 0.0
    }

    fn scale(&self) -> f64 {
        self.width / self.slice
    }
}

struct Piece {
    source: Rect,
    destination: Rect,
    horizontal: Tiling,
    vertical: Tiling,
}

fn slice(value: &NonNegativeNumberOrPercentage, extent: f64, number_scale: f64) -> f64 {
    let resolved = match value.0 {
        NumberOrPercentage::Percentage(percentage) => f64::from(percentage.0) * extent,
        NumberOrPercentage::Number(number) => f64::from(number) * number_scale,
    };
    resolved.min(extent)
}

impl ElementCx<'_, '_> {
    fn border_image_source(&self) -> Option<Source<'_>> {
        match &self.style.get_border().border_image_source {
            Image::Gradient(gradient) => Some(Source::Gradient(gradient)),
            Image::Url(_) => {
                let loaded = self.element.border_images.first()?.as_ref()?;
                if loaded.status != Status::Ok {
                    return None;
                }
                match &loaded.image {
                    ImageData::Raster(image) if image.width > 0 && image.height > 0 => {
                        Some(Source::Raster(image))
                    }
                    #[cfg(feature = "svg")]
                    ImageData::Svg(svg) if !svg.intrinsic_dimensions.degenerate_view_box => {
                        Some(Source::Svg(svg))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn device_length(&self, css: f64) -> f64 {
        css * self.scale
    }

    fn outset(&self, value: &NonNegativeLengthOrNumber, border_width: f64) -> f64 {
        match value {
            GenericLengthOrNumber::Number(number) => f64::from(number.0) * border_width,
            GenericLengthOrNumber::Length(length) => self.device_length(f64::from(length.0.px())),
        }
    }

    fn image_width(
        &self,
        value: &BorderImageSideWidth,
        border_width: f64,
        slice: f64,
        extent: f64,
    ) -> f64 {
        match value {
            BorderImageSideWidth::Number(number) => (f64::from(number.0) * border_width).round(),
            BorderImageSideWidth::LengthPercentage(length) => {
                let basis = CSSPixelLength::new((extent / self.scale).to_f32().unwrap_or(0.0));
                self.device_length(f64::from(length.0.resolve(basis).px()))
                    .round()
            }
            BorderImageSideWidth::Auto => slice.round(),
        }
    }

    fn virtual_size(&self, source: &Source, area: Size) -> Size {
        match source {
            Source::Raster(image) => Size::new(
                self.device_length(f64::from(image.width)),
                self.device_length(f64::from(image.height)),
            ),
            Source::Gradient(_) => area,
            #[cfg(feature = "svg")]
            Source::Svg(svg) => {
                let width = svg
                    .intrinsic_width()
                    .filter(|length| length.is_finite() && *length > 0.0);
                let height = svg
                    .intrinsic_height()
                    .filter(|length| length.is_finite() && *length > 0.0);
                let ratio = match (width, height) {
                    (Some(known_width), Some(known_height)) => Some(known_width / known_height),
                    _ => svg.viewbox_aspect_ratio(),
                }
                .filter(|ratio| ratio.is_finite() && *ratio > 0.0);
                let (sized_width, sized_height) = crate::render::background::default_sizing(
                    width,
                    height,
                    ratio,
                    (area.width / self.scale).to_f32().unwrap_or(0.0),
                    (area.height / self.scale).to_f32().unwrap_or(0.0),
                );
                Size::new(
                    self.device_length(f64::from(sized_width)),
                    self.device_length(f64::from(sized_height)),
                )
            }
        }
    }

    pub(super) fn draw_border_image(&self, scene: &mut impl PaintScene) -> bool {
        let Some(source) = self.border_image_source() else {
            return false;
        };
        let border = self.style.get_border();
        let widths = self.frame.border_width;
        let outsets = &border.border_image_outset;
        let bounds = self.frame.border_box;
        let area = Rect::new(
            (bounds.x0 - self.outset(&outsets.3, widths.x0)).round(),
            (bounds.y0 - self.outset(&outsets.0, widths.y0)).round(),
            (bounds.x1 + self.outset(&outsets.1, widths.x1)).round(),
            (bounds.y1 + self.outset(&outsets.2, widths.y1)).round(),
        );
        if area.width() <= 0.0 || area.height() <= 0.0 {
            return true;
        }
        let image = self.virtual_size(&source, area.size());
        if image.width <= 0.0 || image.height <= 0.0 {
            return true;
        }

        let slices = &border.border_image_slice;
        let number_scale = self.scale;
        let side = |value, extent| slice(value, extent, number_scale);
        let image_widths = &border.border_image_width;
        let mut top = Edge {
            slice: side(&slices.offsets.0, image.height),
            width: 0.0,
        };
        let mut right = Edge {
            slice: side(&slices.offsets.1, image.width),
            width: 0.0,
        };
        let mut bottom = Edge {
            slice: side(&slices.offsets.2, image.height),
            width: 0.0,
        };
        let mut left = Edge {
            slice: side(&slices.offsets.3, image.width),
            width: 0.0,
        };
        top.width = self.image_width(&image_widths.0, widths.y0.round(), top.slice, area.height());
        right.width = self.image_width(
            &image_widths.1,
            widths.x1.round(),
            right.slice,
            area.width(),
        );
        bottom.width = self.image_width(
            &image_widths.2,
            widths.y1.round(),
            bottom.slice,
            area.height(),
        );
        left.width = self.image_width(&image_widths.3, widths.x0.round(), left.slice, area.width());
        let factor = (area.width() / (left.width + right.width).max(1.0))
            .min(area.height() / (top.width + bottom.width).max(1.0));
        if factor < 1.0 {
            for edge in [&mut top, &mut right, &mut bottom, &mut left] {
                edge.width = (edge.width * factor).floor();
            }
        }

        let BorderImageRepeat(horizontal_rule, vertical_rule) = border.border_image_repeat;
        let stretch = Tiling {
            scale: 1.0,
            phase: 0.0,
            spacing: 0.0,
            stretch: true,
        };
        let inner_x = (area.x0 + left.width, area.x1 - right.width);
        let inner_y = (area.y0 + top.width, area.y1 - bottom.width);
        let source_x = (left.slice, image.width - right.slice);
        let source_y = (top.slice, image.height - bottom.slice);
        let mut pieces = Vec::with_capacity(9);
        let corners = [
            (&top, &left, (0.0, 0.0), (area.x0, area.y0)),
            (
                &top,
                &right,
                (image.width - right.slice, 0.0),
                (area.x1 - right.width, area.y0),
            ),
            (
                &bottom,
                &right,
                (image.width - right.slice, image.height - bottom.slice),
                (area.x1 - right.width, area.y1 - bottom.width),
            ),
            (
                &bottom,
                &left,
                (0.0, image.height - bottom.slice),
                (area.x0, area.y1 - bottom.width),
            ),
        ];
        for (vertical, horizontal, (sx, sy), (dx, dy)) in corners {
            if vertical.drawable() && horizontal.drawable() {
                pieces.push(Piece {
                    source: Rect::new(sx, sy, sx + horizontal.slice, sy + vertical.slice),
                    destination: Rect::new(dx, dy, dx + horizontal.width, dy + vertical.width),
                    horizontal: stretch,
                    vertical: stretch,
                });
            }
        }
        let source_width = source_x.1 - source_x.0;
        let source_height = source_y.1 - source_y.0;
        let destination_width = inner_x.1 - inner_x.0;
        let destination_height = inner_y.1 - inner_y.0;
        if source_width > 0.0 && destination_width > 0.0 {
            for (edge, source_top, destination_top) in [
                (&top, 0.0, area.y0),
                (&bottom, image.height - bottom.slice, area.y1 - bottom.width),
            ] {
                if !edge.drawable() {
                    continue;
                }
                let Some(horizontal) = tiling(
                    horizontal_rule,
                    destination_width,
                    source_width,
                    edge.scale(),
                ) else {
                    continue;
                };
                pieces.push(Piece {
                    source: Rect::new(source_x.0, source_top, source_x.1, source_top + edge.slice),
                    destination: Rect::new(
                        inner_x.0,
                        destination_top,
                        inner_x.1,
                        destination_top + edge.width,
                    ),
                    horizontal,
                    vertical: Tiling {
                        scale: edge.scale(),
                        ..stretch
                    },
                });
            }
        }
        if source_height > 0.0 && destination_height > 0.0 {
            for (edge, source_left, destination_left) in [
                (&left, 0.0, area.x0),
                (&right, image.width - right.slice, area.x1 - right.width),
            ] {
                if !edge.drawable() {
                    continue;
                }
                let Some(vertical) = tiling(
                    vertical_rule,
                    destination_height,
                    source_height,
                    edge.scale(),
                ) else {
                    continue;
                };
                pieces.push(Piece {
                    source: Rect::new(
                        source_left,
                        source_y.0,
                        source_left + edge.slice,
                        source_y.1,
                    ),
                    destination: Rect::new(
                        destination_left,
                        inner_y.0,
                        destination_left + edge.width,
                        inner_y.1,
                    ),
                    horizontal: Tiling {
                        scale: edge.scale(),
                        ..stretch
                    },
                    vertical,
                });
            }
        }
        if slices.fill
            && source_width > 0.0
            && source_height > 0.0
            && destination_width > 0.0
            && destination_height > 0.0
        {
            let scale_x = if top.drawable() {
                top.scale()
            } else if bottom.drawable() {
                bottom.scale()
            } else {
                1.0
            };
            let scale_y = if left.drawable() {
                left.scale()
            } else if right.drawable() {
                right.scale()
            } else {
                1.0
            };
            if let (Some(horizontal), Some(vertical)) = (
                tiling(horizontal_rule, destination_width, source_width, scale_x),
                tiling(vertical_rule, destination_height, source_height, scale_y),
            ) {
                pieces.push(Piece {
                    source: Rect::new(source_x.0, source_y.0, source_x.1, source_y.1),
                    destination: Rect::new(inner_x.0, inner_y.0, inner_x.1, inner_y.1),
                    horizontal,
                    vertical,
                });
            }
        }

        let gradient = match source {
            Source::Gradient(gradient) => {
                let (gradient, transform) = to_peniko_gradient(
                    gradient,
                    Rect::from_origin_size((0.0, 0.0), image),
                    self.scale,
                    &self.style.clone_color(),
                );
                Some((gradient, transform.unwrap_or(Affine::IDENTITY)))
            }
            _ => None,
        };
        let art = Art {
            source,
            gradient,
            image,
        };
        for piece in &pieces {
            self.draw_border_image_piece(scene, &art, piece);
        }
        true
    }

    fn draw_border_image_piece(&self, scene: &mut impl PaintScene, art: &Art, piece: &Piece) {
        let tile_width = piece.source.width() * piece.horizontal.scale;
        let tile_height = piece.source.height() * piece.vertical.scale;
        let destination = piece.destination;
        if piece.horizontal.stretch && piece.vertical.stretch {
            self.draw_border_image_tile(scene, art, piece.source, (destination, destination));
            return;
        }
        let width = if piece.horizontal.stretch {
            destination.width()
        } else {
            tile_width
        };
        let height = if piece.vertical.stretch {
            destination.height()
        } else {
            tile_height
        };
        let columns = tile_starts(destination.x0, destination.x1, tile_width, piece.horizontal);
        let rows = tile_starts(destination.y0, destination.y1, tile_height, piece.vertical);
        let (Some(columns), Some(rows)) = (columns, rows) else {
            self.fill_repeating(scene, art, piece, Size::new(width, height));
            return;
        };
        if columns.is_empty() || rows.is_empty() {
            return;
        }
        if columns.len() * rows.len() > MAX_TILES {
            self.fill_repeating(scene, art, piece, Size::new(width, height));
            return;
        }
        let overlap = |axis: Tiling| {
            if axis.stretch || axis.spacing > 0.0 {
                0.0
            } else {
                1.0
            }
        };
        let (overlap_x, overlap_y) = (overlap(piece.horizontal), overlap(piece.vertical));
        scene.push_clip_layer(Fill::NonZero, self.transform, &destination);
        for &top in &rows {
            for &left in &columns {
                let tile = Rect::new(left, top, left + width, top + height);
                let shape = Rect::new(
                    left,
                    top,
                    left + width + overlap_x,
                    top + height + overlap_y,
                );
                self.draw_border_image_tile(scene, art, piece.source, (tile, shape));
            }
        }
        scene.pop_layer();
    }

    fn fill_repeating(&self, scene: &mut impl PaintScene, art: &Art, piece: &Piece, tile: Size) {
        let destination = piece.destination;
        let period = |start: f64, length: f64, axis: Tiling| {
            if axis.stretch {
                Some((start, length))
            } else {
                first_tile(start, length, axis)
            }
        };
        let Some((left, period_x)) = period(destination.x0, tile.width, piece.horizontal) else {
            return;
        };
        let Some((top, period_y)) = period(destination.y0, tile.height, piece.vertical) else {
            return;
        };
        let fits = |side: f64| (1.0..=MAX_PATTERN_SIDE).contains(&side);
        let (pattern_width, pattern_height) = (period_x.ceil(), period_y.ceil());
        if !fits(pattern_width)
            || !fits(pattern_height)
            || !self.transform.determinant().is_normal()
        {
            return;
        }
        let (Some(pixels_wide), Some(pixels_high)) =
            (pattern_width.to_u32(), pattern_height.to_u32())
        else {
            return;
        };
        let local = Rect::from_origin_size((0.0, 0.0), tile);
        let mut recording = Scene::new();
        self.draw_border_image_tile(&mut recording, art, piece.source, (local, local));
        let to_pattern =
            Affine::scale_non_uniform(pattern_width / period_x, pattern_height / period_y)
                * self.transform.inverse();
        let premultiplied =
            anyrender::render_to_buffer::<anyrender_vello_cpu::VelloCpuImageRenderer, _>(
                |pattern| pattern.append_scene(recording, to_pattern),
                pixels_wide,
                pixels_high,
            );
        let mut brush = peniko::ImageBrush::new(peniko::ImageData {
            data: peniko::Blob::new(Arc::new(straight_alpha(&premultiplied))),
            format: peniko::ImageFormat::Rgba8,
            alpha_type: peniko::ImageAlphaType::Alpha,
            width: pixels_wide,
            height: pixels_high,
        });
        brush.sampler.x_extend = Extend::Repeat;
        brush.sampler.y_extend = Extend::Repeat;
        let brush_transform = Affine::translate((left, top))
            * Affine::scale_non_uniform(period_x / pattern_width, period_y / pattern_height);
        scene.fill(
            Fill::NonZero,
            self.transform,
            brush.as_ref(),
            Some(brush_transform),
            &destination,
        );
    }

    fn fill_raster(
        &self,
        scene: &mut impl PaintScene,
        raster: &RasterImageData,
        image: Size,
        (from, tile, shape): (Rect, Rect, Rect),
    ) {
        let pixels_x = image.width / f64::from(raster.width);
        let pixels_y = image.height / f64::from(raster.height);
        let left = (from.x0 / pixels_x).floor().max(0.0);
        let top = (from.y0 / pixels_y).floor().max(0.0);
        let right = (from.x1 / pixels_x).ceil().min(f64::from(raster.width));
        let bottom = (from.y1 / pixels_y).ceil().min(f64::from(raster.height));
        let Some(cropped) = crop(raster, left, top, right, bottom) else {
            return;
        };
        let brush_transform = Affine::translate(tile.origin().to_vec2())
            * Affine::scale_non_uniform(tile.width() / from.width(), tile.height() / from.height())
            * Affine::translate(-from.origin().to_vec2())
            * Affine::scale_non_uniform(pixels_x, pixels_y)
            * Affine::translate((left, top));
        let quality = to_image_quality(self.style.clone_image_rendering());
        let mut brush = to_peniko_image(&cropped, quality);
        brush.sampler.x_extend = Extend::Pad;
        brush.sampler.y_extend = Extend::Pad;
        scene.fill(
            Fill::NonZero,
            self.transform,
            brush.as_ref(),
            Some(brush_transform),
            &shape,
        );
    }

    fn draw_border_image_tile(
        &self,
        scene: &mut impl PaintScene,
        art: &Art,
        from: Rect,
        (to, shape): (Rect, Rect),
    ) {
        let image = art.image;
        if from.width() <= 0.0 || from.height() <= 0.0 {
            return;
        }
        let map = Affine::translate(to.origin().to_vec2())
            * Affine::scale_non_uniform(to.width() / from.width(), to.height() / from.height())
            * Affine::translate(-from.origin().to_vec2());
        match &art.source {
            Source::Raster(raster) => {
                self.fill_raster(scene, raster, image, (from, to, shape));
            }
            Source::Gradient(_) => {
                let Some((gradient, gradient_transform)) = &art.gradient else {
                    return;
                };
                scene.fill(
                    Fill::NonZero,
                    self.transform,
                    anyrender::Paint::Gradient(gradient),
                    Some(map * *gradient_transform),
                    &shape,
                );
            }
            #[cfg(feature = "svg")]
            Source::Svg(svg) => {
                let viewport = crate::render::svg_viewport_affine(
                    svg,
                    image.width.to_f32().unwrap_or(0.0),
                    image.height.to_f32().unwrap_or(0.0),
                );
                scene.push_clip_layer(Fill::NonZero, self.transform, &shape);
                anyrender_svg::render_svg_tree(scene, &svg.tree, self.transform * map * viewport);
                scene.pop_layer();
            }
        }
    }
}

fn crop(
    raster: &RasterImageData,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
) -> Option<RasterImageData> {
    let (x0, y0, x1, y1) = (
        left.to_u32()?,
        top.to_u32()?,
        right.to_u32()?,
        bottom.to_u32()?,
    );
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    if x0 == 0 && y0 == 0 && x1 == raster.width && y1 == raster.height {
        return Some(raster.clone());
    }
    let mut hasher = DefaultHasher::new();
    (raster.data.id(), x0, y0, x1, y1).hash(&mut hasher);
    let key = hasher.finish();
    if let Some(image) = SLICE_CACHE.with_borrow_mut(|cache| cache.get(key)) {
        return Some(RasterImageData {
            width: image.width,
            height: image.height,
            data: image.data,
        });
    }
    let stride = usize::try_from(raster.width).ok()? * 4;
    let (start, end) = (usize::try_from(x0).ok()? * 4, usize::try_from(x1).ok()? * 4);
    let bytes = raster.data.data();
    let mut pixels = Vec::with_capacity((end - start) * usize::try_from(y1 - y0).ok()?);
    for row in usize::try_from(y0).ok()?..usize::try_from(y1).ok()? {
        pixels.extend_from_slice(bytes.get(row * stride + start..row * stride + end)?);
    }
    let cropped = RasterImageData {
        width: x1 - x0,
        height: y1 - y0,
        data: peniko::Blob::new(Arc::new(pixels)),
    };
    let image = peniko::ImageData {
        data: cropped.data.clone(),
        format: peniko::ImageFormat::Rgba8,
        alpha_type: peniko::ImageAlphaType::Alpha,
        width: cropped.width,
        height: cropped.height,
    };
    SLICE_CACHE.with_borrow_mut(|cache| cache.insert(key, image));
    Some(cropped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_fits_whole_tiles() {
        let axis =
            tiling(BorderImageRepeatKeyword::Round, 100.0, 10.0, 3.0).expect("round always tiles");
        assert_eq!(axis.scale, 100.0 / 30.0);
        assert_eq!(
            tile_starts(0.0, 100.0, 10.0 * axis.scale, axis).map(|starts| starts.len()),
            Some(3)
        );
    }

    #[test]
    fn repeat_centres_a_tile() {
        let axis = tiling(BorderImageRepeatKeyword::Repeat, 100.0, 10.0, 3.0)
            .expect("repeat always tiles");
        let starts = tile_starts(0.0, 100.0, 30.0, axis).expect("a few tiles");
        assert!(starts.contains(&35.0));
        assert_eq!(starts.first().copied(), Some(-25.0));
    }

    #[test]
    fn space_skips_edges_too_short_for_a_tile() {
        assert!(tiling(BorderImageRepeatKeyword::Space, 20.0, 10.0, 3.0).is_none());
        let axis =
            tiling(BorderImageRepeatKeyword::Space, 100.0, 10.0, 3.0).expect("three tiles fit");
        assert_eq!(axis.spacing, 2.5);
        assert_eq!(
            tile_starts(0.0, 100.0, 30.0, axis),
            Some(vec![2.5, 35.0, 67.5])
        );
    }

    #[test]
    fn repeated_crops_share_one_blob() {
        let pixels: Vec<u8> = (0..4 * 4 * 4)
            .map(|byte| u8::try_from(byte).unwrap_or(0))
            .collect();
        let raster = RasterImageData {
            width: 4,
            height: 4,
            data: peniko::Blob::new(Arc::new(pixels)),
        };
        let first = crop(&raster, 1.0, 1.0, 3.0, 3.0).expect("inside the image");
        let second = crop(&raster, 1.0, 1.0, 3.0, 3.0).expect("inside the image");
        assert_eq!(first.data.id(), second.data.id());
        assert_eq!((first.width, first.height), (2, 2));
        assert_eq!(first.data.data()[..4], [20, 21, 22, 23]);
    }
}
