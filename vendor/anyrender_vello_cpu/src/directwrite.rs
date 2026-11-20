mod coverage;
mod raster;

use anyrender::{Glyph, PaintRef, PaintScene};
use kurbo::Affine;
use peniko::{Color, Fill, FontData};
use std::{
    cell::RefCell,
    collections::HashMap,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use windows::Win32::Graphics::DirectWrite::*;

pub struct DirectWriteText;

pub struct GlyphRun<'a> {
    pub font: &'a FontData,
    pub font_size: f32,
    pub transform: Affine,
    pub color: Color,
}

pub(crate) struct GlyphPaint {
    color: Color,
    painted: AtomicBool,
}

pub struct FontSource {
    pub path: Arc<Path>,
    pub gasp_version: Option<u16>,
    pub ranges: Vec<(u16, u16)>,
    pub strikes: Vec<u8>,
    pub hinted: bool,
}

impl FontSource {
    fn range(&self, size: u16) -> Option<(u16, u16, u16)> {
        let mut start = 0;
        for &(end, flags) in &self.ranges {
            if size <= end {
                return Some((start, end, flags));
            }
            start = end.saturating_add(1);
        }
        None
    }

    fn rendering(&self, size: f32) -> (DWRITE_RENDERING_MODE, f32) {
        let rounded = (size * 64.0).round() / 64.0;
        let gdi_size = if rounded == 0.0 { 1.0 } else { rounded };
        let ppem = gdi_size as u16;
        let (min, max) = match self.range(ppem) {
            Some((start, end, 1)) => (start, end),
            _ => (ppem, ppem),
        };
        if self
            .strikes
            .iter()
            .any(|s| (min..=max).contains(&u16::from(*s)))
        {
            return (DWRITE_RENDERING_MODE_GDI_CLASSIC, gdi_size);
        }
        if self.gasp_version == Some(1)
            && let Some((_, _, flags)) = self.range(gdi_size.round() as u16)
        {
            return (
                if flags & 8 != 0 {
                    DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC
                } else {
                    DWRITE_RENDERING_MODE_NATURAL
                },
                size,
            );
        }
        if size > 20.0 || !self.hinted {
            (DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC, size)
        } else {
            (DWRITE_RENDERING_MODE_NATURAL, gdi_size)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct MaskKey {
    face: (u64, u32),
    ppem: u32,
    stretch: u32,
    mode: i32,
    glyph: u16,
    quarter: u8,
}

struct Cache {
    factory: IDWriteFactory,
    faces: HashMap<(u64, u32), Option<(IDWriteFontFace, FontSource)>>,
    masks: HashMap<MaskKey, raster::Mask>,
    bytes: usize,
}

thread_local! {
    static CACHE: RefCell<Option<Cache>> = {
        // SAFETY: The binding supplies the IDWriteFactory interface ID and writable storage for the returned pointer.
        let factory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) };
        RefCell::new(factory.ok().map(|factory| Cache {
            factory,
            faces: HashMap::new(),
            masks: HashMap::new(),
            bytes: 0,
        }))
    };
}

impl DirectWriteText {
    pub fn draw(
        &self,
        scene: &mut impl PaintScene,
        run: GlyphRun<'_>,
        source: impl FnOnce() -> Option<FontSource>,
        glyphs: impl Iterator<Item = Glyph> + Clone,
    ) -> bool {
        let eligible = CACHE.with_borrow_mut(|cache| {
            let Some(cache) = cache.as_mut() else {
                return false;
            };
            let face_key = (run.font.data.id(), run.font.index);
            if cache.faces.len() >= 128 && !cache.faces.contains_key(&face_key) {
                cache.faces.clear();
            }
            cache
                .faces
                .entry(face_key)
                .or_insert_with(|| {
                    let profile = source()?;
                    let face = raster::open(&cache.factory, &profile.path, run.font.index)?;
                    Some((face, profile))
                })
                .is_some()
        });
        if !eligible {
            return false;
        }
        let paint = GlyphPaint {
            color: run.color,
            painted: AtomicBool::new(false),
        };
        scene.draw_glyphs(
            run.font,
            run.font_size,
            true,
            &[],
            kurbo::Vec2::ZERO,
            Fill::NonZero,
            PaintRef::Custom(&paint),
            1.0,
            run.transform,
            None,
            glyphs,
        );
        paint.painted.into_inner()
    }
}

impl GlyphPaint {
    pub(crate) fn draw(
        &self,
        scene: &mut impl PaintScene,
        font: &FontData,
        size: f32,
        transform: Affine,
        glyphs: impl Iterator<Item = Glyph>,
    ) {
        self.painted.store(
            paint(scene, font, size, transform, self.color, glyphs),
            Ordering::Relaxed,
        );
    }
}

fn paint(
    scene: &mut impl PaintScene,
    font: &FontData,
    font_size: f32,
    transform: Affine,
    color: Color,
    glyphs: impl Iterator<Item = Glyph>,
) -> bool {
    let [sx, b, c, sy, tx, ty] = transform.as_coeffs();
    let size = (f64::from(font_size) * sy) as f32;
    let stretch = (sx / sy) as f32;
    if b != 0.0
        || c != 0.0
        || sx <= 0.0
        || sy <= 0.0
        || !size.is_finite()
        || size <= 0.0
        || !stretch.is_finite()
        || stretch <= 0.0
    {
        return false;
    }
    let images = CACHE.with_borrow_mut(|cache| {
        let cache = cache.as_mut()?;
        let face_key = (font.data.id(), font.index);
        let (face, profile) = cache.faces.get(&face_key)?.as_ref()?;
        let (mode, ppem) = profile.rendering(size);
        let mut images = Vec::new();
        for glyph in glyphs {
            let x = ((sx * f64::from(glyph.x) + tx) * 4.0 + 0.5).floor();
            let y = crate::glyph_origin::hinted_baseline(sy * f64::from(glyph.y) + ty);
            if !x.is_finite() || !y.is_finite() {
                return None;
            }
            let key = MaskKey {
                face: face_key,
                ppem: ppem.to_bits(),
                stretch: stretch.to_bits(),
                mode: mode.0,
                glyph: glyph.id.try_into().ok()?,
                quarter: x.rem_euclid(4.0) as u8,
            };
            let mask = match cache.masks.entry(key) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let mask = raster::mask(&cache.factory, face, key)?;
                    cache.bytes += mask.coverage.len() * 5;
                    entry.insert(mask)
                }
            };
            if let Some(image) = mask.image(color) {
                images.push((
                    image,
                    (
                        x.div_euclid(4.0) + f64::from(mask.left),
                        y + f64::from(mask.top),
                    ),
                ));
            }
            if cache.masks.len() >= 4096 || cache.bytes >= 16 << 20 {
                cache.masks.clear();
                cache.bytes = 0;
            }
        }
        Some(images)
    });
    let Some(images) = images else { return false };
    for (image, origin) in images {
        scene.draw_image(image.as_ref(), Affine::translate(origin));
    }
    true
}
