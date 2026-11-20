use std::sync::{Arc, Mutex};

use crate::stylo_to_parley;
use app_units::Au;
use parley::{FontContext, FontVariation};
use skrifa::MetadataProvider as _;
use skrifa::charmap::Charmap;
use style::properties::style_structs::Font as FontStyles;
use style::{
    device::servo::FontMetricsProvider,
    font_metrics::FontMetrics,
    values::computed::{CSSPixelLength, font::QueryFontMetricsFlags},
};

/// Query fontique for the fonts matching `font_styles` (family list and attributes).
fn query_for_font_styles<'a>(
    font_ctx: &'a mut FontContext,
    font_styles: &'a FontStyles,
) -> parley::fontique::Query<'a> {
    use parley::fontique::Attributes;

    let mut query = font_ctx.collection.query(&mut font_ctx.source_cache);
    let families = font_styles.font_family.families.iter().map(|family| {
        stylo_to_parley::query_font_family(
            family,
            &font_styles._x_lang.0,
            font_styles.font_family.is_initial,
        )
    });
    query.set_families(families);
    query.set_attributes(Attributes {
        width: stylo_to_parley::font_width(font_styles.font_width),
        weight: stylo_to_parley::font_weight(font_styles.font_weight),
        style: stylo_to_parley::font_style(font_styles.font_style),
    });
    query
}

impl crate::BaseDocument {
    pub fn inline_font_ascent_descent(&self, node_id: crate::NodeId) -> Option<(f32, f32)> {
        let extent = self.primary_font_extent(self.get_node(node_id)?)?;
        Some((extent.ascent, extent.descent))
    }
}

fn find_font_for(
    query: &mut parley::fontique::Query,
    ch: char,
) -> Option<parley::fontique::QueryFont> {
    use parley::fontique::{QueryFont, QueryStatus};

    let mut font = None;
    query.matches_with(|q_font: &QueryFont| {
        let Ok(font_ref) = skrifa::FontRef::from_index(q_font.blob.as_ref(), q_font.index) else {
            return QueryStatus::Continue;
        };

        if font_ref.charmap().map(ch).is_some() {
            font = Some(q_font.clone());
            QueryStatus::Stop
        } else {
            QueryStatus::Continue
        }
    });
    font
}

pub(crate) fn text_advances(
    font_ctx: &mut FontContext,
    font_styles: &FontStyles,
    font_size: f32,
    texts: &[&str],
) -> Vec<f32> {
    use skrifa::instance::{LocationRef, Size};
    use skrifa::metrics::{GlyphMetrics, Metrics};

    let variations = stylo_to_parley::font_variations(font_styles, font_size);
    let mut query = query_for_font_styles(font_ctx, font_styles);
    let mut fonts: Vec<(char, Option<parley::fontique::QueryFont>)> = Vec::new();
    texts
        .iter()
        .map(|text| {
            text.chars()
                .map(|ch| {
                    let font = match fonts.iter().find(|(known, _)| *known == ch) {
                        Some((_, font)) => font.clone(),
                        None => {
                            let font = find_font_for(&mut query, ch);
                            fonts.push((ch, font.clone()));
                            font
                        }
                    };
                    let Some(font) = font else {
                        return 0.0;
                    };
                    let Ok(font_ref) = skrifa::FontRef::from_index(font.blob.as_ref(), font.index)
                    else {
                        return 0.0;
                    };
                    let location = font_ref.axes().location(variations.iter().map(|variation| {
                        (
                            skrifa::Tag::from_be_bytes(variation.tag.to_bytes()),
                            variation.value,
                        )
                    }));
                    let location_ref = LocationRef::from(&location);
                    let units_per_em =
                        Metrics::new(&font_ref, Size::unscaled(), location_ref).units_per_em;
                    let advance = Charmap::new(&font_ref).map(ch).and_then(|glyph_id| {
                        GlyphMetrics::new(&font_ref, Size::unscaled(), location_ref)
                            .advance_width(glyph_id)
                    });
                    match (advance, units_per_em) {
                        (Some(advance), 1..) => advance * font_size / f32::from(units_per_em),
                        _ => 0.0,
                    }
                })
                .sum()
        })
        .collect()
}

pub(crate) struct FormControlMetrics {
    pub(crate) avg_char_width: f32,
    pub(crate) max_char_width: f32,
    pub(crate) normal_line_height: f32,
    pub(crate) ascent: f32,
    pub(crate) descent: f32,
}

impl FormControlMetrics {
    pub(crate) fn normal_baseline(&self) -> f32 {
        let ascent = self.ascent.round();
        ascent + ((self.normal_line_height - ascent - self.descent.round()) / 2.0).floor()
    }
}

pub(crate) fn form_control_metrics(
    font_ctx: &mut FontContext,
    font_styles: &FontStyles,
    font_size: f32,
    scale: f32,
) -> Option<FormControlMetrics> {
    use skrifa::instance::{LocationRef, Size};
    use skrifa::metrics::{GlyphMetrics, Metrics};

    let variations = stylo_to_parley::font_variations(font_styles, font_size);
    let mut query = query_for_font_styles(font_ctx, font_styles);
    let font = find_font_for(&mut query, 'x')?;
    let font_ref = skrifa::FontRef::from_index(font.blob.as_ref(), font.index).ok()?;
    let location = font_ref.axes().location(
        variations
            .iter()
            .map(|v| (skrifa::Tag::from_be_bytes(v.tag.to_bytes()), v.value)),
    );
    let location_ref = LocationRef::from(&location);
    let device_size = font_size * scale;
    let metrics = Metrics::new(&font_ref, Size::new(device_size), location_ref);
    let unscaled = Metrics::new(&font_ref, Size::unscaled(), location_ref);
    if unscaled.units_per_em == 0 {
        return None;
    }
    let glyph_id = Charmap::new(&font_ref).map('x')?;
    let advance =
        GlyphMetrics::new(&font_ref, Size::unscaled(), location_ref).advance_width(glyph_id)?;
    let x_advance = advance * device_size / f32::from(unscaled.units_per_em);
    let avg_char_width = x_advance.max(x_advance.round());
    let max_char_width = metrics
        .bounds
        .map(|bounds| (bounds.x_max - bounds.x_min).round())
        .unwrap_or(avg_char_width);
    let px_per_unit = device_size / f32::from(unscaled.units_per_em);
    let vertical = cfg!(windows)
        .then(|| dwrite_vertical_metrics(&font_ref, px_per_unit))
        .flatten()
        .unwrap_or(VerticalMetrics {
            ascent: metrics.ascent,
            descent: -metrics.descent,
            line_gap: metrics.leading,
        });
    let normal_line_height =
        vertical.ascent.round() + vertical.descent.round() + vertical.line_gap.round();

    Some(FormControlMetrics {
        avg_char_width,
        max_char_width,
        normal_line_height,
        ascent: vertical.ascent,
        descent: vertical.descent,
    })
}

struct VerticalMetrics {
    ascent: f32,
    descent: f32,
    line_gap: f32,
}

fn dwrite_vertical_metrics(
    font_ref: &skrifa::FontRef,
    px_per_unit: f32,
) -> Option<VerticalMetrics> {
    use skrifa::raw::TableProvider as _;
    use skrifa::raw::tables::os2::SelectionFlags;

    let os2 = font_ref.os2().ok()?;
    if os2
        .fs_selection()
        .contains(SelectionFlags::USE_TYPO_METRICS)
    {
        return None;
    }
    let ascent = f32::from(os2.us_win_ascent());
    let descent = f32::from(os2.us_win_descent());
    if ascent + descent <= 0.0 {
        return None;
    }
    let hhea = font_ref.hhea().ok()?;
    let hhea_height = f32::from(hhea.ascender().to_i16()) - f32::from(hhea.descender().to_i16())
        + f32::from(hhea.line_gap().to_i16());
    Some(VerticalMetrics {
        ascent: ascent * px_per_unit,
        descent: descent * px_per_unit,
        line_gap: (hhea_height - ascent - descent).max(0.0) * px_per_unit,
    })
}

#[derive(Clone)]
pub(crate) struct PageframeFontMetricsProvider {
    pub(crate) font_ctx: Arc<Mutex<FontContext>>,
}

impl core::fmt::Debug for PageframeFontMetricsProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PageframeFontMetricsProvider")
    }
}

impl FontMetricsProvider for PageframeFontMetricsProvider {
    fn query_font_metrics(
        &self,
        _vertical: bool,
        font_styles: &FontStyles,
        font_size: CSSPixelLength,
        _flags: QueryFontMetricsFlags,
    ) -> FontMetrics {
        use parley::fontique::Query;
        use skrifa::instance::{LocationRef, Size};
        use skrifa::metrics::{GlyphMetrics, Metrics};

        // Lock font_ctx. Explicit reborrow required for borrow checker.
        let mut font_ctx = self.font_ctx.lock().unwrap();
        let font_ctx = &mut *font_ctx;

        // Query fontique for the font that matches the font styles
        let mut query = query_for_font_styles(font_ctx, font_styles);
        // let fb_script = crate::swash_convert::script_to_fontique(script);
        // let fb_language = locale.and_then(crate::swash_convert::locale_to_fontique);
        // query.set_fallbacks(fontique::FallbackKey::new(fb_script, fb_language.as_ref()));

        let variations = stylo_to_parley::font_variations(font_styles, font_size.px());
        // let features = self.rcx.features(style.font_features).unwrap_or(&[]);

        // fn name_of(font_ref: &skrifa::FontRef) -> String {
        //     use skrifa::string::StringId;
        //     font_ref
        //         .localized_strings(StringId::POSTSCRIPT_NAME)
        //         .english_or_first()
        //         .unwrap()
        //         .chars()
        //         .collect()
        // }

        /// Scales the advance the same way as Parley's shaped glyph advances (font units × size / upem)
        /// rather than with skrifa's FreeType-compatible fixed-point scale, so that `ch`/`ic`
        /// lengths exactly match the width of the corresponding laid-out text.
        fn advance_of(
            query: &mut Query,
            ch: char,
            font_size: f32,
            variations: &[FontVariation],
        ) -> Option<f32> {
            let font = find_font_for(query, ch)?;
            let font_ref = skrifa::FontRef::from_index(font.blob.as_ref(), font.index).ok()?;
            let location = font_ref.axes().location(
                variations
                    .iter()
                    .map(|v| (skrifa::Tag::from_be_bytes(v.tag.to_bytes()), v.value)),
            );
            let location_ref = LocationRef::from(&location);
            let upem = Metrics::new(&font_ref, Size::unscaled(), location_ref).units_per_em;
            if upem == 0 {
                return None;
            }
            let glyph_metrics = GlyphMetrics::new(&font_ref, Size::unscaled(), location_ref);
            let char_map = Charmap::new(&font_ref);
            let glyph_id = char_map.map(ch)?;
            let advance = glyph_metrics.advance_width(glyph_id)?;
            Some(advance * (font_size / upem as f32))
        }

        fn metrics_of(
            query: &mut Query,
            ch: char,
            font_size: Size,
            variations: &[FontVariation],
        ) -> Option<(f32, Option<f32>, Option<f32>)> {
            let font = find_font_for(query, ch)?;
            let font_ref = skrifa::FontRef::from_index(font.blob.as_ref(), font.index).ok()?;
            let location = font_ref.axes().location(
                variations
                    .iter()
                    .map(|v| (skrifa::Tag::from_be_bytes(v.tag.to_bytes()), v.value)),
            );
            let location_ref = LocationRef::from(&location);
            let metrics = Metrics::new(&font_ref, font_size, location_ref);
            Some((metrics.ascent, metrics.x_height, metrics.cap_height))
        }

        let zero_advance = advance_of(&mut query, '0', font_size.px(), &variations);
        let ic_advance = advance_of(&mut query, '\u{6C34}', font_size.px(), &variations);
        let (ascent, x_height, cap_height) =
            metrics_of(&mut query, ' ', Size::new(font_size.px()), &variations)
                .unwrap_or((0.0, None, None));

        FontMetrics {
            ascent: CSSPixelLength::new(ascent),
            x_height: x_height.filter(|xh| *xh != 0.0).map(CSSPixelLength::new),
            cap_height: cap_height.map(CSSPixelLength::new),
            zero_advance_measure: zero_advance.map(CSSPixelLength::new),
            ic_width: ic_advance.map(CSSPixelLength::new),
            script_percent_scale_down: None,
            script_script_percent_scale_down: None,
        }
    }

    fn base_size_for_generic(
        &self,
        generic: style::values::computed::font::GenericFontFamily,
    ) -> style::values::computed::Length {
        let size = match generic {
            style::values::computed::font::GenericFontFamily::Monospace => 13.0,
            _ => 16.0,
        };
        style::values::computed::Length::from(Au::from_f32_px(size))
    }
}
