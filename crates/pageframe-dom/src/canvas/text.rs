use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use kurbo::{Affine, BezPath, Rect, Shape};
use parley::{FontContext, LayoutContext, PositionedLayoutItem};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::types::F2Dot14;
use skrifa::raw::{FontRef, TableProvider};
use skrifa::{GlyphId, MetadataProvider};

use super::font::{FontSlant, FontSpec};

const CACHE_LIMIT: usize = 512;
const HANGING_RATIO: f64 = 0.8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Start,
    End,
    Left,
    Right,
    Center,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextBaseline {
    Top,
    Hanging,
    Middle,
    #[default]
    Alphabetic,
    Ideographic,
    Bottom,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct FontMetrics {
    pub(crate) ascent: f64,
    pub(crate) descent: f64,
    pub(crate) em_ascent: f64,
    pub(crate) em_descent: f64,
}

impl FontMetrics {
    pub(crate) fn baseline(&self, baseline: TextBaseline) -> f64 {
        match baseline {
            TextBaseline::Top => self.em_ascent,
            TextBaseline::Hanging => self.ascent * HANGING_RATIO,
            TextBaseline::Middle => (self.em_ascent - self.em_descent) / 2.0,
            TextBaseline::Alphabetic => 0.0,
            TextBaseline::Ideographic => -self.descent,
            TextBaseline::Bottom => -self.em_descent,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Shaped {
    pub(crate) width: f64,
    pub(crate) outline: BezPath,
    pub(crate) ink: Rect,
    pub(crate) metrics: FontMetrics,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextMetrics {
    pub width: f64,
    pub actual_left: f64,
    pub actual_right: f64,
    pub actual_ascent: f64,
    pub actual_descent: f64,
    pub font_ascent: f64,
    pub font_descent: f64,
    pub em_ascent: f64,
    pub em_descent: f64,
    pub hanging_baseline: f64,
    pub alphabetic_baseline: f64,
    pub ideographic_baseline: f64,
}

thread_local! {
    static LAYOUT: RefCell<LayoutContext<()>> = RefCell::new(LayoutContext::new());
    static CACHE: RefCell<HashMap<(String, String), Rc<Shaped>>> = RefCell::new(HashMap::new());
}

pub(crate) fn prepare(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\t' | '\n' | '\u{000C}' | '\r' => ' ',
            c => c,
        })
        .collect()
}

pub(crate) fn shape(fonts: &mut FontContext, spec: &FontSpec, text: &str) -> Rc<Shaped> {
    let key = (spec.serialize(), text.to_string());
    if let Some(hit) = CACHE.with(|cache| cache.borrow().get(&key).cloned()) {
        return hit;
    }
    let shaped = Rc::new(shape_uncached(fonts, spec, text));
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(key, Rc::clone(&shaped));
    });
    shaped
}

fn shape_uncached(fonts: &mut FontContext, spec: &FontSpec, text: &str) -> Shaped {
    let probe = if text.is_empty() { " " } else { text };
    let mut layout = LAYOUT.with(|layout| {
        let mut layout = layout.borrow_mut();
        let mut builder = layout.ranged_builder(fonts, probe, 1.0, false);
        builder.push_default(parley::StyleProperty::FontFamily(
            parley::FontFamily::Source(Cow::Borrowed(&spec.family)),
        ));
        builder.push_default(parley::StyleProperty::FontSize(spec.size));
        builder.push_default(parley::StyleProperty::FontWeight(parley::FontWeight::new(
            spec.weight,
        )));
        builder.push_default(parley::StyleProperty::FontWidth(
            parley::FontWidth::from_percentage(spec.stretch),
        ));
        builder.push_default(parley::StyleProperty::FontStyle(match spec.slant {
            FontSlant::Normal => parley::FontStyle::Normal,
            FontSlant::Italic => parley::FontStyle::Italic,
            FontSlant::Oblique => parley::FontStyle::Oblique(None),
        }));
        builder.build(probe)
    });
    layout.break_all_lines(None);
    let mut outline = BezPath::new();
    let mut ink: Option<Rect> = None;
    let mut metrics = None;
    let mut baseline = None;
    for line in layout.lines() {
        let line_baseline = f64::from(line.metrics().baseline);
        let base = *baseline.get_or_insert(line_baseline);
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };
            let run = glyph_run.run();
            let font = run.font();
            let Ok(font_ref) = FontRef::from_index(font.data.as_ref(), font.index) else {
                continue;
            };
            let size = run.font_size();
            let coords: Vec<F2Dot14> = run
                .normalized_coords()
                .iter()
                .map(|coord| F2Dot14::from_bits(coord.to_bits()))
                .collect();
            if metrics.is_none() {
                metrics = Some(font_metrics(&font_ref, size, &coords));
            }
            if text.is_empty() {
                continue;
            }
            let skew = run
                .synthesis()
                .skew()
                .map_or(0.0, |deg| f64::from(deg).to_radians().tan());
            let outlines = font_ref.outline_glyphs();
            for glyph in glyph_run.positioned_glyphs() {
                let Some(shape) = outlines.get(GlyphId::new(glyph.id)) else {
                    continue;
                };
                let (gx, gy) = (f64::from(glyph.x), f64::from(glyph.y) - base);
                let mut path = BezPath::new();
                let mut pen = Pen {
                    path: &mut path,
                    transform: Affine::new([1.0, 0.0, skew, -1.0, gx, gy]),
                };
                let settings = DrawSettings::unhinted(Size::new(size), LocationRef::new(&coords));
                let _ = shape.draw(settings, &mut pen);
                if path.is_empty() {
                    continue;
                }
                let local = path.bounding_box();
                let bounds = Rect::new(
                    gx + (local.x0 - gx).floor(),
                    local.y0,
                    gx + (local.x1 - gx).ceil(),
                    local.y1,
                );
                ink = Some(ink.map_or(bounds, |r: Rect| r.union(bounds)));
                outline.extend(path);
            }
        }
    }
    let width = if text.is_empty() {
        0.0
    } else {
        f64::from(layout.full_width())
    };
    let ink = ink.map_or(Rect::ZERO, |r| {
        Rect::new(r.x0, -(-r.y0 - 0.4).ceil(), r.x1, (r.y1 - 0.5).ceil())
    });
    Shaped {
        width,
        outline,
        ink,
        metrics: metrics.unwrap_or_else(|| fallback_metrics(spec.size)),
    }
}

fn fallback_metrics(size: f32) -> FontMetrics {
    let size = f64::from(size);
    FontMetrics {
        ascent: (size * 0.9).round(),
        descent: (size * 0.25).round(),
        em_ascent: size * 0.8,
        em_descent: size * 0.2,
    }
}

fn font_metrics(font: &FontRef<'_>, size: f32, coords: &[F2Dot14]) -> FontMetrics {
    let scaled = font.metrics(Size::new(size), LocationRef::new(coords));
    let size = f64::from(size);
    let ascent = f64::from(scaled.ascent).round();
    let descent = f64::from(-scaled.descent).round();
    let typo = font
        .os2()
        .ok()
        .map(|os2| {
            (
                f64::from(os2.s_typo_ascender()),
                f64::from(-os2.s_typo_descender()),
            )
        })
        .filter(|(a, d)| a + d > 0.0);
    let (a, d) = typo.unwrap_or((ascent, descent));
    let total = a + d;
    let snap = |v: f64| (v * 64.0).round() / 64.0;
    let (em_ascent, em_descent) = if total > 0.0 {
        (snap(size * a / total), snap(size * d / total))
    } else {
        (size * 0.8, size * 0.2)
    };
    FontMetrics {
        ascent,
        descent,
        em_ascent,
        em_descent,
    }
}

pub(crate) fn align_offset(align: TextAlign, width: f64) -> f64 {
    match align {
        TextAlign::Start | TextAlign::Left => 0.0,
        TextAlign::Center => -width / 2.0,
        TextAlign::End | TextAlign::Right => -width,
    }
}

pub(crate) fn measure(shaped: &Shaped, align: TextAlign, baseline: TextBaseline) -> TextMetrics {
    let m = shaped.metrics;
    let y = m.baseline(baseline);
    let dx = -align_offset(align, shaped.width);
    let (left, right, top, bottom) = if shaped.outline.is_empty() {
        (0.0, 0.0, 0.0, 0.0)
    } else {
        (shaped.ink.x0, shaped.ink.x1, shaped.ink.y0, shaped.ink.y1)
    };
    TextMetrics {
        width: shaped.width,
        actual_left: -left + dx,
        actual_right: right - dx,
        actual_ascent: -top - y,
        actual_descent: bottom + y,
        font_ascent: m.ascent - y,
        font_descent: m.descent + y,
        em_ascent: m.em_ascent - y,
        em_descent: m.em_descent + y,
        hanging_baseline: m.ascent * HANGING_RATIO - y,
        alphabetic_baseline: -y,
        ideographic_baseline: -m.descent - y,
    }
}

struct Pen<'a> {
    path: &'a mut BezPath,
    transform: Affine,
}

impl Pen<'_> {
    fn point(&self, x: f32, y: f32) -> kurbo::Point {
        self.transform * kurbo::Point::new(f64::from(x), f64::from(y))
    }
}

impl OutlinePen for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.point(x, y);
        self.path.move_to(p);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.point(x, y);
        self.path.line_to(p);
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (c, p) = (self.point(cx0, cy0), self.point(x, y));
        self.path.quad_to(c, p);
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (c0, c1, p) = (self.point(cx0, cy0), self.point(cx1, cy1), self.point(x, y));
        self.path.curve_to(c0, c1, p);
    }

    fn close(&mut self) {
        self.path.close_path();
    }
}
