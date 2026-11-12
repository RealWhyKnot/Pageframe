use std::sync::PoisonError;

use anyrender::{Glyph, Paint, PaintScene};
use kurbo::{Affine, Rect, Vec2};
use pageframe_dom::{PopupLayout, PopupRow};
use parley::{FontFamily, FontWeight, LayoutContext, PositionedLayoutItem, StyleProperty};
use peniko::{Color, Fill};

use super::PageframeDomPainter;

const BORDER: f64 = 1.0;
const TEXT_INSET: f64 = 4.0;
const GROUP_INDENT: f64 = 16.0;
const BORDER_COLOR: Color = Color::from_rgb8(118, 118, 118);
const HIGHLIGHT: Color = Color::from_rgb8(25, 103, 210);
const DISABLED_TEXT: Color = Color::from_rgb8(109, 109, 109);

fn text_color(row: &PopupRow) -> Color {
    if row.highlighted {
        Color::WHITE
    } else if row.disabled && !row.heading {
        DISABLED_TEXT
    } else {
        Color::BLACK
    }
}

impl PageframeDomPainter<'_, '_> {
    pub(super) fn draw_select_popup(&self, scene: &mut impl PaintScene) {
        let Some(popup) = self.dom.as_ref().select_popup_layout() else {
            return;
        };
        let scroll = self.dom.as_ref().viewport_scroll();
        let transform = Affine::translate(Vec2 {
            x: self.initial_x - scroll.x * self.scale,
            y: self.initial_y - scroll.y * self.scale,
        }) * Affine::scale(self.scale);
        let outer = Rect::new(
            popup.x,
            popup.y,
            popup.x + popup.width,
            popup.y + popup.height(),
        );
        let inner = outer.inset(-BORDER);
        scene.fill(Fill::NonZero, transform, BORDER_COLOR, None, &outer);
        scene.fill(Fill::NonZero, transform, Color::WHITE, None, &inner);
        scene.push_clip_layer(Fill::NonZero, transform, &inner);
        let mut layouts = LayoutContext::<()>::new();
        let mut top = inner.y0;
        for row in &popup.rows {
            if row.highlighted {
                let band = Rect::new(inner.x0, top, inner.x1, top + popup.row_height);
                scene.fill(Fill::NonZero, transform, HIGHLIGHT, None, &band);
            }
            self.draw_popup_row(scene, &mut layouts, &popup, row, transform, top);
            top += popup.row_height;
        }
        scene.pop_layer();
    }

    fn draw_popup_row(
        &self,
        scene: &mut impl PaintScene,
        layouts: &mut LayoutContext<()>,
        popup: &PopupLayout,
        row: &PopupRow,
        transform: Affine,
        top: f64,
    ) {
        if row.label.is_empty() {
            return;
        }
        let fonts = self.dom.as_ref().font_context();
        let mut fonts = fonts.lock().unwrap_or_else(PoisonError::into_inner);
        let mut builder = layouts.ranged_builder(&mut fonts, &row.label, 1.0, false);
        builder.push_default(FontFamily::from(popup.font_family.as_str()));
        builder.push_default(StyleProperty::FontSize(popup.font_size));
        if row.heading {
            builder.push_default(StyleProperty::FontWeight(FontWeight::BOLD));
        }
        let mut layout = builder.build(&row.label);
        drop(fonts);
        layout.break_all_lines(None);
        let indent = if row.indented { GROUP_INDENT } else { 0.0 };
        let left = popup.x + BORDER + TEXT_INSET + indent;
        let slack = (popup.row_height - f64::from(layout.height())) / 2.0;
        let paint = Paint::from(text_color(row));
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let run = glyph_run.run();
                let origin = Vec2::new(left, top + slack);
                let slant = run
                    .synthesis()
                    .skew()
                    .map(|angle| Affine::skew(f64::from(angle.to_radians().tan()), 0.0));
                scene.draw_glyphs(
                    run.font(),
                    run.font_size(),
                    true,
                    bytemuck::cast_slice(run.normalized_coords()),
                    Vec2::ZERO,
                    Fill::NonZero,
                    &paint,
                    1.0,
                    transform * Affine::translate(origin),
                    slant,
                    glyph_run.positioned_glyphs().map(|glyph| Glyph {
                        id: glyph.id,
                        x: glyph.x,
                        y: glyph.y,
                    }),
                );
            }
        }
    }
}
