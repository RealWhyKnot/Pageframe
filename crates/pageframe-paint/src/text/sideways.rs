use std::collections::HashMap;
use std::sync::Arc;

use kurbo::{Affine, BezPath, PathEl, Point};
use num_traits::ToPrimitive as _;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, Engine, HintingInstance, HintingOptions, OutlinePen, Target};
use skrifa::raw::TableProvider as _;
use skrifa::raw::tables::gasp::GaspRangeBehavior;
use skrifa::raw::types::Tag;
use skrifa::{FontRef, GlyphId, MetadataProvider as _};

use super::upright::{FaceKey, GlyphKey, SizedFace};

const MAX_CACHED_GLYPHS: usize = 4096;
const MAX_CACHED_FACES: usize = 64;
const COLOR_TABLES: [&[u8; 4]; 5] = [b"COLR", b"CBDT", b"sbix", b"SVG ", b"EBDT"];
const HINTING: HintingOptions = HintingOptions {
    engine: Engine::Interpreter,
    target: Target::Mono,
};

#[derive(Default)]
pub(super) struct SidewaysCache {
    paths: HashMap<GlyphKey, Option<Arc<BezPath>>>,
    faces: HashMap<FaceKey, Option<Option<HintingInstance>>>,
}

#[derive(Clone, Copy)]
pub(super) struct SidewaysFrame {
    transform: Affine,
    turn: Affine,
    scale: f64,
}

impl SidewaysFrame {
    pub(super) fn new(transform: Affine) -> Option<Self> {
        let [across, along, back, down, _, _] = transform.as_coeffs();
        let scale = along.abs();
        (across == 0.0 && down == 0.0 && scale > 0.0 && back == -along).then(|| Self {
            transform,
            turn: Affine::new([0.0, along / scale, back / scale, 0.0, 0.0, 0.0]) * Affine::FLIP_Y,
            scale,
        })
    }

    pub(super) fn ppem(self, font_size: f32) -> Option<f32> {
        (f64::from(font_size) * self.scale).to_f32()
    }

    pub(super) fn place(self, inline: f32, block: f32) -> Affine {
        let origin = self.transform * Point::new(f64::from(inline), f64::from(block));
        Affine::translate((origin.x, origin.y.round())) * self.turn
    }
}

impl SidewaysCache {
    pub(super) fn outlines(
        &mut self,
        face: &SizedFace<'_>,
        glyphs: impl Iterator<Item = u32>,
        out: &mut Vec<Arc<BezPath>>,
    ) -> bool {
        out.clear();
        if self.faces.len() >= MAX_CACHED_FACES {
            self.faces.clear();
        }
        let Some(instance) = self
            .faces
            .entry(face.key())
            .or_insert_with(|| {
                let font = face.font_ref()?;
                point_sampled(&font, face.ppem()).then(|| hinting_instance(face, &font))
            })
            .as_ref()
        else {
            return false;
        };
        if self.paths.len() >= MAX_CACHED_GLYPHS {
            self.paths.clear();
        }
        for glyph in glyphs {
            let key = face.glyph_key(glyph);
            let path = match self.paths.get(&key) {
                Some(path) => path.as_ref().map(Arc::clone),
                None => {
                    let path = sampled_outline(face, instance.as_ref(), glyph).map(Arc::new);
                    self.paths.insert(key, path.as_ref().map(Arc::clone));
                    path
                }
            };
            let Some(path) = path else {
                out.clear();
                return false;
            };
            out.push(path);
        }
        true
    }
}

fn point_sampled(font: &FontRef<'_>, ppem: f32) -> bool {
    let Some(size) = ppem.round().to_u16() else {
        return false;
    };
    let plain = font.glyf().is_ok()
        && COLOR_TABLES
            .iter()
            .all(|tag| font.table_data(Tag::new(tag)).is_none());
    plain
        && font.gasp().is_ok_and(|gasp| {
            gasp.gasp_ranges()
                .iter()
                .find(|range| range.range_max_ppem() >= size)
                .is_some_and(|range| {
                    let behavior = range.range_gasp_behavior();
                    behavior.contains(GaspRangeBehavior::GASP_GRIDFIT)
                        && !behavior.contains(GaspRangeBehavior::GASP_SYMMETRIC_SMOOTHING)
                })
        })
}

fn hinting_instance(face: &SizedFace<'_>, font: &FontRef<'_>) -> Option<HintingInstance> {
    let coords = face.location();
    HintingInstance::new(
        &font.outline_glyphs(),
        Size::new(face.ppem()),
        LocationRef::new(&coords),
        HINTING,
    )
    .ok()
}

fn sampled_outline(
    face: &SizedFace<'_>,
    instance: Option<&HintingInstance>,
    glyph: u32,
) -> Option<BezPath> {
    let font = face.font_ref()?;
    let coords = face.location();
    let outlines = font.outline_glyphs();
    let outline = outlines.get(GlyphId::new(glyph))?;
    let unhinted = DrawSettings::unhinted(Size::new(face.ppem()), LocationRef::new(&coords));
    let mut plain = Recorder::default();
    outline.draw(unhinted, &mut plain).ok()?;
    let mut hinted = Recorder::default();
    let fitted = instance.is_some_and(|instance| {
        outline
            .draw(DrawSettings::hinted(instance, false), &mut hinted)
            .is_ok()
    });
    let across = if fitted && hinted.path.elements().len() == plain.path.elements().len() {
        &hinted.path
    } else {
        &plain.path
    };
    Some(merge(across, &plain.path))
}

fn merge(across: &BezPath, plain: &BezPath) -> BezPath {
    let mix = |snapped: Point, kept: Point| Point::new(snapped.x.round(), kept.y);
    plain
        .elements()
        .iter()
        .zip(across.elements())
        .map(|(&kept, &snapped)| match (snapped, kept) {
            (PathEl::MoveTo(first), PathEl::MoveTo(second)) => PathEl::MoveTo(mix(first, second)),
            (PathEl::LineTo(first), PathEl::LineTo(second)) => PathEl::LineTo(mix(first, second)),
            (PathEl::QuadTo(first, end), PathEl::QuadTo(second, other)) => {
                PathEl::QuadTo(mix(first, second), mix(end, other))
            }
            (PathEl::CurveTo(first, middle, end), PathEl::CurveTo(second, centre, other)) => {
                PathEl::CurveTo(mix(first, second), mix(middle, centre), mix(end, other))
            }
            (_, kept) => kept,
        })
        .collect()
}

#[derive(Default)]
struct Recorder {
    path: BezPath,
}

impl OutlinePen for Recorder {
    fn move_to(&mut self, to_x: f32, to_y: f32) {
        self.path.move_to((f64::from(to_x), f64::from(to_y)));
    }

    fn line_to(&mut self, to_x: f32, to_y: f32) {
        self.path.line_to((f64::from(to_x), f64::from(to_y)));
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, to_x: f32, to_y: f32) {
        self.path.quad_to(
            (f64::from(cx0), f64::from(cy0)),
            (f64::from(to_x), f64::from(to_y)),
        );
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, to_x: f32, to_y: f32) {
        self.path.curve_to(
            (f64::from(cx0), f64::from(cy0)),
            (f64::from(cx1), f64::from(cy1)),
            (f64::from(to_x), f64::from(to_y)),
        );
    }

    fn close(&mut self) {
        self.path.close_path();
    }
}
