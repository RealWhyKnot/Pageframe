use super::*;

mod blur;

/// The largest side, in pixels, of the image a blurred shadow is rendered into. Larger
/// shadows are drawn without blur.
const MAX_SHADOW_IMAGE_SIDE: f64 = 4096.0;

/// How many bytes of blurred shadow images each thread keeps for reuse.
const SHADOW_CACHE_BYTES: usize = 32 << 20;

thread_local! {
    static SHADOW_CACHE: std::cell::RefCell<ImageCache> =
        std::cell::RefCell::new(ImageCache::new(SHADOW_CACHE_BYTES));
}

/// Renders what `draw` paints into an image, blurs it with a Gaussian of `std_deviation` and
/// tints it with `color`. Renderers without filter support, the GPU one among them, draw the
/// result like any other image. `bounds` must contain the unblurred ink.
///
/// `describe` hashes everything `draw` depends on, with positions taken relative to the image
/// origin it is given. Images with the same hash are reused from a per-thread cache, so a
/// repaint or scroll doesn't render the shadow again.
///
/// Returns the image and the offset to draw it at.
pub(super) fn blurred_shadow_image(
    bounds: Rect,
    std_deviation: f64,
    color: Color,
    describe: impl FnOnce(&mut DefaultHasher, kurbo::Vec2),
    draw: impl FnOnce(&mut anyrender_vello_cpu::VelloCpuScenePainter, Affine),
) -> Option<(peniko::ImageBrush, kurbo::Vec2)> {
    let bounds = bounds
        .inflate(3.0 * std_deviation, 3.0 * std_deviation)
        .expand();
    if !(1.0..=MAX_SHADOW_IMAGE_SIDE).contains(&bounds.width())
        || !(1.0..=MAX_SHADOW_IMAGE_SIDE).contains(&bounds.height())
    {
        return None;
    }
    let origin = kurbo::Vec2::new(bounds.x0, bounds.y0);
    let (width, height) = (bounds.width() as u32, bounds.height() as u32);

    let mut hasher = DefaultHasher::new();
    describe(&mut hasher, origin);
    (width, height).hash(&mut hasher);
    std_deviation.to_bits().hash(&mut hasher);
    color.to_rgba8().to_u8_array().hash(&mut hasher);
    let key = hasher.finish();
    if let Some(image) = SHADOW_CACHE.with_borrow_mut(|cache| cache.get(key)) {
        return Some((peniko::ImageBrush::new(image), origin));
    }

    let coverage = anyrender::render_to_buffer::<anyrender_vello_cpu::VelloCpuImageRenderer, _>(
        |scene| draw(scene, Affine::translate(-origin)),
        width,
        height,
    );
    let mut alpha: Vec<u8> = coverage.as_chunks::<4>().0.iter().map(|px| px[3]).collect();
    blur::mask_blur(&mut alpha, width as usize, height as usize, std_deviation);

    let [r, g, b, a] = color.to_rgba8().to_u8_array();
    let data: Vec<u8> = alpha
        .iter()
        .flat_map(|&coverage| {
            [
                r,
                g,
                b,
                (f32::from(coverage) * f32::from(a) / 255.0).round() as u8,
            ]
        })
        .collect();
    let image = peniko::ImageBrush {
        image: peniko::ImageData {
            data: peniko::Blob::new(Arc::new(data)),
            format: peniko::ImageFormat::Rgba8,
            alpha_type: peniko::ImageAlphaType::Alpha,
            width,
            height,
        },
        sampler: peniko::ImageSampler::default(),
    };
    SHADOW_CACHE.with_borrow_mut(|cache| cache.insert(key, image.image.clone()));
    Some((image, origin))
}

pub(crate) fn gaussian_blur(values: &mut [f32], width: usize, height: usize, std_deviation: f64) {
    let size = (std_deviation * 3.0 * (2.0 * std::f64::consts::PI).sqrt() / 4.0 + 0.5)
        .to_usize()
        .unwrap_or(0);
    if size < 2 {
        return;
    }
    let boxes = if size % 2 == 1 {
        [(size, size / 2); 3]
    } else {
        [(size, size / 2), (size, size / 2 - 1), (size + 1, size / 2)]
    };
    let mut scratch = vec![0.0; width.max(height)];
    for (len, left) in boxes {
        for row in values.chunks_exact_mut(width) {
            box_blur_line(row, 1, &mut scratch[..width], len, left);
        }
    }
    for (len, left) in boxes {
        for x in 0..width {
            box_blur_line(&mut values[x..], width, &mut scratch[..height], len, left);
        }
    }
}

fn box_blur_line(values: &mut [f32], stride: usize, out: &mut [f32], len: usize, left: usize) {
    let count = out.len();
    let at = |shifted: usize| -> f32 {
        shifted
            .checked_sub(left)
            .filter(|&index| index < count)
            .map_or(0.0, |index| values[index * stride])
    };
    let scale = len.to_f32().unwrap_or(1.0);
    let mut sum: f32 = (0..len).map(at).sum();
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = sum / scale;
        sum += at(i + len) - at(i);
    }
    for (i, value) in out.iter().enumerate() {
        values[i * stride] = *value;
    }
}
