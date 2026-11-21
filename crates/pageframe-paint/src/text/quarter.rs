use anyrender::Glyph;
use kurbo::Affine;

const STEPS: f64 = 4.0;
const NUDGE: f64 = 1.0 / 64.0;

pub(super) fn snap(
    transform: Affine,
    glyphs: impl Iterator<Item = Glyph> + Clone,
) -> impl Iterator<Item = Glyph> + Clone {
    let [a, b, c, _, e, _] = transform.as_coeffs();
    let level = b == 0.0 && c == 0.0 && a != 0.0 && a.is_finite() && e.is_finite();
    glyphs.map(move |glyph| {
        if !level {
            return glyph;
        }
        let x = ((a * f64::from(glyph.x) + e) * STEPS + 0.5).floor() / STEPS + NUDGE;
        let x = ((x - e) / a) as f32;
        if x.is_finite() {
            Glyph { x, ..glyph }
        } else {
            glyph
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placed(scale: f64, origin: f64, x: f32) -> (f64, u8) {
        let transform = Affine::scale(scale).then_translate((origin, 0.0).into());
        let glyph = snap(transform, std::iter::once(Glyph { id: 1, x, y: 0.0 }))
            .next()
            .expect("one glyph");
        let device = transform * kurbo::Point::new(f64::from(glyph.x), 0.0);
        let fract = (device.x - device.x.floor()) as f32;
        let bucket = ((fract * 4.0).round() as u8).min(3);
        (device.x.floor(), bucket)
    }

    #[test]
    fn near_whole_pixels_carry_to_the_next_pixel() {
        for scale in [1.0, 1.5, 1.75, 2.0] {
            let x = (10.9 / scale) as f32;
            assert_eq!(placed(scale, 0.0, x), (11.0, 0), "scale {scale}");
            let x = (10.3 / scale) as f32;
            assert_eq!(placed(scale, 0.0, x), (10.0, 1), "scale {scale}");
            let x = (10.0 / scale) as f32;
            assert_eq!(placed(scale, 0.0, x), (10.0, 0), "scale {scale}");
            let x = (7.62 / scale) as f32;
            assert_eq!(placed(scale, 3.13, x), (10.0, 3), "scale {scale}");
        }
    }

    #[test]
    fn boundaries_round_half_quarters_up() {
        assert_eq!(placed(1.0, 0.0, 0.875), (1.0, 0));
        assert_eq!(placed(1.0, 0.0, -0.125), (0.0, 0));
        assert_eq!(placed(1.0, 0.0, -0.2), (-1.0, 3));
        assert_eq!(placed(1.0, -3.5, 0.0), (-4.0, 2));
    }

    #[test]
    fn extreme_inputs_stay_finite() {
        let tiny = Affine::scale(1e-300);
        for x in [0.0, 1.0, f32::MAX, -f32::MAX] {
            let glyph = Glyph { id: 1, x, y: 0.0 };
            let out = snap(tiny, std::iter::once(glyph))
                .next()
                .expect("one glyph");
            assert!(out.x.is_finite(), "{x}");
        }
        let glyph = Glyph {
            id: 1,
            x: f32::INFINITY,
            y: 0.0,
        };
        let out = snap(Affine::IDENTITY, std::iter::once(glyph))
            .next()
            .expect("one glyph");
        assert!(!out.x.is_nan());
    }

    #[test]
    fn rotated_runs_keep_their_positions() {
        let transform = Affine::rotate(0.5);
        let glyph = Glyph {
            id: 1,
            x: 3.3,
            y: 1.0,
        };
        let out = snap(transform, std::iter::once(glyph))
            .next()
            .expect("one glyph");
        assert_eq!((out.x, out.y), (glyph.x, glyph.y));
    }
}
