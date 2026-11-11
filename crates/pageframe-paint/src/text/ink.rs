use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::FontRef;
use skrifa::raw::types::F2Dot14;
use skrifa::{GlyphId, MetadataProvider as _};

const MAX_CACHED_GLYPHS: usize = 4096;
const CURVE_STEPS: u8 = 16;

#[derive(PartialEq, Eq, Hash)]
struct InkKey {
    font: u64,
    index: u32,
    coords: u64,
    size: u32,
    glyph: u32,
    low: u32,
    high: u32,
}

#[derive(Default)]
pub(super) struct InkCache {
    extents: HashMap<InkKey, Option<(f32, f32)>>,
}

pub(super) struct InkRun<'a> {
    font: &'a parley::FontData,
    font_size: f32,
    coords: &'a [i16],
    coords_hash: u64,
}

impl<'a> InkRun<'a> {
    pub(super) fn new(font: &'a parley::FontData, font_size: f32, coords: &'a [i16]) -> Self {
        let mut hasher = DefaultHasher::new();
        coords.hash(&mut hasher);
        Self {
            font,
            font_size,
            coords,
            coords_hash: hasher.finish(),
        }
    }
}

impl InkCache {
    pub(super) fn extent(
        &mut self,
        run: &InkRun<'_>,
        glyph: u32,
        low: f32,
        high: f32,
    ) -> Option<(f32, f32)> {
        let key = InkKey {
            font: run.font.data.id(),
            index: run.font.index,
            coords: run.coords_hash,
            size: run.font_size.to_bits(),
            glyph,
            low: low.to_bits(),
            high: high.to_bits(),
        };
        if let Some(extent) = self.extents.get(&key) {
            return *extent;
        }
        if self.extents.len() >= MAX_CACHED_GLYPHS {
            self.extents.clear();
        }
        let extent = outline_extent(run, glyph, low, high);
        self.extents.insert(key, extent);
        extent
    }
}

fn outline_extent(run: &InkRun<'_>, glyph: u32, low: f32, high: f32) -> Option<(f32, f32)> {
    let font = FontRef::from_index(run.font.data.as_ref(), run.font.index).ok()?;
    let coords: Vec<F2Dot14> = run
        .coords
        .iter()
        .map(|&coord| F2Dot14::from_bits(coord))
        .collect();
    let outline = font.outline_glyphs().get(GlyphId::new(glyph))?;
    let mut pen = BandPen {
        low,
        high,
        start: (0.0, 0.0),
        last: (0.0, 0.0),
        extent: None,
    };
    let settings = DrawSettings::unhinted(Size::new(run.font_size), LocationRef::new(&coords));
    outline.draw(settings, &mut pen).ok()?;
    pen.extent
}

struct BandPen {
    low: f32,
    high: f32,
    start: (f32, f32),
    last: (f32, f32),
    extent: Option<(f32, f32)>,
}

impl BandPen {
    fn include(&mut self, x: f32) {
        self.extent = Some(match self.extent {
            Some((left, right)) => (left.min(x), right.max(x)),
            None => (x, x),
        });
    }

    fn line(&mut self, to: (f32, f32)) {
        let (x0, y0) = self.last;
        let (x1, y1) = to;
        self.last = to;
        if y0.max(y1) < self.low || y0.min(y1) > self.high {
            return;
        }
        let rise = y1 - y0;
        if rise == 0.0 {
            self.include(x0);
            self.include(x1);
            return;
        }
        for edge in [self.low, self.high] {
            let along = ((edge - y0) / rise).clamp(0.0, 1.0);
            self.include(x0 + (x1 - x0) * along);
        }
    }
}

impl OutlinePen for BandPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.start = (x, y);
        self.last = (x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.line((x, y));
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (x0, y0) = self.last;
        for step in 1..=CURVE_STEPS {
            let t = f32::from(step) / f32::from(CURVE_STEPS);
            let u = 1.0 - t;
            self.line((
                u * u * x0 + 2.0 * u * t * cx0 + t * t * x,
                u * u * y0 + 2.0 * u * t * cy0 + t * t * y,
            ));
        }
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (x0, y0) = self.last;
        for step in 1..=CURVE_STEPS {
            let t = f32::from(step) / f32::from(CURVE_STEPS);
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            self.line((
                a * x0 + b * cx0 + c * cx1 + d * x,
                a * y0 + b * cy0 + c * cy1 + d * y,
            ));
        }
    }

    fn close(&mut self) {
        let start = self.start;
        self.line(start);
    }
}
