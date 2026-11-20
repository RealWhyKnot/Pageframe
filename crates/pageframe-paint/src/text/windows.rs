use super::{Color, RunFace};
use anyrender::{Glyph, PaintScene};
use anyrender_vello_cpu::directwrite::{DirectWriteText, FontSource, GlyphRun};
use kurbo::{Affine, Vec2};
use parley::fontique::SourceKind;
use skrifa::raw::{TableProvider, types::Tag};
use skrifa::string::StringId;
use skrifa::{FontRef, MetadataProvider};

pub(super) fn renderer(
    scene: &impl PaintScene,
    face: &RunFace<'_>,
    transform: Affine,
) -> Option<Box<DirectWriteText>> {
    if !transform.is_finite() || transform.determinant() == 0.0 {
        return None;
    }
    if face.installed.is_none()
        || face.glyph_xform.is_some_and(|t| t != Affine::IDENTITY)
        || face.embolden != Vec2::ZERO
        || face.coords.iter().any(|&coord| coord != 0)
    {
        return None;
    }
    scene.renderer_specific_context()?.downcast().ok()
}

pub(super) fn draw(
    scene: &mut impl PaintScene,
    face: &RunFace<'_>,
    transform: Affine,
    color: Color,
    glyphs: impl Iterator<Item = Glyph> + Clone,
) -> bool {
    let Some(native) = renderer(scene, face, transform) else {
        return false;
    };
    native.draw(
        scene,
        GlyphRun {
            font: face.font,
            font_size: face.font_size,
            transform,
            color,
        },
        || source(face),
        glyphs,
    )
}

fn source(face: &RunFace<'_>) -> Option<FontSource> {
    let font = FontRef::from_index(face.font.data.as_ref(), face.font.index).ok()?;
    if [b"COLR", b"CBDT", b"sbix", b"SVG "]
        .iter()
        .any(|tag| font.table_data(Tag::new(tag)).is_some())
    {
        return None;
    }
    let context = face.installed?.font_context();
    let mut context = context.lock().ok()?;
    let path = [StringId::TYPOGRAPHIC_FAMILY_NAME, StringId::FAMILY_NAME]
        .into_iter()
        .find_map(|id| {
            let name = font.localized_strings(id).english_or_first()?.to_string();
            let family = context.collection.family_by_name(&name)?;
            family.fonts().iter().find_map(|info| {
                let SourceKind::Path(path) = info.source().kind() else {
                    return None;
                };
                (info.index() == face.font.index
                    && info.load(Some(&mut context.source_cache))?.id() == face.font.data.id())
                .then(|| path.clone())
            })
        });
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
    Some(FontSource {
        path,
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
