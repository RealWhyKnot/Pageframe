use crate::{coverage, glyphs::MaskKey};
use peniko::{Blob, Color, ImageAlphaType, ImageBrush, ImageData, ImageFormat, ImageQuality};
use std::{mem::ManuallyDrop, os::windows::ffi::OsStrExt as _, path::Path, sync::Arc};
use windows::{
    Win32::Graphics::DirectWrite::*,
    core::{BOOL, PCWSTR},
};

pub(super) fn local_file(factory: &IDWriteFactory, path: &Path) -> Option<IDWriteFontFile> {
    let name: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: The UTF-16 path is NUL-terminated and remains live throughout the call.
    unsafe { factory.CreateFontFileReference(PCWSTR(name.as_ptr()), None) }.ok()
}

pub(super) fn open(
    factory: &IDWriteFactory,
    file: IDWriteFontFile,
    index: u32,
) -> Option<IDWriteFontFace> {
    // SAFETY: The outputs and font file remain live during analysis and the face index is checked before use.
    unsafe {
        let mut supported = BOOL::default();
        let mut face_type = DWRITE_FONT_FACE_TYPE_UNKNOWN;
        let mut count = 0;
        file.Analyze(
            &mut supported,
            &mut DWRITE_FONT_FILE_TYPE::default(),
            Some(&mut face_type),
            &mut count,
        )
        .ok()?;
        if !supported.as_bool() || index >= count {
            return None;
        }
        factory
            .CreateFontFace(
                face_type,
                &[Some(file)],
                index,
                DWRITE_FONT_SIMULATIONS_NONE,
            )
            .ok()
    }
}

pub(super) struct Mask {
    pub(super) left: i32,
    pub(super) top: i32,
    pub(super) coverage: Box<[u8]>,
    width: u32,
    height: u32,
    tinted: Vec<([u8; 4], ImageBrush)>,
}

const MAX_TINTS: usize = 8;

impl Mask {
    pub(super) fn image(&mut self, color: Color) -> Option<(ImageBrush, usize)> {
        if self.coverage.is_empty() {
            return None;
        }
        let rgba = color.to_rgba8().to_u8_array();
        if let Some(index) = self.tinted.iter().position(|(key, _)| *key == rgba) {
            self.tinted[..=index].rotate_right(1);
            return Some((self.tinted[0].1.clone(), 0));
        }
        let [r, g, b, alpha] = rgba;
        let table = coverage::table(r, g, b);
        let pixels: Vec<_> = self
            .coverage
            .iter()
            .flat_map(|&v| {
                let a = (u16::from(table[usize::from(v)]) * u16::from(alpha) + 127) / 255;
                [r, g, b, u8::try_from(a).unwrap_or(u8::MAX)]
            })
            .collect();
        let image = ImageBrush::new(ImageData {
            data: Blob::new(Arc::new(pixels)),
            format: ImageFormat::Rgba8,
            alpha_type: ImageAlphaType::Alpha,
            width: self.width,
            height: self.height,
        })
        .with_quality(ImageQuality::Low);
        let added = if self.tinted.len() < MAX_TINTS {
            self.coverage.len() * 4
        } else {
            self.tinted.pop();
            0
        };
        self.tinted.insert(0, (rgba, image.clone()));
        Some((image, added))
    }
}

pub(super) fn mask(factory: &IDWriteFactory, face: &IDWriteFontFace, key: MaskKey) -> Option<Mask> {
    let mode = DWRITE_RENDERING_MODE(key.mode);
    let measure = if key.gdi_measure {
        DWRITE_MEASURING_MODE_GDI_CLASSIC
    } else {
        DWRITE_MEASURING_MODE_NATURAL
    };
    let [m11, m12, m21, m22] = key.matrix.map(f32::from_bits);
    let [dx, dy] = key.quarters.map(|v| f32::from(v) / 4.0);
    let matrix = DWRITE_MATRIX {
        m11,
        m12,
        m21,
        m22,
        dx,
        dy,
    };
    let analyze = |mode| {
        let advance = 0.0;
        let offset = DWRITE_GLYPH_OFFSET::default();
        let mut run = DWRITE_GLYPH_RUN {
            fontFace: ManuallyDrop::new(Some(face.clone())),
            fontEmSize: f32::from_bits(key.ppem),
            glyphCount: 1,
            glyphIndices: &key.glyph,
            glyphAdvances: &advance,
            glyphOffsets: &offset,
            ..Default::default()
        };
        // SAFETY: The glyph, advance and offset outlive the call, and the run's cloned face is released exactly once.
        unsafe {
            let result =
                factory.CreateGlyphRunAnalysis(&run, 1.0, Some(&matrix), mode, measure, 0.0, 0.0);
            ManuallyDrop::drop(&mut run.fontFace);
            result.ok()
        }
    };
    let mut analysis = analyze(mode)?;
    let mut texture = DWRITE_TEXTURE_CLEARTYPE_3x1;
    // SAFETY: The analysis is live and the binding supplies writable storage for the ClearType bounds.
    let mut bounds = unsafe { analysis.GetAlphaTextureBounds(texture) }.ok()?;
    if bounds.left >= bounds.right || bounds.top >= bounds.bottom {
        analysis = analyze(DWRITE_RENDERING_MODE_ALIASED)?;
        texture = DWRITE_TEXTURE_ALIASED_1x1;
        // SAFETY: The aliased analysis is live and the binding supplies writable storage for its bounds.
        bounds = unsafe { analysis.GetAlphaTextureBounds(texture) }.ok()?;
    }
    let width = u32::try_from(bounds.right.checked_sub(bounds.left)?).ok()?;
    let height = u32::try_from(bounds.bottom.checked_sub(bounds.top)?).ok()?;
    let pixels = width.checked_mul(height)?;
    if pixels > 4 << 20 {
        return None;
    }
    if pixels == 0 {
        let mut metrics = DWRITE_GLYPH_METRICS::default();
        // SAFETY: The input and output pointers each cover the one glyph requested from the live face.
        unsafe { face.GetDesignGlyphMetrics(&key.glyph, 1, &mut metrics, false) }.ok()?;
        let ink_width = i64::from(metrics.advanceWidth)
            - i64::from(metrics.leftSideBearing)
            - i64::from(metrics.rightSideBearing);
        let ink_height = i64::from(metrics.advanceHeight)
            - i64::from(metrics.topSideBearing)
            - i64::from(metrics.bottomSideBearing);
        if ink_width > 0 && ink_height > 0 {
            return None;
        }
    }
    let channels = if texture == DWRITE_TEXTURE_CLEARTYPE_3x1 {
        3
    } else {
        1
    };
    let count = usize::try_from(pixels).ok()?;
    let mut coverage = vec![0; count * channels];
    if pixels > 0 {
        // SAFETY: The writable slice holds exactly the bytes required for the texture type and bounds.
        unsafe { analysis.CreateAlphaTexture(texture, &bounds, &mut coverage) }.ok()?;
        if channels == 3 {
            for i in 0..count {
                let rgb = &coverage[3 * i..3 * i + 3];
                coverage[i] =
                    u8::try_from((u16::from(rgb[0]) + u16::from(rgb[1]) + u16::from(rgb[2])) / 3)
                        .unwrap_or(u8::MAX);
            }
            coverage.truncate(count);
        }
    }
    Some(Mask {
        left: bounds.left,
        top: bounds.top,
        coverage: coverage.into_boxed_slice(),
        width,
        height,
        tinted: Vec::new(),
    })
}
