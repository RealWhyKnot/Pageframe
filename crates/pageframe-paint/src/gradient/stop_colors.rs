use super::*;

const BLINK_WHITE_A: f32 = 4.547_163e-5;
const BLINK_WHITE_B: f32 = 2.009_6e-5;
const BLINK_ACHROMATIC_CHROMA: f32 = 0.02;
const SKIA_POWERLESS_CHROMA: f32 = 1e-6;
const NEAR_ACHROMATIC_CHROMA: f32 = 1e-3;

pub(super) struct BlinkLch {
    lightness: f32,
    chroma: f32,
    hue: f32,
    hue_is_none: bool,
}

pub(super) fn blink_lch_from_lab(lightness: f32, axis_a: f32, axis_b: f32) -> BlinkLch {
    let chroma = axis_a.hypot(axis_b);
    BlinkLch {
        lightness,
        chroma,
        hue: axis_b.atan2(axis_a).to_degrees().rem_euclid(360.0),
        hue_is_none: chroma <= BLINK_ACHROMATIC_CHROMA,
    }
}

pub(super) fn blink_lch(color: DynamicColor) -> Option<BlinkLch> {
    let missing = color.flags.missing();
    let [lightness, first, second, _] = color.components;
    match color.cs {
        ColorSpaceTag::Oklch => {
            if missing.contains(0) || missing.contains(1) || missing.contains(3) {
                return None;
            }
            Some(BlinkLch {
                lightness,
                chroma: first,
                hue: second,
                hue_is_none: missing.contains(2),
            })
        }
        ColorSpaceTag::Oklab => missing
            .is_empty()
            .then(|| blink_lch_from_lab(lightness, first, second)),
        _ => {
            if !missing.is_empty() {
                return None;
            }
            let [lightness, axis_a, axis_b, _] = color.convert(ColorSpaceTag::Oklab).components;
            Some(blink_lch_from_lab(
                lightness,
                axis_a + lightness * BLINK_WHITE_A,
                axis_b + lightness * BLINK_WHITE_B,
            ))
        }
    }
}

pub(super) fn blink_resolved_hue(color: DynamicColor, neighbour: DynamicColor) -> DynamicColor {
    let Some(own) = blink_lch(color).filter(|own| own.hue_is_none) else {
        return color;
    };
    let Some(other) = blink_lch(neighbour) else {
        return color;
    };
    DynamicColor {
        cs: ColorSpaceTag::Oklch,
        flags: Flags::default(),
        components: [own.lightness, own.chroma, other.hue, color.components[3]],
    }
}

pub(super) fn chrome_oklch_stop(
    color: DynamicColor,
    neighbour: DynamicColor,
) -> Option<DynamicColor> {
    let own = blink_lch(color)?;
    if !own.hue_is_none && own.chroma > NEAR_ACHROMATIC_CHROMA {
        return None;
    }
    let hue = if own.hue_is_none {
        blink_lch(neighbour)?.hue
    } else {
        own.hue
    };
    let radians = hue.to_radians();
    let axis_a = own.chroma * radians.cos() - own.lightness * BLINK_WHITE_A;
    let axis_b = own.chroma * radians.sin() - own.lightness * BLINK_WHITE_B;
    let chroma = axis_a.hypot(axis_b);
    if chroma <= SKIA_POWERLESS_CHROMA {
        return None;
    }
    Some(DynamicColor {
        cs: ColorSpaceTag::Oklch,
        flags: Flags::default(),
        components: [
            own.lightness,
            chroma,
            axis_b.atan2(axis_a).to_degrees().rem_euclid(360.0),
            color.components[3],
        ],
    })
}

pub(super) fn color_space_tag(space: ColorSpace) -> ColorSpaceTag {
    match space {
        ColorSpace::Srgb => ColorSpaceTag::Srgb,
        ColorSpace::Hsl => ColorSpaceTag::Hsl,
        ColorSpace::Hwb => ColorSpaceTag::Hwb,
        ColorSpace::Lab => ColorSpaceTag::Lab,
        ColorSpace::Lch => ColorSpaceTag::Lch,
        ColorSpace::Oklab => ColorSpaceTag::Oklab,
        ColorSpace::Oklch => ColorSpaceTag::Oklch,
        ColorSpace::SrgbLinear => ColorSpaceTag::LinearSrgb,
        ColorSpace::DisplayP3 => ColorSpaceTag::DisplayP3,
        ColorSpace::A98Rgb => ColorSpaceTag::A98Rgb,
        ColorSpace::ProphotoRgb => ColorSpaceTag::ProphotoRgb,
        ColorSpace::Rec2020 => ColorSpaceTag::Rec2020,
        ColorSpace::XyzD50 => ColorSpaceTag::XyzD50,
        ColorSpace::XyzD65 | ColorSpace::DisplayP3Linear => ColorSpaceTag::XyzD65,
    }
}

pub(super) fn stop_color(color: &AbsoluteColor) -> DynamicColor {
    let srgb = color.as_dynamic_color();
    if color.color_space == ColorSpace::DisplayP3Linear {
        return srgb;
    }
    let missing = [
        ColorFlags::C0_IS_NONE,
        ColorFlags::C1_IS_NONE,
        ColorFlags::C2_IS_NONE,
        ColorFlags::ALPHA_IS_NONE,
    ]
    .into_iter()
    .enumerate()
    .filter(|(_, flag)| color.flags.contains(*flag))
    .fold(Missing::default(), |mut missing, (index, _)| {
        missing.insert(index);
        missing
    });
    if matches!(color.color_space, ColorSpace::Oklch | ColorSpace::Oklab) {
        let mut components = *color.raw_components();
        for (index, component) in components.iter_mut().enumerate() {
            if missing.contains(index) {
                *component = 0.0;
            }
        }
        return DynamicColor {
            cs: color_space_tag(color.color_space),
            flags: Flags::from_missing(missing),
            components,
        };
    }
    if missing.is_empty() {
        return srgb;
    }
    let mut own = srgb.convert(color_space_tag(color.color_space));
    for (index, component) in own.components.iter_mut().enumerate() {
        if missing.contains(index) {
            *component = 0.0;
        }
    }
    own.flags = Flags::from_missing(own.flags.missing() | missing);
    own
}

pub(super) fn interpolated(
    gradient: peniko::Gradient,
    method: &ColorInterpolationMethod,
) -> peniko::Gradient {
    let space = color_space_tag(method.space);
    let hue = match method.hue {
        HueInterpolationMethod::Shorter | HueInterpolationMethod::Specified => {
            HueDirection::Shorter
        }
        HueInterpolationMethod::Longer => HueDirection::Longer,
        HueInterpolationMethod::Increasing => HueDirection::Increasing,
        HueInterpolationMethod::Decreasing => HueDirection::Decreasing,
    };
    gradient
        .with_interpolation_cs(space)
        .with_hue_direction(hue)
}
