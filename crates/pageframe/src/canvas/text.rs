use std::sync::PoisonError;

use anyrender::{Glyph, Paint, PaintScene};
use kurbo::{Affine, Vec2};
use parley::{FontFamily, FontStyle, FontWeight, Layout, PositionedLayoutItem, StyleProperty};
use peniko::Fill;

use super::{Canvas, finite};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextBaseline {
    #[default]
    Alphabetic,
    Top,
    Middle,
    Bottom,
}

impl Canvas {
    pub fn set_font(&mut self, family: &str, size: f32) -> &mut Self {
        if size.is_finite() && size > 0.0 {
            family.clone_into(&mut self.state.font_family);
            self.state.font_size = size;
        }
        self
    }

    pub fn set_font_weight(&mut self, weight: f32) -> &mut Self {
        if (1.0..=1000.0).contains(&weight) {
            self.state.font_weight = weight;
        }
        self
    }

    pub fn set_font_italic(&mut self, italic: bool) -> &mut Self {
        self.state.font_italic = italic;
        self
    }

    pub fn set_text_align(&mut self, align: TextAlign) -> &mut Self {
        self.state.text_align = align;
        self
    }

    pub fn set_text_baseline(&mut self, baseline: TextBaseline) -> &mut Self {
        self.state.text_baseline = baseline;
        self
    }

    pub fn measure_text(&mut self, text: &str) -> f64 {
        if text.is_empty() {
            return 0.0;
        }
        f64::from(self.shape(text).full_width())
    }

    pub fn fill_text(&mut self, text: &str, x: f64, y: f64) -> &mut Self {
        if text.is_empty() || !finite(&[x, y]) {
            return self;
        }
        let layout = self.shape(text);
        let width = f64::from(layout.full_width());
        let shift_x = match self.state.text_align {
            TextAlign::Left => 0.0,
            TextAlign::Center => -width / 2.0,
            TextAlign::Right => -width,
        };
        let paint = Paint::from(self.state.fill.brush(self.state.alpha));
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let run = glyph_run.run();
                let metrics = run.font_metrics();
                let height = metrics.ascent + metrics.descent;
                let em = if height > 0.0 {
                    run.font_size() / height
                } else {
                    1.0
                };
                let (ascent, descent) = (metrics.ascent * em, metrics.descent * em);
                let shift_y = match self.state.text_baseline {
                    TextBaseline::Alphabetic => 0.0,
                    TextBaseline::Top => ascent,
                    TextBaseline::Middle => (ascent - descent) / 2.0,
                    TextBaseline::Bottom => -descent,
                };
                let origin = Vec2::new(
                    x + shift_x,
                    y + f64::from(shift_y) - f64::from(glyph_run.baseline()),
                );
                let slant = run
                    .synthesis()
                    .skew()
                    .map(|angle| Affine::skew(f64::from(angle.to_radians().tan()), 0.0));
                self.scene.draw_glyphs(
                    run.font(),
                    run.font_size(),
                    true,
                    bytemuck::cast_slice(run.normalized_coords()),
                    Vec2::ZERO,
                    Fill::NonZero,
                    &paint,
                    1.0,
                    self.state.transform * Affine::translate(origin),
                    slant,
                    glyph_run.positioned_glyphs().map(|glyph| Glyph {
                        id: glyph.id,
                        x: glyph.x,
                        y: glyph.y,
                    }),
                );
            }
        }
        self
    }

    fn shape(&mut self, text: &str) -> Layout<()> {
        let text: String = text
            .chars()
            .map(|ch| {
                if matches!(ch, '\t' | '\n' | '\u{c}' | '\r') {
                    ' '
                } else {
                    ch
                }
            })
            .collect();
        let mut fonts = self.fonts.lock().unwrap_or_else(PoisonError::into_inner);
        let mut builder = self.layouts.ranged_builder(&mut fonts, &text, 1.0, false);
        builder.push_default(FontFamily::from(self.state.font_family.as_str()));
        builder.push_default(StyleProperty::FontSize(self.state.font_size));
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(
            self.state.font_weight,
        )));
        builder.push_default(StyleProperty::FontStyle(if self.state.font_italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        }));
        let mut layout = builder.build(&text);
        drop(fonts);
        layout.break_all_lines(None);
        layout
    }
}
