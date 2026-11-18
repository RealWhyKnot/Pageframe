use anyrender::PaintScene;
use kurbo::{Insets, Rect};
use pageframe_dom::{ColumnLayout, CopyStyleGetters as _};
use style::servo_arc::Arc;
use style::values::computed::BorderStyle;

use crate::color::ToColorColor as _;
use crate::kurbo_css::{CssBox, NonUniformRoundedRectRadii};
use crate::render::{ElementCx, pixel_phase};

impl ElementCx<'_, '_> {
    pub(crate) fn draw_column_rules(&self, scene: &mut impl PaintScene, columns: &ColumnLayout) {
        let column = self.style.get_column();
        let style = match column.column_rule_style {
            BorderStyle::Inset => BorderStyle::Ridge,
            BorderStyle::Outset => BorderStyle::Groove,
            other => other,
        };
        let width =
            super::table_borders::device_width(column.column_rule_width.0.to_f64_px() * self.scale);
        if matches!(style, BorderStyle::None | BorderStyle::Hidden) || width <= 0.0 {
            return;
        }
        let color = column
            .column_rule_color
            .resolve_to_absolute(&self.style.clone_color())
            .as_srgb_color();
        let origin = self.transform.translation();
        let snap_x = |x: f64| (origin.x + x).round() - origin.x;
        let snap_y = |y: f64| (origin.y + y).round() - origin.y;
        let is_vertical = columns.is_vertical();
        let (span_start, span_end) = columns.rule_span();
        let span_snap = |value: f64| {
            if is_vertical {
                snap_x(value)
            } else {
                snap_y(value)
            }
        };
        let span_start = span_snap(f64::from(span_start) * self.scale);
        let span_end = span_snap(f64::from(span_end) * self.scale);
        let hidden = BorderStyle::None;
        for index in 1..columns.fragments.len() {
            let centre = f64::from(columns.rule_x(index)) * self.scale;
            let (rect, insets, sides) = if is_vertical {
                let top = snap_y(centre - width / 2.0);
                (
                    Rect::new(span_start, top, span_end, top + width),
                    Insets::new(0.0, width, 0.0, 0.0),
                    [style, hidden, hidden, hidden],
                )
            } else {
                let left = snap_x(centre - width / 2.0);
                (
                    Rect::new(left, span_start, left + width, span_end),
                    Insets::new(width, 0.0, 0.0, 0.0),
                    [hidden, hidden, hidden, style],
                )
            };
            let frame = CssBox::new(
                rect,
                insets,
                Insets::ZERO,
                0.0,
                NonUniformRoundedRectRadii::default(),
            )
            .on_pixels(pixel_phase(self.transform));
            let rule = ElementCx {
                frame,
                style: Arc::clone(&self.style),
                ..*self
            };
            rule.draw_sides(scene, sides, [color; 4], false);
        }
    }
}
