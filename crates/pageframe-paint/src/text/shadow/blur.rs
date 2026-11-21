const NO_BLUR_SIGMA: f64 = 1.0 / 3.0;
const SMALL_SIGMA: f64 = 2.0;
const MAX_SIGMA: f64 = 135.0;
const GOOD_ENOUGH: f64 = 0.01;
const MAX_TAPS: usize = 6;

pub(super) fn mask_blur(mask: &mut [u8], width: usize, height: usize, sigma: f64) {
    if sigma < NO_BLUR_SIGMA || width == 0 || height == 0 {
        return;
    }
    if sigma < SMALL_SIGMA {
        let kernel = gauss_kernel(sigma);
        columns(mask, width, height, |line, out| {
            gauss_line(line, out, &kernel)
        });
        rows(mask, width, |line, out| gauss_line(line, out, &kernel));
    } else {
        let plan = BoxPlan::new(sigma.min(MAX_SIGMA));
        rows(mask, width, |line, out| plan.blur(line, out));
        columns(mask, width, height, |line, out| plan.blur(line, out));
    }
}

fn rows(mask: &mut [u8], width: usize, blur: impl Fn(&[u8], &mut [u8])) {
    let mut out = vec![0; width];
    for row in mask.chunks_exact_mut(width) {
        blur(row, &mut out);
        row.copy_from_slice(&out);
    }
}

fn columns(mask: &mut [u8], width: usize, height: usize, blur: impl Fn(&[u8], &mut [u8])) {
    let mut line = vec![0; height];
    let mut out = vec![0; height];
    for x in 0..width {
        for (y, value) in line.iter_mut().enumerate() {
            *value = mask[y * width + x];
        }
        blur(&line, &mut out);
        for (y, value) in out.iter().enumerate() {
            mask[y * width + x] = *value;
        }
    }
}

fn bessel(t: f64, first: f64, next: impl Fn(f64) -> f64) -> f64 {
    let quarter = t * t / 4.0;
    let (mut sum, mut factor, mut k) = (first, first, 1.0);
    while factor > 1e-6 {
        factor *= quarter / next(k);
        sum += factor;
        k += 1.0;
    }
    sum
}

fn gauss_kernel(sigma: f64) -> Vec<u32> {
    let variance = sigma * sigma;
    let scale = variance.exp();
    let mut bessels = [0.0; MAX_TAPS];
    bessels[0] = bessel(variance, 1.0, |k| k * k);
    bessels[1] = bessel(variance, variance / 2.0, |k| k * (k + 1.0));
    let mut taps = vec![bessels[0] / scale, bessels[1] / scale];
    let mut n = 1;
    while taps[n] > GOOD_ENOUGH && n + 1 < MAX_TAPS {
        bessels[n + 1] = -(2.0 * n as f64 / variance) * bessels[n] + bessels[n - 1];
        taps.push(bessels[n + 1] / scale);
        n += 1;
    }
    taps.truncate(n);
    let sides = |taps: &[f64]| taps.iter().skip(1).rev().map(|tap| 2.0 * tap).sum::<f64>();
    let total = sides(&taps) + taps[0];
    for tap in &mut taps {
        *tap /= total;
    }
    taps[0] = 1.0 - sides(&taps);
    taps.iter()
        .map(|tap| (tap * 65536.0).round() as u32)
        .collect()
}

fn gauss_line(line: &[u8], out: &mut [u8], kernel: &[u32]) {
    let radius = kernel.len() as isize - 1;
    let len = line.len() as isize;
    for (i, slot) in out.iter_mut().enumerate() {
        let i = i as isize;
        let sum: u32 = (-radius..=radius)
            .filter_map(|offset| {
                let at = i + offset;
                (0..len).contains(&at).then(|| {
                    ((u32::from(line[at as usize]) << 8) * kernel[offset.unsigned_abs()]) >> 16
                })
            })
            .sum();
        *slot = ((sum + 0x80) >> 8).min(255) as u8;
    }
}

struct BoxPlan {
    windows: [usize; 3],
    border: usize,
    weight: u64,
}

impl BoxPlan {
    fn new(sigma: f64) -> Self {
        let window = ((sigma * 3.0 * (2.0 * std::f64::consts::PI).sqrt() / 4.0 + 0.5).floor()
            as usize)
            .max(1);
        let odd = window % 2 == 1;
        let border = if odd {
            3 * ((window - 1) / 2)
        } else {
            3 * (window / 2) - 1
        };
        let cube = (window * window * window) as f64;
        let divisor = if odd {
            cube
        } else {
            cube + (window * window) as f64
        };
        Self {
            windows: [window, window, if odd { window } else { window + 1 }],
            border,
            weight: (4_294_967_296.0 / divisor).round() as u64,
        }
    }

    fn blur(&self, line: &[u8], out: &mut [u8]) {
        let mut sums: Vec<u64> = line.iter().map(|&value| u64::from(value)).collect();
        sums.resize(line.len() + 2 * self.border, 0);
        for window in self.windows {
            let source = sums.clone();
            let mut running = 0;
            for (i, slot) in sums.iter_mut().enumerate() {
                running += source[i];
                if i >= window {
                    running -= source[i - window];
                }
                *slot = running;
            }
        }
        for (slot, &sum) in out.iter_mut().zip(&sums[self.border..]) {
            *slot = ((self.weight * sum + (1 << 31)) >> 32).min(255) as u8;
        }
    }
}
