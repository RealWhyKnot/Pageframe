use crate::{memory, raster};
use anyrender::{Glyph, PaintRef, PaintScene};
use kurbo::Affine;
use num_traits::ToPrimitive as _;
use peniko::{Color, Fill, FontData};
use std::{
    cell::RefCell,
    collections::{HashMap, hash_map::Entry},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use windows::Win32::Graphics::DirectWrite::*;

const MAX_FACES: usize = 128;
const MAX_MASKS: usize = 4096;
const MAX_BYTES: usize = 16 << 20;

pub struct GlyphRun<'a> {
    pub font: &'a FontData,
    pub font_size: f32,
    pub transform: Affine,
    pub color: Color,
}

pub struct GlyphPaint {
    color: Color,
    probe: bool,
    painted: AtomicBool,
}

pub struct FontSource {
    pub path: Option<Arc<Path>>,
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

    fn rendering(&self, size: f32, aligned: bool) -> (DWRITE_RENDERING_MODE, f32, bool) {
        let rounded = (size * 64.0).round() / 64.0;
        let gdi_size = if rounded == 0.0 { 1.0 } else { rounded };
        let ppem = gdi_size
            .clamp(0.0, f32::from(u16::MAX))
            .to_u16()
            .unwrap_or(0);
        let (min, max) = match self.range(ppem) {
            Some((start, end, 1)) => (start, end),
            _ => (ppem, ppem),
        };
        if self
            .strikes
            .iter()
            .any(|s| (min..=max).contains(&u16::from(*s)))
        {
            return (
                if aligned {
                    DWRITE_RENDERING_MODE_GDI_CLASSIC
                } else {
                    DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC
                },
                gdi_size,
                true,
            );
        }
        if self.gasp_version == Some(1)
            && let Some((_, _, flags)) = self.range(
                gdi_size
                    .round()
                    .clamp(0.0, f32::from(u16::MAX))
                    .to_u16()
                    .unwrap_or(0),
            )
        {
            return (
                if flags & 8 != 0 {
                    DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC
                } else {
                    DWRITE_RENDERING_MODE_NATURAL
                },
                size,
                false,
            );
        }
        if size > 20.0 || !self.hinted {
            (DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC, size, false)
        } else {
            (DWRITE_RENDERING_MODE_NATURAL, gdi_size, false)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MaskKey {
    face: (u64, u32),
    pub(crate) ppem: u32,
    pub(crate) matrix: [u32; 4],
    pub(crate) gdi_measure: bool,
    pub(crate) mode: i32,
    pub(crate) glyph: u16,
    pub(crate) quarters: [u8; 2],
}

struct CachedFace {
    face: IDWriteFontFace,
    profile: FontSource,
    _memory_file: Option<Arc<memory::MemoryFile>>,
}

struct Cache {
    factory: IDWriteFactory,
    faces: HashMap<(u64, u32), Option<CachedFace>>,
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

pub fn supported(scene: &mut impl PaintScene, font: &FontData) -> bool {
    let paint = GlyphPaint {
        color: Color::TRANSPARENT,
        probe: true,
        painted: AtomicBool::new(false),
    };
    submit(
        scene,
        font,
        0.0,
        Affine::IDENTITY,
        &paint,
        std::iter::empty(),
    );
    paint.painted.into_inner()
}

impl GlyphRun<'_> {
    pub fn draw(
        self,
        scene: &mut impl PaintScene,
        source: impl FnOnce() -> Option<FontSource>,
        glyphs: impl Iterator<Item = Glyph> + Clone,
    ) -> bool {
        let eligible = CACHE.with_borrow_mut(|cache| {
            let Some(cache) = cache.as_mut() else {
                return false;
            };
            let face_key = (self.font.data.id(), self.font.index);
            if cache.faces.len() >= MAX_FACES && !cache.faces.contains_key(&face_key) {
                cache.faces.clear();
            }
            cache
                .faces
                .entry(face_key)
                .or_insert_with(|| {
                    let profile = source()?;
                    let (file, memory_file) = match &profile.path {
                        Some(path) => (raster::local_file(&cache.factory, path)?, None),
                        None => {
                            let memory = memory::file(&cache.factory, &self.font.data)?;
                            (memory.file.clone(), Some(memory))
                        }
                    };
                    let face = raster::open(&cache.factory, file, self.font.index)?;
                    Some(CachedFace {
                        face,
                        profile,
                        _memory_file: memory_file,
                    })
                })
                .is_some()
        });
        if !eligible {
            return false;
        }
        let paint = GlyphPaint {
            color: self.color,
            probe: false,
            painted: AtomicBool::new(false),
        };
        submit(
            scene,
            self.font,
            self.font_size,
            self.transform,
            &paint,
            glyphs,
        );
        paint.painted.into_inner()
    }
}

fn submit(
    scene: &mut impl PaintScene,
    font: &FontData,
    font_size: f32,
    transform: Affine,
    paint: &GlyphPaint,
    glyphs: impl Iterator<Item = Glyph> + Clone,
) {
    scene.draw_glyphs(
        font,
        font_size,
        true,
        &[],
        kurbo::Vec2::ZERO,
        Fill::NonZero,
        PaintRef::Custom(paint),
        1.0,
        transform,
        None,
        glyphs,
    );
}

impl GlyphPaint {
    pub fn draw(
        &self,
        scene: &mut impl PaintScene,
        font: &FontData,
        size: f32,
        transform: Affine,
        glyphs: impl Iterator<Item = Glyph>,
    ) {
        let painted = self.probe || paint(scene, font, size, transform, self.color, glyphs);
        self.painted.store(painted, Ordering::Relaxed);
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
    let [xx, yx, xy, yy] =
        [sx, b, c, sy].map(|v| (v.to_f32().unwrap_or(f32::NAN) * 1024.0 + 0.5).floor() / 1024.0);
    let baseline = xx.hypot(yx);
    let vertical = if yx == 0.0 {
        yy.abs()
    } else {
        ((xx / baseline) * yy - (yx / baseline) * xy).abs()
    };
    let size = font_size * vertical;
    if !size.is_finite() || size <= 0.0 || !baseline.is_finite() || baseline <= 0.0 {
        return false;
    }
    let matrix = [xx, yx, xy, yy].map(|v| (v / vertical).to_bits());
    let aligned = (yx == 0.0 && xy == 0.0) || (xx == 0.0 && yy == 0.0);
    let images = CACHE.with_borrow_mut(|cache| {
        let cache = cache.as_mut()?;
        let face_key = (font.data.id(), font.index);
        let cached = cache.faces.get(&face_key)?.as_ref()?;
        let (mode, ppem, gdi_measure) = cached.profile.rendering(size, aligned);
        let mut images = Vec::new();
        for glyph in glyphs {
            let gx = f64::from(glyph.x);
            let gy = f64::from(glyph.y);
            let x = sx * gx + c * gy + tx;
            let y = b * gx + sy * gy + ty;
            let x = if xx == 0.0 {
                (x + 0.5).floor() * 4.0
            } else {
                (x * 4.0 + 0.5).floor()
            };
            let y = if yx == 0.0 {
                crate::hinted_baseline(y) * 4.0
            } else {
                (y * 4.0 + 0.5).floor()
            };
            if !x.is_finite() || !y.is_finite() {
                return None;
            }
            let key = MaskKey {
                face: face_key,
                ppem: ppem.to_bits(),
                matrix,
                gdi_measure,
                mode: mode.0,
                glyph: glyph.id.try_into().ok()?,
                quarters: [x, y].map(|v| v.rem_euclid(4.0).to_u8().unwrap_or(0)),
            };
            let mask = match cache.masks.entry(key) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    let mask = raster::mask(&cache.factory, &cached.face, key)?;
                    cache.bytes += mask.coverage.len();
                    entry.insert(mask)
                }
            };
            if let Some((image, added)) = mask.image(color) {
                cache.bytes += added;
                images.push((
                    image,
                    (
                        x.div_euclid(4.0) + f64::from(mask.left),
                        y.div_euclid(4.0) + f64::from(mask.top),
                    ),
                ));
            }
            if cache.masks.len() >= MAX_MASKS || cache.bytes >= MAX_BYTES {
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
