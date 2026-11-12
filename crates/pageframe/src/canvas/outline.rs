use kurbo::{BezPath, Point};
use parley::FontData;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{
    DrawSettings, Engine, HintingInstance, HintingOptions, OutlinePen, SmoothMode, Target,
};
use skrifa::raw::types::F2Dot14;
use skrifa::{FontRef, GlyphId, MetadataProvider as _, Tag};

const HINTING: HintingOptions = HintingOptions {
    engine: Engine::AutoFallback,
    target: Target::Smooth {
        mode: SmoothMode::Lcd,
        symmetric_rendering: false,
        preserve_linear_metrics: true,
    },
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Fit {
    Smooth { skew: f64 },
    Grid,
}

const COLOUR_TABLES: [&[u8; 4]; 4] = [b"COLR", b"CBDT", b"sbix", b"SVG "];

pub(super) struct Placed {
    pub(super) id: u32,
    pub(super) x: f64,
    pub(super) y: f64,
}

struct Pen {
    path: BezPath,
    origin: Point,
    skew: f64,
}

impl Pen {
    fn at(&self, x: f32, y: f32) -> Point {
        let (x, y) = (f64::from(x), f64::from(y));
        Point::new(self.origin.x + x + self.skew * y, self.origin.y - y)
    }
}

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to(self.at(x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to(self.at(x, y));
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.path.quad_to(self.at(cx0, cy0), self.at(x, y));
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.path
            .curve_to(self.at(cx0, cy0), self.at(cx1, cy1), self.at(x, y));
    }

    fn close(&mut self) {
        self.path.close_path();
    }
}

pub(super) fn glyph_path(
    font: &FontData,
    size: f32,
    coords: &[i16],
    fit: Fit,
    glyphs: impl Iterator<Item = Placed>,
) -> Option<BezPath> {
    let font_ref = FontRef::from_index(font.data.as_ref(), font.index).ok()?;
    let coloured = COLOUR_TABLES
        .iter()
        .any(|name| font_ref.table_data(Tag::new(name)).is_some());
    if coloured {
        return None;
    }
    let coords: Vec<F2Dot14> = coords.iter().copied().map(F2Dot14::from_bits).collect();
    let location = LocationRef::new(&coords);
    let outlines = font_ref.outline_glyphs();
    let hinting = match fit {
        Fit::Grid => HintingInstance::new(&outlines, Size::new(size), location, HINTING)
            .inspect_err(|error| tracing::debug!(%error, "canvas text hinting unavailable"))
            .ok(),
        Fit::Smooth { .. } => None,
    };
    let skew = match fit {
        Fit::Smooth { skew } => skew,
        Fit::Grid => 0.0,
    };
    let mut pen = Pen {
        path: BezPath::new(),
        origin: Point::ZERO,
        skew,
    };
    for glyph in glyphs {
        let Some(outline) = outlines.get(GlyphId::new(glyph.id)) else {
            continue;
        };
        pen.origin = Point::new(glyph.x, glyph.y);
        let settings = match &hinting {
            Some(instance) => DrawSettings::hinted(instance, false),
            None => DrawSettings::unhinted(Size::new(size), location),
        };
        if outline.draw(settings, &mut pen).is_err() {
            tracing::debug!(glyph = glyph.id, "canvas glyph outline failed");
        }
    }
    Some(pen.path)
}
