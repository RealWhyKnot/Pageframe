use anyrender::{Glyph, Paint, PaintRef, PaintScene, RenderContext};
use anyrender_directwrite::GlyphPaint;
use kurbo::{Affine, Shape};
use peniko::{BlendMode, Blob, Color, Fill, FontData, ImageData, StyleRef};
use std::{path::Path, sync::Arc};

#[derive(Default)]
pub(crate) struct Recorder {
    pub(crate) images: Vec<ImageData>,
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
            self.images.push(image.image.clone());
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

pub(crate) fn system_font(file: &str) -> FontData {
    let windows = std::env::var("WINDIR").expect("WINDIR is set on Windows");
    let bytes =
        std::fs::read(Path::new(&windows).join("Fonts").join(file)).expect("font is installed");
    FontData::new(Blob::new(Arc::new(bytes)), 0)
}
