use std::sync::LazyLock;

fn linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb(value: f32) -> f32 {
    if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

fn canonical(value: u8) -> u8 {
    let high = value & 0xe0;
    high | high >> 3 | high >> 6
}

static TABLES: LazyLock<[[u8; 256]; 8]> = LazyLock::new(|| {
    std::array::from_fn(|i| {
        let src = f32::from(canonical((i as u8) << 5)) / 255.0;
        let dst = 1.0 - src;
        let (s, d) = (linear(src), linear(dst));
        std::array::from_fn(|v| {
            let raw = v as f32 / 255.0;
            let alpha = raw + ((1.0 - raw) * d * raw);
            let corrected = (srgb(s * alpha + (1.0 - alpha) * d) - dst) / (src - dst);
            (255.0 * corrected).round() as u8
        })
    })
});

pub(super) fn table(r: u8, g: u8, b: u8) -> &'static [u8; 256] {
    let luminance = (54 * u32::from(canonical(r))
        + 183 * u32::from(canonical(g))
        + 19 * u32::from(canonical(b)))
        >> 8;
    &TABLES[(luminance >> 5) as usize]
}
