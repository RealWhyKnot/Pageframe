use std::collections::HashMap;
use std::sync::Arc;

use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape as _};
use num_traits::ToPrimitive as _;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{
    DrawSettings, Engine, HintingInstance, HintingOptions, OutlinePen, SmoothMode, Target,
};
use skrifa::raw::TableProvider as _;
use skrifa::raw::tables::gasp::GaspRangeBehavior;
use skrifa::raw::types::Tag;
use skrifa::{FontRef, GlyphId, MetadataProvider as _};

use super::upright::{FaceKey, GlyphKey, SizedFace};

const MAX_CACHED_GLYPHS: usize = 4096;
const MAX_CACHED_FACES: usize = 64;
const COLOR_TABLES: [&[u8; 4]; 5] = [b"COLR", b"CBDT", b"sbix", b"SVG ", b"EBDT"];
const SIDEWAYS_HINTING: HintingOptions = HintingOptions {
    engine: Engine::Interpreter,
    target: Target::Mono,
};
const LEVEL_HINTING: HintingOptions = HintingOptions {
    engine: Engine::AutoFallback,
    target: Target::Smooth {
        mode: SmoothMode::Lcd,
        symmetric_rendering: false,
        preserve_linear_metrics: true,
    },
};
const FLATTEN_TOLERANCE: f64 = 0.01;
const SUBPIXEL_STEPS: f64 = 4.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Turn {
    Level,
    Sideways,
}

#[derive(Default)]
pub(super) struct SampledCache {
    paths: HashMap<(Turn, GlyphKey), Option<Arc<BezPath>>>,
    faces: HashMap<(Turn, FaceKey), Option<Option<HintingInstance>>>,
}

#[derive(Clone, Copy)]
pub(super) struct SampledFrame {
    transform: Affine,
    turn: Turn,
    rotation: Affine,
    scale: f64,
}

impl SampledFrame {
    pub(super) fn new(transform: Affine) -> Option<Self> {
        let [first, second, third, fourth, _, _] = transform.as_coeffs();
        if second == 0.0 && third == 0.0 && first > 0.0 && fourth == first {
            return Some(Self {
                transform,
                turn: Turn::Level,
                rotation: Affine::IDENTITY,
                scale: first,
            });
        }
        let scale = second.abs();
        (first == 0.0 && fourth == 0.0 && scale > 0.0 && third == -second).then(|| Self {
            transform,
            turn: Turn::Sideways,
            rotation: Affine::new([0.0, second / scale, third / scale, 0.0, 0.0, 0.0])
                * Affine::FLIP_Y,
            scale,
        })
    }

    pub(super) fn ppem(self, font_size: f32) -> Option<f32> {
        (f64::from(font_size) * self.scale).to_f32()
    }

    pub(super) fn place(self, inline: f32, block: f32) -> Affine {
        let origin = self.transform * Point::new(f64::from(inline), f64::from(block));
        let across = match self.turn {
            Turn::Level => (origin.x * SUBPIXEL_STEPS).round() / SUBPIXEL_STEPS,
            Turn::Sideways => origin.x,
        };
        Affine::translate((across, origin.y.round())) * self.rotation
    }
}

impl SampledCache {
    pub(super) fn outlines(
        &mut self,
        frame: SampledFrame,
        face: &SizedFace<'_>,
        glyphs: impl Iterator<Item = u32>,
        out: &mut Vec<Arc<BezPath>>,
    ) -> bool {
        out.clear();
        if self.faces.len() >= MAX_CACHED_FACES {
            self.faces.clear();
        }
        let turn = frame.turn;
        let Some(instance) = self
            .faces
            .entry((turn, face.key()))
            .or_insert_with(|| {
                let font = face.font_ref()?;
                point_sampled(&font, face.ppem()).then(|| hinting_instance(face, &font, turn))
            })
            .as_ref()
        else {
            return false;
        };
        if self.paths.len() >= MAX_CACHED_GLYPHS {
            self.paths.clear();
        }
        for glyph in glyphs {
            let key = (turn, face.glyph_key(glyph));
            let path = match self.paths.get(&key) {
                Some(path) => path.as_ref().map(Arc::clone),
                None => {
                    let path = match turn {
                        Turn::Sideways => sideways_outline(face, instance.as_ref(), glyph),
                        Turn::Level => level_outline(face, instance.as_ref(), glyph),
                    }
                    .map(Arc::new);
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

fn hinting_instance(
    face: &SizedFace<'_>,
    font: &FontRef<'_>,
    turn: Turn,
) -> Option<HintingInstance> {
    let coords = face.location();
    HintingInstance::new(
        &font.outline_glyphs(),
        Size::new(face.ppem()),
        LocationRef::new(&coords),
        match turn {
            Turn::Sideways => SIDEWAYS_HINTING,
            Turn::Level => LEVEL_HINTING,
        },
    )
    .ok()
}

fn sideways_outline(
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

fn level_outline(
    face: &SizedFace<'_>,
    instance: Option<&HintingInstance>,
    glyph: u32,
) -> Option<BezPath> {
    let font = face.font_ref()?;
    let coords = face.location();
    let outlines = font.outline_glyphs();
    let outline = outlines.get(GlyphId::new(glyph))?;
    let mut pen = Recorder::default();
    let hinted = instance.is_some_and(|instance| {
        outline
            .draw(DrawSettings::hinted(instance, false), &mut pen)
            .is_ok()
    });
    if !hinted {
        pen = Recorder::default();
        let unhinted = DrawSettings::unhinted(Size::new(face.ppem()), LocationRef::new(&coords));
        outline.draw(unhinted, &mut pen).ok()?;
    }
    Some(scan_rows(&pen.path))
}

fn edges(path: &BezPath) -> Vec<(Point, Point)> {
    let mut edges = Vec::new();
    let mut start = Point::ZERO;
    let mut last = Point::ZERO;
    kurbo::flatten(path, FLATTEN_TOLERANCE, |element| match element {
        PathEl::MoveTo(point) => {
            if last != start {
                edges.push((last, start));
            }
            start = point;
            last = point;
        }
        PathEl::LineTo(point) => {
            edges.push((last, point));
            last = point;
        }
        PathEl::ClosePath => {
            if last != start {
                edges.push((last, start));
            }
            last = start;
        }
        _ => {}
    });
    if last != start {
        edges.push((last, start));
    }
    edges
}

fn scan_rows(path: &BezPath) -> BezPath {
    let edges = edges(path);
    let mut rows = BezPath::new();
    let bounds = path.bounding_box();
    let (Some(first), Some(end)) = (
        (-bounds.y1 - 0.5).floor().to_i64(),
        (-bounds.y0 - 0.5).ceil().to_i64(),
    ) else {
        return rows;
    };
    let mut crossings: Vec<(f64, i32)> = Vec::new();
    for row in first..=end {
        let Some(top) = row.to_f64() else {
            continue;
        };
        let centre = -(top + 0.5);
        crossings.clear();
        crossings.extend(
            edges
                .iter()
                .filter(|(from, to)| (from.y <= centre) != (to.y <= centre))
                .map(|&(from, to)| {
                    let across = from.x + (centre - from.y) * (to.x - from.x) / (to.y - from.y);
                    (across, if to.y > from.y { 1 } else { -1 })
                }),
        );
        crossings.sort_by(|left, right| left.0.total_cmp(&right.0));
        let mut winding = 0;
        let mut opened = 0.0;
        for &(across, direction) in &crossings {
            let was = winding;
            winding += direction;
            if was == 0 && winding != 0 {
                opened = across;
            } else if was != 0 && winding == 0 && across > opened {
                rows.extend(Rect::new(opened, top, across, top + 1.0).path_elements(0.1));
            }
        }
    }
    rows
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
