use anyrender::PaintScene;
use kurbo::{Affine, BezPath, Rect, Shape as _};
use peniko::{BlendMode, Color, ColorStop as Stop, Compose, Extend, Fill, Gradient, Mix};
use skrifa::color::{
    Brush, ColorPainter, ColorStop, CompositeMode, Extend as ColrExtend, Transform,
};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::TableProvider as _;
use skrifa::raw::types::{BoundingBox, Tag};
use skrifa::{FontRef, GlyphId, MetadataProvider as _};

pub(super) fn colored(font: &FontRef<'_>) -> bool {
    font.table_data(Tag::new(b"COLR")).is_some()
}

pub(super) fn has(font: &FontRef<'_>, glyph: u32) -> bool {
    font.color_glyphs().get(GlyphId::new(glyph)).is_some()
}

pub(super) fn paint(
    scene: &mut impl PaintScene,
    font: &FontRef<'_>,
    location: LocationRef<'_>,
    text: Color,
    transform: Affine,
    glyph: u32,
) {
    let Some(color_glyph) = font.color_glyphs().get(GlyphId::new(glyph)) else {
        return;
    };
    let mut painter = Painter {
        scene,
        font,
        location,
        palette: palette(font),
        text,
        transforms: vec![transform],
        clips: vec![Rect::new(-1e7, -1e7, 1e7, 1e7)],
    };
    let _ = color_glyph.paint(location, &mut painter);
    while painter.clips.len() > 1 {
        painter.clips.pop();
        painter.scene.pop_layer();
    }
}

fn palette(font: &FontRef<'_>) -> Vec<Color> {
    let Ok(cpal) = font.cpal() else {
        return Vec::new();
    };
    let first = cpal
        .color_record_indices()
        .first()
        .map_or(0, |index| usize::from(index.get()));
    let count = usize::from(cpal.num_palette_entries());
    let Some(Ok(records)) = cpal.color_records_array() else {
        return Vec::new();
    };
    records
        .iter()
        .skip(first)
        .take(count)
        .map(|c| Color::from_rgba8(c.red, c.green, c.blue, c.alpha))
        .collect()
}

struct Painter<'a, 'f, S> {
    scene: &'a mut S,
    font: &'a FontRef<'f>,
    location: LocationRef<'a>,
    palette: Vec<Color>,
    text: Color,
    transforms: Vec<Affine>,
    clips: Vec<Rect>,
}

impl<S: PaintScene> Painter<'_, '_, S> {
    fn current(&self) -> Affine {
        self.transforms.last().copied().unwrap_or_default()
    }

    fn bounds(&self) -> Rect {
        self.clips.last().copied().unwrap_or_default()
    }

    fn color(&self, index: u16, alpha: f32) -> Color {
        let base = if index == u16::MAX {
            self.text
        } else {
            self.palette
                .get(usize::from(index))
                .copied()
                .unwrap_or(Color::BLACK)
        };
        base.multiply_alpha(alpha)
    }

    fn stops(&self, stops: &[ColorStop]) -> Vec<Stop> {
        stops
            .iter()
            .map(|stop| Stop::from((stop.offset, self.color(stop.palette_index, stop.alpha))))
            .collect()
    }

    fn clip(&mut self, path: &BezPath) {
        let transform = self.current();
        let bounds = transform
            .transform_rect_bbox(path.bounding_box())
            .intersect(self.bounds());
        self.scene.push_clip_layer(Fill::NonZero, transform, path);
        self.clips.push(bounds);
    }

    fn gradient(&mut self, gradient: Gradient, stops: &[ColorStop], extend: ColrExtend) {
        let stops = self.stops(stops);
        let extend = match extend {
            ColrExtend::Repeat => Extend::Repeat,
            ColrExtend::Reflect => Extend::Reflect,
            _ => Extend::Pad,
        };
        let gradient = gradient.with_extend(extend).with_stops(stops.as_slice());
        let bounds = self.bounds();
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &gradient,
            Some(self.current()),
            &bounds,
        );
    }
}

impl<S: PaintScene> ColorPainter for Painter<'_, '_, S> {
    fn push_transform(&mut self, transform: Transform) {
        let Transform {
            xx,
            yx,
            xy,
            yy,
            dx,
            dy,
        } = transform;
        let local = Affine::new([xx, yx, xy, yy, dx, dy].map(f64::from));
        self.transforms.push(self.current() * local);
    }

    fn pop_transform(&mut self) {
        self.transforms.pop();
    }

    fn push_clip_glyph(&mut self, glyph: GlyphId) {
        let mut pen = Pen::default();
        if let Some(outline) = self.font.outline_glyphs().get(glyph) {
            let settings = DrawSettings::unhinted(Size::unscaled(), self.location);
            if outline.draw(settings, &mut pen).is_err() {
                pen.path = BezPath::new();
            }
        }
        self.clip(&pen.path);
    }

    fn push_clip_box(&mut self, clip: BoundingBox<f32>) {
        let rect = Rect::new(
            f64::from(clip.x_min),
            f64::from(clip.y_min),
            f64::from(clip.x_max),
            f64::from(clip.y_max),
        );
        self.clip(&rect.to_path(0.1));
    }

    fn pop_clip(&mut self) {
        if self.clips.len() > 1 {
            self.clips.pop();
            self.scene.pop_layer();
        }
    }

    fn fill(&mut self, brush: Brush<'_>) {
        let bounds = self.bounds();
        if bounds.is_zero_area() {
            return;
        }
        match brush {
            Brush::Solid {
                palette_index,
                alpha,
            } => {
                let color = self.color(palette_index, alpha);
                self.scene
                    .fill(Fill::NonZero, Affine::IDENTITY, color, None, &bounds);
            }
            Brush::LinearGradient {
                p0,
                p1,
                color_stops,
                extend,
            } => {
                let gradient = Gradient::new_linear((p0.x, p0.y), (p1.x, p1.y));
                self.gradient(gradient, color_stops, extend);
            }
            Brush::RadialGradient {
                c0,
                r0,
                c1,
                r1,
                color_stops,
                extend,
            } => {
                let gradient = Gradient::new_two_point_radial((c0.x, c0.y), r0, (c1.x, c1.y), r1);
                self.gradient(gradient, color_stops, extend);
            }
            Brush::SweepGradient {
                c0,
                start_angle,
                end_angle,
                color_stops,
                extend,
            } => {
                let gradient = Gradient::new_sweep(
                    (c0.x, c0.y),
                    start_angle.to_radians(),
                    end_angle.to_radians(),
                );
                self.gradient(gradient, color_stops, extend);
            }
        }
    }

    fn push_layer(&mut self, mode: CompositeMode) {
        let bounds = self.bounds();
        self.scene.push_layer(
            Fill::NonZero,
            blend(mode),
            1.0,
            Affine::IDENTITY,
            &bounds,
            None,
            None,
        );
        self.clips.push(bounds);
    }

    fn pop_layer(&mut self) {
        if self.clips.len() > 1 {
            self.clips.pop();
            self.scene.pop_layer();
        }
    }
}

fn blend(mode: CompositeMode) -> BlendMode {
    let (mix, compose) = match mode {
        CompositeMode::Clear => (Mix::Normal, Compose::Clear),
        CompositeMode::Src => (Mix::Normal, Compose::Copy),
        CompositeMode::Dest => (Mix::Normal, Compose::Dest),
        CompositeMode::DestOver => (Mix::Normal, Compose::DestOver),
        CompositeMode::SrcIn => (Mix::Normal, Compose::SrcIn),
        CompositeMode::DestIn => (Mix::Normal, Compose::DestIn),
        CompositeMode::SrcOut => (Mix::Normal, Compose::SrcOut),
        CompositeMode::DestOut => (Mix::Normal, Compose::DestOut),
        CompositeMode::SrcAtop => (Mix::Normal, Compose::SrcAtop),
        CompositeMode::DestAtop => (Mix::Normal, Compose::DestAtop),
        CompositeMode::Xor => (Mix::Normal, Compose::Xor),
        CompositeMode::Plus => (Mix::Normal, Compose::Plus),
        CompositeMode::Screen => (Mix::Screen, Compose::SrcOver),
        CompositeMode::Overlay => (Mix::Overlay, Compose::SrcOver),
        CompositeMode::Darken => (Mix::Darken, Compose::SrcOver),
        CompositeMode::Lighten => (Mix::Lighten, Compose::SrcOver),
        CompositeMode::ColorDodge => (Mix::ColorDodge, Compose::SrcOver),
        CompositeMode::ColorBurn => (Mix::ColorBurn, Compose::SrcOver),
        CompositeMode::HardLight => (Mix::HardLight, Compose::SrcOver),
        CompositeMode::SoftLight => (Mix::SoftLight, Compose::SrcOver),
        CompositeMode::Difference => (Mix::Difference, Compose::SrcOver),
        CompositeMode::Exclusion => (Mix::Exclusion, Compose::SrcOver),
        CompositeMode::Multiply => (Mix::Multiply, Compose::SrcOver),
        CompositeMode::HslHue => (Mix::Hue, Compose::SrcOver),
        CompositeMode::HslSaturation => (Mix::Saturation, Compose::SrcOver),
        CompositeMode::HslColor => (Mix::Color, Compose::SrcOver),
        CompositeMode::HslLuminosity => (Mix::Luminosity, Compose::SrcOver),
        _ => (Mix::Normal, Compose::SrcOver),
    };
    BlendMode::new(mix, compose)
}

#[derive(Default)]
struct Pen {
    path: BezPath,
}

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to((f64::from(x), f64::from(y)));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to((f64::from(x), f64::from(y)));
    }

    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.path
            .quad_to((f64::from(cx), f64::from(cy)), (f64::from(x), f64::from(y)));
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.path.curve_to(
            (f64::from(cx0), f64::from(cy0)),
            (f64::from(cx1), f64::from(cy1)),
            (f64::from(x), f64::from(y)),
        );
    }

    fn close(&mut self) {
        self.path.close_path();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyrender::recording::{RenderCommand, Scene};

    fn emoji_font() -> Vec<u8> {
        let windows = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
        std::fs::read(
            std::path::Path::new(&windows)
                .join("Fonts")
                .join("seguiemj.ttf"),
        )
        .expect("Segoe UI Emoji is installed")
    }

    #[test]
    fn color_glyphs_paint_as_balanced_vector_layers() {
        let data = emoji_font();
        let font = FontRef::new(&data).expect("font parses");
        let map = font.charmap();
        let smile = map.map('\u{1F600}').expect("smile glyph").to_u32();
        let digit = map.map('0').expect("digit glyph").to_u32();
        assert!(colored(&font));
        assert!(has(&font, smile));
        assert!(!has(&font, digit));
        let mut scene = Scene::new();
        let scale = 20.0 / f64::from(font.head().expect("head").units_per_em());
        let placed = Affine::new([scale, 0.0, 0.0, -scale, 100.0, 120.0]);
        paint(
            &mut scene,
            &font,
            LocationRef::default(),
            Color::BLACK,
            placed,
            smile,
        );
        let opened = scene
            .commands
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    RenderCommand::PushLayer(_) | RenderCommand::PushClipLayer(_)
                )
            })
            .count();
        let closed = scene
            .commands
            .iter()
            .filter(|c| matches!(c, RenderCommand::PopLayer))
            .count();
        assert!(opened > 0);
        assert_eq!(opened, closed);
        let area = Rect::new(96.0, 96.0, 124.0, 126.0);
        let fills: Vec<_> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                RenderCommand::Fill(fill) => Some(fill),
                _ => None,
            })
            .collect();
        assert!(
            fills
                .iter()
                .any(|f| matches!(f.brush, anyrender::Paint::Gradient(_)))
        );
        for fill in fills {
            let bounds = fill
                .transform
                .transform_rect_bbox(fill.shape.bounding_box());
            assert_eq!(bounds.intersect(area), bounds, "fill stays on the glyph");
        }
        assert!(
            !scene
                .commands
                .iter()
                .any(|c| matches!(c, RenderCommand::GlyphRun(_)))
        );
    }

    #[test]
    fn missing_color_glyphs_paint_nothing() {
        let data = emoji_font();
        let font = FontRef::new(&data).expect("font parses");
        let digit = font.charmap().map('0').expect("digit glyph").to_u32();
        let mut scene = Scene::new();
        paint(
            &mut scene,
            &font,
            LocationRef::default(),
            Color::BLACK,
            Affine::IDENTITY,
            digit,
        );
        assert!(scene.commands.is_empty());
    }
}
