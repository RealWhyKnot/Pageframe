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
        let width = column.column_rule_width.0.to_f64_px() * self.scale;
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
        let top = snap_y(f64::from(columns.content_top) * self.scale);
        let bottom = snap_y(f64::from(columns.content_top + columns.height) * self.scale);
        for index in 1..columns.fragments.len() {
            let centre = f64::from(columns.rule_x(index)) * self.scale;
            let left = snap_x(centre - width / 2.0);
            let frame = CssBox::new(
                Rect::new(left, top, left + width, bottom),
                Insets::new(width, 0.0, 0.0, 0.0),
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
            let hidden = BorderStyle::None;
            rule.draw_sides(scene, [hidden, hidden, hidden, style], [color; 4], false);
        }
    }
}
