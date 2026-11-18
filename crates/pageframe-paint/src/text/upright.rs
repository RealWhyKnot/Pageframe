use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use kurbo::Affine;
use num_traits::ToPrimitive as _;
use pageframe_dom::node::TextBrush;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{
    DrawSettings, Engine, HintingInstance, HintingOptions, OutlinePen, SmoothMode, Target,
};
use skrifa::raw::types::{F2Dot14, Tag};
use skrifa::raw::{FontRef, TableProvider as _};
use skrifa::{GlyphId, MetadataProvider as _};

const MAX_CACHED_GLYPHS: usize = 4096;
const MAX_CACHED_INSTANCES: usize = 64;
const CURVE_STEPS: u8 = 16;
const COVERAGE_SLACK: f32 = 1.0 / 64.0;
const HINTING: HintingOptions = HintingOptions {
    engine: Engine::AutoFallback,
    target: Target::Smooth {
        mode: SmoothMode::Lcd,
        symmetric_rendering: false,
        preserve_linear_metrics: true,
    },
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct FaceKey {
    font: u64,
    index: u32,
    coords: u64,
    size: u32,
}

#[derive(PartialEq, Eq, Hash)]
struct GlyphKey {
    face: FaceKey,
    glyph: u32,
}

#[derive(Default)]
pub(super) struct UprightCache {
    shifts: HashMap<GlyphKey, f32>,
    instances: HashMap<FaceKey, Option<HintingInstance>>,
    bounded: HashMap<(u64, u32), bool>,
}

pub(super) struct UprightRun<'a> {
    font: &'a parley::FontData,
    ppem: f32,
    coords: &'a [i16],
    face: FaceKey,
}

impl<'a> UprightRun<'a> {
    pub(super) fn new(font: &'a parley::FontData, ppem: f32, coords: &'a [i16]) -> Self {
        let mut hasher = DefaultHasher::new();
        coords.hash(&mut hasher);
        Self {
            font,
            ppem,
            coords,
            face: FaceKey {
                font: font.data.id(),
                index: font.index,
                coords: hasher.finish(),
                size: ppem.to_bits(),
            },
        }
    }

    fn font_ref(&self) -> Option<FontRef<'a>> {
        FontRef::from_index(self.font.data.as_ref(), self.font.index).ok()
    }

    fn location(&self) -> Vec<F2Dot14> {
        self.coords
            .iter()
            .map(|&coord| F2Dot14::from_bits(coord))
            .collect()
    }
}

#[derive(Clone, Copy)]
pub(super) struct InlineSnap {
    scale: f64,
    offset: f64,
    start: f64,
}

impl InlineSnap {
    pub(super) fn new(transform: Affine, run_start: f32) -> Option<Self> {
        let [across, scale, _, along, _, offset] = transform.as_coeffs();
        (across == 0.0 && along == 0.0 && scale > 0.0).then(|| Self {
            scale,
            offset,
            start: scale * f64::from(run_start) + offset,
        })
    }

    pub(super) fn ppem(self, font_size: f32) -> Option<f32> {
        (f64::from(font_size) * self.scale).to_f32()
    }

    pub(super) fn place(self, inline: f32, shift: f32) -> f32 {
        let device = self.scale * f64::from(inline) + self.offset;
        let snapped = self.start.round() + (device - self.start + f64::from(shift)).round();
        ((snapped - self.offset) / self.scale)
            .to_f32()
            .unwrap_or(inline)
    }
}

impl UprightCache {
    pub(super) fn place_run(
        &mut self,
        placed: &mut Vec<f32>,
        transform: Affine,
        glyph_run: &parley::GlyphRun<'_, TextBrush>,
    ) {
        placed.clear();
        let run = glyph_run.run();
        if !run.is_upright() {
            return;
        }
        let Some(snap) = InlineSnap::new(transform, glyph_run.offset()) else {
            return;
        };
        let Some(ppem) = snap.ppem(run.font_size()) else {
            return;
        };
        let face = UprightRun::new(
            run.font(),
            ppem,
            bytemuck::cast_slice(run.normalized_coords()),
        );
        placed.extend(
            glyph_run
                .positioned_glyphs()
                .map(|glyph| snap.place(glyph.x, self.origin_shift(&face, glyph.id))),
        );
    }

    pub(super) fn origin_shift(&mut self, run: &UprightRun<'_>, glyph: u32) -> f32 {
        let face = (run.face.font, run.face.index);
        let bounded = *self
            .bounded
            .entry(face)
            .or_insert_with(|| run.font_ref().is_some_and(|font| origin_from_bounds(&font)));
        if !bounded {
            return 0.0;
        }
        let key = GlyphKey {
            face: run.face,
            glyph,
        };
        if let Some(shift) = self.shifts.get(&key) {
            return *shift;
        }
        if self.shifts.len() >= MAX_CACHED_GLYPHS {
            self.shifts.clear();
        }
        if self.instances.len() >= MAX_CACHED_INSTANCES {
            self.instances.clear();
        }
        let instance = self
            .instances
            .entry(run.face)
            .or_insert_with(|| hinting_instance(run));
        let shift = bounds_shift(run, instance.as_ref(), glyph).unwrap_or(0.0);
        self.shifts.insert(key, shift);
        shift
    }
}

fn origin_from_bounds(font: &FontRef<'_>) -> bool {
    font.vmtx().is_ok() && font.table_data(Tag::new(b"VORG")).is_none()
}

fn hinting_instance(run: &UprightRun<'_>) -> Option<HintingInstance> {
    let font = run.font_ref()?;
    let coords = run.location();
    HintingInstance::new(
        &font.outline_glyphs(),
        Size::new(run.ppem),
        LocationRef::new(&coords),
        HINTING,
    )
    .ok()
}

fn bounds_shift(
    run: &UprightRun<'_>,
    instance: Option<&HintingInstance>,
    glyph: u32,
) -> Option<f32> {
    let font = run.font_ref()?;
    let coords = run.location();
    let outlines = font.outline_glyphs();
    let outline = outlines.get(GlyphId::new(glyph))?;
    let mut points = TopPen::default();
    outline
        .draw(
            DrawSettings::unhinted(Size::new(run.ppem), LocationRef::new(&coords)),
            &mut points,
        )
        .ok()?;
    let mut ink = TopPen {
        sample_curves: true,
        ..TopPen::default()
    };
    let settings = match instance {
        Some(instance) => DrawSettings::hinted(instance, false),
        None => DrawSettings::unhinted(Size::new(run.ppem), LocationRef::new(&coords)),
    };
    outline.draw(settings, &mut ink).ok()?;
    let ink_top = (ink.top? - COVERAGE_SLACK).ceil();
    Some(ink_top - points.top?)
}

#[derive(Default)]
struct TopPen {
    sample_curves: bool,
    last: f32,
    top: Option<f32>,
}

impl TopPen {
    fn include(&mut self, height: f32) {
        self.top = Some(self.top.map_or(height, |top| top.max(height)));
    }

    fn point(&mut self, height: f32) {
        self.last = height;
        self.include(height);
    }
}

impl OutlinePen for TopPen {
    fn move_to(&mut self, _to_x: f32, to_y: f32) {
        self.point(to_y);
    }

    fn line_to(&mut self, _to_x: f32, to_y: f32) {
        self.point(to_y);
    }

    fn quad_to(&mut self, _cx0: f32, cy0: f32, _to_x: f32, to_y: f32) {
        if self.sample_curves {
            let start = self.last;
            for step in 1..CURVE_STEPS {
                let along = f32::from(step) / f32::from(CURVE_STEPS);
                let rest = 1.0 - along;
                self.include(rest * rest * start + 2.0 * rest * along * cy0 + along * along * to_y);
            }
        } else {
            self.include(cy0);
        }
        self.point(to_y);
    }

    fn curve_to(&mut self, _cx0: f32, cy0: f32, _cx1: f32, cy1: f32, _to_x: f32, to_y: f32) {
        if self.sample_curves {
            let start = self.last;
            for step in 1..CURVE_STEPS {
                let along = f32::from(step) / f32::from(CURVE_STEPS);
                let rest = 1.0 - along;
                self.include(
                    rest * rest * rest * start
                        + 3.0 * rest * rest * along * cy0
                        + 3.0 * rest * along * along * cy1
                        + along * along * along * to_y,
                );
            }
        } else {
            self.include(cy0);
            self.include(cy1);
        }
        self.point(to_y);
    }

    fn close(&mut self) {}
}
