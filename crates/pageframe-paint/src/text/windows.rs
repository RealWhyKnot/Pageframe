use super::{Color, RunFace};
use anyrender::{Glyph, PaintScene};
use anyrender_directwrite::{FontSource, GlyphRun};
use kurbo::{Affine, Vec2};
use parley::FontData;
use peniko::Fill;
use skrifa::raw::{TableProvider, types::Tag};
use skrifa::{FontRef, MetadataProvider};

fn oblique(face: &RunFace<'_>) -> Option<bool> {
    let Some(xform) = face.glyph_xform else {
        return Some(false);
    };
    let [a, b, c, d, e, f] = xform.as_coeffs();
    (a == 1.0 && b == 0.0 && d == 1.0 && e == 0.0 && f == 0.0).then_some(c != 0.0)
}

pub(super) fn renderer(scene: &mut impl PaintScene, face: &RunFace<'_>, transform: Affine) -> bool {
    transform.is_finite()
        && transform.determinant() != 0.0
        && face.installed.is_some()
        && oblique(face).is_some()
        && anyrender_directwrite::supported(scene, face.font)
}

pub(super) fn draw(
    scene: &mut impl PaintScene,
    face: &RunFace<'_>,
    transform: Affine,
    color: Color,
    glyphs: impl Iterator<Item = Glyph> + Clone,
) -> bool {
    let bold = face.embolden != Vec2::ZERO;
    let run = GlyphRun {
        font: face.font,
        font_size: face.font_size,
        transform,
        color,
        coords: face.coords,
        bold,
        oblique: oblique(face).unwrap_or(false),
    };
    if renderer(scene, face, transform) && run.draw(scene, || source(face), glyphs.clone()) {
        return true;
    }
    if !bold {
        return false;
    }
    let [a, b, c, d, e, f] = transform.as_coeffs();
    let snap = b == 0.0 && c == 0.0 && a != 0.0 && d != 0.0;
    let glyphs = glyphs.map(|glyph| {
        if !snap {
            return glyph;
        }
        let x = ((a * f64::from(glyph.x) + e) * 4.0 + 0.5).floor() / 4.0;
        let y = anyrender_directwrite::hinted_baseline(d * f64::from(glyph.y) + f);
        Glyph {
            x: ((x - e) / a) as f32,
            y: ((y - f) / d) as f32,
            ..glyph
        }
    });
    scene.draw_glyphs(
        face.font,
        face.font_size,
        false,
        face.coords,
        face.embolden,
        Fill::NonZero,
        &anyrender::Paint::from(color),
        1.0,
        transform,
        face.glyph_xform,
        glyphs,
    );
    true
}

fn denormalize(font: &FontData, coords: &[i16]) -> Vec<([u8; 4], f32)> {
    let Ok(font) = FontRef::from_index(font.data.as_ref(), font.index) else {
        return Vec::new();
    };
    let axes = font.axes();
    axes.iter()
        .zip(coords)
        .map(|(axis, &coord)| {
            let tag = axis.tag();
            let normalized = |value: f32| {
                axes.location([(tag, value)])
                    .coords()
                    .get(axis.index())
                    .map_or(0, |c| c.to_bits())
            };
            let (mut low, mut high) = (axis.min_value(), axis.max_value());
            if normalized(axis.default_value()) == coord {
                return (tag.to_be_bytes(), axis.default_value());
            }
            for _ in 0..40 {
                let mid = (low + high) / 2.0;
                if normalized(mid) < coord {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            (tag.to_be_bytes(), high)
        })
        .collect()
}

fn source(face: &RunFace<'_>) -> Option<FontSource> {
    let font = FontRef::from_index(face.font.data.as_ref(), face.font.index).ok()?;
    if [b"COLR", b"CBDT", b"sbix", b"SVG "]
        .iter()
        .any(|tag| font.table_data(Tag::new(tag)).is_some())
    {
        return None;
    }
    let gasp = font
        .gasp()
        .ok()
        .filter(|t| t.version() <= 1 && t.num_ranges() <= 1024);
    let mut strikes = Vec::new();
    if let Ok(eblc) = font.eblc()
        && (eblc.major_version(), eblc.minor_version()) == (2, 0)
        && eblc.num_sizes() <= 1024
    {
        strikes.extend(
            eblc.bitmap_sizes()
                .iter()
                .filter(|s| {
                    s.ppem_x() == s.ppem_y()
                        && s.end_glyph_index().to_u32() >= s.start_glyph_index().to_u32() + 3
                })
                .map(|s| s.ppem_y()),
        );
        if let Some(data) = font.table_data(Tag::new(b"EBSC")) {
            let data = data.as_bytes();
            if let Some(header) = data.get(..8)
                && header[..4] == [0, 2, 0, 0]
                && let Ok(count) = <[u8; 4]>::try_from(&header[4..])
                && let count = u32::from_be_bytes(count) as usize
                && count <= 1024
                && let Some(records) = data.get(8..8 + count * 28)
            {
                strikes.extend(
                    records
                        .as_chunks::<28>()
                        .0
                        .iter()
                        .filter(|r| r[24] == r[25])
                        .map(|r| r[24]),
                );
            }
        }
    }
    let data = face.font.clone();
    Some(FontSource {
        axes: Box::new(move |coords| denormalize(&data, coords)),
        gasp_version: gasp.as_ref().map(|t| t.version()),
        ranges: gasp
            .map(|t| {
                t.gasp_ranges()
                    .iter()
                    .map(|r| (r.range_max_ppem(), r.range_gasp_behavior().bits()))
                    .collect()
            })
            .unwrap_or_default(),
        strikes,
        hinted: font
            .maxp()
            .ok()
            .and_then(|t| t.max_size_of_instructions())
            .unwrap_or(0)
            > 0,
    })
}
