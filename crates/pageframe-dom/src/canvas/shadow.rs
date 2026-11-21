use super::raster::Mask;

const MAX_SIGMA: f64 = 512.0;

pub(crate) fn shadow_mask(
    mask: &Mask,
    blur: f64,
    offset: (f64, f64),
    width: usize,
    height: usize,
) -> Option<Mask> {
    let sigma = (blur / 2.0).min(MAX_SIGMA);
    let radius = if sigma > 0.0 {
        (sigma * 3.0).ceil() as usize
    } else {
        0
    };
    let limit = (width + height + radius) as f64;
    let shift_x = offset.0.clamp(-limit, limit).round() as i64;
    let shift_y = offset.1.clamp(-limit, limit).round() as i64;
    let x0 = (mask.x as i64 + shift_x - radius as i64).clamp(0, width as i64) as usize;
    let y0 = (mask.y as i64 + shift_y - radius as i64).clamp(0, height as i64) as usize;
    let x1 =
        (mask.x as i64 + mask.w as i64 + shift_x + radius as i64).clamp(0, width as i64) as usize;
    let y1 =
        (mask.y as i64 + mask.h as i64 + shift_y + radius as i64).clamp(0, height as i64) as usize;
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let (w, h) = (x1 - x0, y1 - y0);
    let mut cov = vec![0.0f32; w * h];
    for row in 0..h {
        for col in 0..w {
            let sx = (x0 + col) as i64 - shift_x;
            let sy = (y0 + row) as i64 - shift_y;
            if sx >= 0 && sy >= 0 {
                cov[row * w + col] = mask.at(sx as usize, sy as usize);
            }
        }
    }
    if radius > 0 {
        let kernel = gaussian(sigma, radius);
        cov = convolve(&cov, w, h, &kernel, true);
        cov = convolve(&cov, w, h, &kernel, false);
    }
    Some(Mask {
        x: x0,
        y: y0,
        w,
        h,
        cov,
    })
}

fn gaussian(sigma: f64, radius: usize) -> Vec<f32> {
    let weights: Vec<f64> = (0..=2 * radius)
        .map(|i| {
            let d = i as f64 - radius as f64;
            (-(d * d) / (2.0 * sigma * sigma)).exp()
        })
        .collect();
    let sum: f64 = weights.iter().sum();
    weights.iter().map(|w| (w / sum) as f32).collect()
}

fn convolve(src: &[f32], w: usize, h: usize, kernel: &[f32], horizontal: bool) -> Vec<f32> {
    let radius = (kernel.len() / 2) as i64;
    let mut out = vec![0.0; w * h];
    for row in 0..h {
        for col in 0..w {
            let mut sum = 0.0;
            for (k, weight) in kernel.iter().enumerate() {
                let offset = k as i64 - radius;
                let (x, y) = if horizontal {
                    (col as i64 + offset, row as i64)
                } else {
                    (col as i64, row as i64 + offset)
                };
                if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
                    sum += src[y as usize * w + x as usize] * weight;
                }
            }
            out[row * w + col] = sum;
        }
    }
    out
}
