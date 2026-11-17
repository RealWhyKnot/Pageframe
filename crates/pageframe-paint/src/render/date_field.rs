use super::ElementCx;
use crate::color::Color;
use crate::text::TextPaintOptions;
use anyrender::PaintScene;
use kurbo::{Affine, BezPath, Rect};
use pageframe_dom::node::{DateField, DateIndicator, SpecialElementData};
use peniko::{BlendMode, Fill};

const ICON_VIEWBOX: f64 = 24.0;
const LAYOUT_UNIT: f64 = 64.0;
const CALENDAR_WIDTH: f64 = 16.0;
const CALENDAR_HEIGHT: f64 = 15.0;
const CALENDAR_ICON: &[&str] = &[
    "M20 3h-1V1h-2v2H7V1H5v2H4c-1.1 0-2 .9-2 2v16c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm0 18H4V8h16v13z",
];
const CLOCK_ICON: &[&str] = &[
    "M11.99 2C6.47 2 2 6.48 2 12s4.47 10 9.99 10C17.52 22 22 17.52 22 12S17.52 2 11.99 2zM12 20c-4.42 0-8-3.58-8-8s3.58-8 8-8 8 3.58 8 8-3.58 8-8 8z",
    "M12.5 7H11v6l5.25 3.15.75-1.23-4.5-2.67z",
];
const WINDOW_TEXT: Color = Color::from_rgba8(0x00, 0x00, 0x00, 0xff);

impl ElementCx<'_, '_> {
    pub(super) fn draw_date_field(&self, scene: &mut impl PaintScene) {
        let SpecialElementData::DateField(field) = &self.element.special_data else {
            return;
        };
        let content = self.frame.content_box;
        let height = content.height() / self.scale;
        let edit_top = (height - f64::from(field.edit_height())) / 2.0;
        let edit_right =
            (content.x1 - f64::from(field.indicator_width()) * self.scale).max(content.x0);
        let clip = Rect::new(
            content.x0,
            content.y0 + edit_top * self.scale,
            edit_right,
            content.y0 + (edit_top + f64::from(field.edit_height())) * self.scale,
        );
        let translation = self.transform.translation();
        let baseline = content.y0
            + (edit_top + f64::from(DateField::FIELD_PADDING + field.baseline)) * self.scale;
        let snapped = (translation.y + baseline).round() - translation.y;
        let line_baseline = field
            .layout
            .layout
            .lines()
            .next()
            .map_or(0.0, |line| f64::from(line.metrics().baseline));
        let transform = self.transform * Affine::translate((content.x0, snapped - line_baseline));
        self.context.layer_manager.maybe_with_layer(
            scene,
            true,
            BlendMode::default(),
            1.0,
            self.transform,
            &clip,
            None,
            None,
            |scene| {
                crate::text::stroke_text(
                    scene,
                    field.layout.layout.lines(),
                    self.context.dom,
                    transform,
                    TextPaintOptions {
                        scale: self.scale,
                        brush_override: None,
                        ellipsis: None,
                        text: None,
                        group: None,
                        flat_ancestors: true,
                    },
                    self.node.id,
                    &mut self.context.draw_text_context.borrow_mut(),
                );
            },
        );
        if field.indicator.visible {
            self.draw_date_indicator(scene, field.indicator);
        }
    }

    fn draw_date_indicator(&self, scene: &mut impl PaintScene, indicator: DateIndicator) {
        let content = self.frame.content_box;
        let size = f64::from(indicator.size);
        let padding = f64::from(indicator.padding);
        let outer = size + padding * 2.0;
        let height = content.height() / self.scale;
        let left = content.x1 / self.scale - outer + padding;
        let top = content.y0 / self.scale + (height - outer) / 2.0 + padding;
        let (paths, aspect) = if indicator.clock {
            (CLOCK_ICON, 1.0)
        } else {
            (CALENDAR_ICON, CALENDAR_HEIGHT / CALENDAR_WIDTH)
        };
        let translation = self.transform.translation();
        let quantize = |value: f64| (value * LAYOUT_UNIT).round() / LAYOUT_UNIT;
        let snap = |value: f64, offset: f64| quantize(value * self.scale + offset).round() - offset;
        let device_left = snap(left, translation.x);
        let device_top = snap(top, translation.y);
        let device_width = quantize(size * self.scale).round_ties_even();
        let device_height = quantize(size * aspect * self.scale).round_ties_even();
        let unit = device_width.min(device_height) / ICON_VIEWBOX;
        let transform = self.transform
            * Affine::translate((
                device_left + (device_width - unit * ICON_VIEWBOX) / 2.0,
                device_top + (device_height - unit * ICON_VIEWBOX) / 2.0,
            ))
            * Affine::scale(unit);
        for data in paths {
            if let Ok(path) = BezPath::from_svg(data) {
                scene.fill(Fill::NonZero, transform, WINDOW_TEXT, None, &path);
            }
        }
    }
}
