#![cfg(target_os = "windows")]

use anyrender::{Glyph, NullScenePainter, Paint, PaintRef, PaintScene, RenderContext};
use anyrender_directwrite::{FontSource, GlyphPaint, GlyphRun, supported};
use kurbo::{Affine, Shape};
use peniko::{BlendMode, Blob, Color, Fill, FontData, StyleRef};
use std::{path::Path, sync::Arc};

#[derive(Default)]
struct Recorder {
    images: Vec<u64>,
}

impl RenderContext for Recorder {}

impl PaintScene for Recorder {
    fn reset(&mut self) {
        self.images.clear();
    }

    fn push_layer(
        &mut self,
        _: Fill,
        _: impl Into<BlendMode>,
        _: f32,
        _: Affine,
        _: &impl Shape,
        _: Option<Arc<anyrender::Filter>>,
        _: Option<Arc<anyrender::Filter>>,
    ) {
    }

    fn push_clip_layer(&mut self, _: Fill, _: Affine, _: &impl Shape) {}

    fn pop_layer(&mut self) {}

    fn stroke<'a>(
        &mut self,
        _: &kurbo::Stroke,
        _: Affine,
        _: impl Into<PaintRef<'a>>,
        _: Option<Affine>,
        _: &impl Shape,
    ) {
    }

    fn fill<'a>(
        &mut self,
        _: Fill,
        _: Affine,
        paint: impl Into<PaintRef<'a>>,
        _: Option<Affine>,
        _: &impl Shape,
    ) {
        if let Paint::Image(image) = paint.into() {
            self.images.push(image.image.data.id());
        }
    }

    fn draw_glyphs<'a, 's: 'a>(
        &'s mut self,
        font: &'a FontData,
        font_size: f32,
        _: bool,
        _: &'a [anyrender::NormalizedCoord],
        _: kurbo::Vec2,
        _: impl Into<StyleRef<'a>>,
        paint: impl Into<PaintRef<'a>>,
        _: f32,
        transform: Affine,
        _: Option<Affine>,
        glyphs: impl Iterator<Item = Glyph>,
    ) {
        if let Paint::Custom(custom) = paint.into()
            && let Some(native) = custom.downcast_ref::<GlyphPaint>()
        {
            native.draw(self, font, font_size, transform, glyphs);
        }
    }

    fn draw_box_shadow(&mut self, _: Affine, _: kurbo::Rect, _: Color, _: f64, _: f64) {}
}

fn arial() -> (FontData, Arc<Path>) {
    let windows = std::env::var("WINDIR").expect("WINDIR is set on Windows");
    let path: Arc<Path> = Path::new(&windows).join("Fonts/arial.ttf").into();
    let bytes = std::fs::read(&path).expect("Arial is installed");
    (FontData::new(Blob::new(Arc::new(bytes)), 0), path)
}

fn frame(scene: &mut Recorder, font: &FontData, path: &Arc<Path>, colors: &[Color]) -> Vec<u64> {
    scene.reset();
    for (row, &color) in (0u8..).zip(colors) {
        let glyphs = (36..44).map(|id| Glyph {
            id,
            x: f32::from(u8::try_from(id - 36).unwrap_or(0)) * 9.25,
            y: 0.0,
        });
        let run = GlyphRun {
            font,
            font_size: 16.0,
            transform: Affine::translate((4.0, 20.0 + f64::from(row) * 20.0)),
            color,
        };
        let source = || {
            Some(FontSource {
                path: Some(Arc::clone(path)),
                gasp_version: None,
                ranges: Vec::new(),
                strikes: Vec::new(),
                hinted: true,
            })
        };
        assert!(run.draw(scene, source, glyphs), "row {row} is painted");
    }
    scene.images.clone()
}

#[test]
fn probe_tells_native_scenes_apart() {
    let (font, _) = arial();
    assert!(supported(&mut Recorder::default(), &font));
    assert!(!supported(&mut NullScenePainter, &font));
    assert!(!supported(&mut anyrender::Scene::new(), &font));
}

#[test]
fn repeated_frames_reuse_every_glyph_image() {
    let (font, path) = arial();
    let colors = [
        Color::BLACK,
        Color::from_rgb8(200, 30, 30),
        Color::BLACK,
        Color::from_rgb8(30, 30, 200),
        Color::from_rgb8(30, 140, 30),
        Color::from_rgb8(90, 90, 90),
        Color::from_rgb8(220, 120, 0),
    ];
    let mut scene = Recorder::default();
    let first = frame(&mut scene, &font, &path, &colors);
    let second = frame(&mut scene, &font, &path, &colors);
    assert_eq!(first.len(), colors.len() * 8);
    assert_eq!(first, second);
    assert_eq!(first[..8], first[16..24]);
    assert!(first[..8].iter().all(|id| !first[8..16].contains(id)));
}
