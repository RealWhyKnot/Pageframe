use anyrender_directwrite::hinted_baseline;
use kurbo::Affine;

/// Snaps glyph origins to the horizontal subpixel grid of the platform's native text renderer.
///
/// CoreGraphics (and so Safari and other native Apple text) floors each glyph origin's device x
/// to a grid that depends on the device font size: 1/4px at 9-11ppem, 1/3px at 12-16ppem, 1/2px
/// at 17-33ppem and whole pixels from 34ppem. Vello positions glyphs continuously, so without
/// this, stems land up to 1/2px away from native ones. Elsewhere, x is rounded to the nearest
/// 1/4px, as Skia does for horizontal text (and so Chrome off Apple). On Apple, CoreGraphics
/// also floors the device y of the baseline to a whole pixel (y up, so the baseline moves down
/// in Vello's y-down device space). Off Apple, hinted glyphs round device y half up after
/// truncating it to Chrome's 1/64px layout units.
///
/// CoreGraphics font smoothing also dilates glyphs upwards only: stems grow by the same amount
/// on both sides in x, but the bottom edge stays on the baseline and the top grows by twice
/// that. Vello's embolden grows both sides in y, so emboldened glyphs are raised by the
/// vertical embolden amount, which keeps baselines and other bottom edges as crisp as native.
#[derive(Clone, Copy)]
pub(crate) struct GlyphOriginSnap {
    scale_x: f64,
    offset_x: f64,
    positions_per_px: f64,
    /// Skia rounds to the nearest grid position; CoreGraphics floors.
    round_x: f64,
    /// The transform's y scale and offset when device y is snapped, as on Apple.
    snap_y: Option<(f64, f64)>,
    hinted_y: Option<(f64, f64)>,
    lift_y: f64,
}

impl GlyphOriginSnap {
    pub(crate) fn new(
        transform: Affine,
        font_size: f32,
        embolden: kurbo::Vec2,
        hint: bool,
    ) -> Option<Self> {
        if cfg!(any(target_os = "macos", target_os = "ios")) {
            Self::core_graphics(transform, font_size, embolden)
        } else {
            Self::skia(transform, hint)
        }
    }

    fn skia(transform: Affine, hint: bool) -> Option<Self> {
        let [a, b, c, d, e, f] = transform.as_coeffs();
        if b != 0.0 || c != 0.0 || a <= 0.0 {
            return None;
        }
        Some(Self {
            scale_x: a,
            offset_x: e,
            positions_per_px: 4.0,
            round_x: 0.5,
            snap_y: None,
            hinted_y: (hint && d == a).then_some((d, f)),
            lift_y: 0.0,
        })
    }

    fn core_graphics(transform: Affine, font_size: f32, embolden: kurbo::Vec2) -> Option<Self> {
        let [a, b, c, d, e, f] = transform.as_coeffs();
        let ppem = font_size as f64 * d.abs();
        if b != 0.0 || c != 0.0 || a <= 0.0 || !ppem.is_finite() || ppem <= 0.0 {
            return None;
        }
        Some(Self {
            scale_x: a,
            offset_x: e,
            // CoreGraphics' grid gets coarser as glyphs grow: n = ceil(100 / (3 * ppem)), at most 5.
            positions_per_px: (100.0 / (3.0 * ppem)).ceil().min(5.0),
            round_x: 0.0,
            snap_y: Some((d, f)),
            hinted_y: None,
            // glifo scales the embolden amount with the transform, so it is in user units.
            lift_y: embolden.y * d.signum(),
        })
    }

    pub(crate) fn x(self, x: f32) -> f32 {
        let device_x = self.scale_x * x as f64 + self.offset_x;
        let snapped =
            (device_x * self.positions_per_px + self.round_x).floor() / self.positions_per_px;
        ((snapped - self.offset_x) / self.scale_x) as f32
    }

    pub(crate) fn y(self, y: f32) -> f32 {
        let mut y = y as f64;
        if let Some((scale_y, offset_y)) = self.snap_y {
            // CoreGraphics tolerates about 1/1000 device px before moving to the next pixel.
            let snapped = (scale_y * y + offset_y - 1.0 / 1024.0).ceil();
            y = (snapped - offset_y) / scale_y;
        }
        if let Some((scale_y, offset_y)) = self.hinted_y {
            let whole = hinted_baseline(scale_y * y + offset_y);
            y = (whole - offset_y) / scale_y;
        }
        (y - self.lift_y) as f32
    }
}

#[cfg(test)]
mod glyph_origin_snap_tests {
    use super::GlyphOriginSnap;
    use kurbo::Affine;

    fn snapped_device_x(transform: Affine, font_size: f32, x: f32) -> f64 {
        let snap = GlyphOriginSnap::core_graphics(transform, font_size, kurbo::Vec2::ZERO).unwrap();
        transform.as_coeffs()[0] * snap.x(x) as f64 + transform.as_coeffs()[4]
    }

    #[test]
    fn grid_depends_on_device_ppem() {
        for (ppem, step) in [
            (8.0, 0.2),
            (11.0, 0.25),
            (12.0, 1.0 / 3.0),
            (16.0, 1.0 / 3.0),
            (17.0, 0.5),
            (33.0, 0.5),
            (34.0, 1.0),
            (72.0, 1.0),
        ] {
            let snap =
                GlyphOriginSnap::core_graphics(Affine::IDENTITY, ppem, kurbo::Vec2::ZERO).unwrap();
            assert_eq!(snap.positions_per_px, (1.0f64 / step).round(), "{ppem}ppem");
        }
    }

    #[test]
    fn floors_device_x() {
        assert_eq!(snapped_device_x(Affine::IDENTITY, 11.0, 10.7), 10.5);
        assert_eq!(snapped_device_x(Affine::IDENTITY, 16.0, 10.3), 10.0);
        assert_eq!(snapped_device_x(Affine::IDENTITY, 40.0, 10.9), 10.0);
        // 8px at 2x is 16 device ppem, so a 1/3 device px grid.
        let transform = Affine::translate((3.0, 0.0)) * Affine::scale(2.0);
        assert!((snapped_device_x(transform, 8.0, 5.2) - 13.333333).abs() < 1e-4);
    }

    #[test]
    fn rounds_device_x_to_quarter_px_for_skia() {
        let device_x = |transform: Affine, x: f32| {
            let snap = GlyphOriginSnap::skia(transform, false).unwrap();
            transform.as_coeffs()[0] * snap.x(x) as f64 + transform.as_coeffs()[4]
        };
        assert_eq!(device_x(Affine::IDENTITY, 10.1), 10.0);
        assert_eq!(device_x(Affine::IDENTITY, 10.2), 10.25);
        assert_eq!(device_x(Affine::IDENTITY, 10.9), 11.0);
        let transform = Affine::translate((3.0, 0.0)) * Affine::scale(2.0);
        assert_eq!(device_x(transform, 5.2), 13.5);
        assert!(GlyphOriginSnap::skia(Affine::rotate(0.5), true).is_none());
    }

    #[test]
    fn snaps_device_y_down_to_whole_px() {
        let device_y = |transform: Affine, y: f32| {
            let snap = GlyphOriginSnap::core_graphics(transform, 16.0, kurbo::Vec2::ZERO).unwrap();
            transform.as_coeffs()[3] * snap.y(y) as f64 + transform.as_coeffs()[5]
        };
        assert_eq!(device_y(Affine::IDENTITY, 10.0), 10.0);
        assert_eq!(device_y(Affine::IDENTITY, 10.0005), 10.0);
        assert_eq!(device_y(Affine::IDENTITY, 10.1), 11.0);
        assert_eq!(device_y(Affine::IDENTITY, 10.9), 11.0);
        // At scale(1.5), a baseline at y = 7 is 10.5 device px down.
        assert!((device_y(Affine::scale(1.5), 7.0) - 11.0).abs() < 1e-6);
        let transform = Affine::translate((0.0, 0.25)) * Affine::scale(2.0);
        assert!((device_y(transform, 5.0) - 11.0).abs() < 1e-6);
        let snap = GlyphOriginSnap::skia(Affine::IDENTITY, false).unwrap();
        assert_eq!(snap.y(10.3), 10.3);
    }

    #[test]
    fn rounds_hinted_device_y_from_whole_layout_units_for_skia() {
        let device_y = |offset: f64, y: f32| {
            let transform = Affine::translate((0.0, offset));
            let snap = GlyphOriginSnap::skia(transform, true).unwrap();
            (f64::from(snap.y(y)) + offset).round()
        };
        assert_eq!(device_y(110.499_999_046_325_68, 23.0), 134.0);
        assert_eq!(device_y(110.5, 23.0), 134.0);
        assert_eq!(device_y(110.48, 23.0), 133.0);
        assert_eq!(device_y(110.0, 23.4), 133.0);
        assert_eq!(device_y(-110.5, -23.0), -133.0);
        let unhinted = GlyphOriginSnap::skia(Affine::translate((0.0, 110.4)), false).unwrap();
        assert_eq!(unhinted.y(23.0), 23.0);
        let stretched = Affine::translate((0.0, 110.4)) * Affine::scale_non_uniform(1.0, 2.0);
        let unhinted = GlyphOriginSnap::skia(stretched, true).unwrap();
        assert_eq!(unhinted.y(23.0), 23.0);
    }

    #[test]
    fn raises_emboldened_glyphs_by_vertical_embolden() {
        let embolden = kurbo::Vec2::new(0.1, 0.3);
        let snap = GlyphOriginSnap::core_graphics(Affine::IDENTITY, 16.0, embolden).unwrap();
        assert!((snap.y(10.0) - 9.7).abs() < 1e-6);
        // The lift is in user units, so at 2x it is 0.6 device px.
        let snap = GlyphOriginSnap::core_graphics(Affine::scale(2.0), 8.0, embolden).unwrap();
        assert!((snap.y(10.0) - 9.7).abs() < 1e-6);
        let snap = GlyphOriginSnap::core_graphics(Affine::IDENTITY, 16.0, kurbo::Vec2::ZERO);
        assert_eq!(snap.unwrap().y(10.0), 10.0);
    }

    #[test]
    fn skips_rotated_runs() {
        assert!(
            GlyphOriginSnap::core_graphics(Affine::rotate(0.3), 16.0, kurbo::Vec2::ZERO).is_none()
        );
    }
}
