use anyrender::PaintScene;
use pageframe_dom::CopyStyleGetters as _;
use peniko::Fill;
use style::servo_arc::Arc;
use style::values::computed::{BorderStyle, OutlineStyle};

use crate::{color::ToColorColor as _, render::ElementCx};

impl ElementCx<'_, '_> {
    pub(crate) fn draw_outline(&self, scene: &mut impl PaintScene) {
        let outline = self.style.get_outline();
        let current_color = self.style.clone_color();
        let color = outline
            .outline_color
            .resolve_to_absolute(&current_color)
            .as_srgb_color();
        let style = match outline.outline_style {
            OutlineStyle::Auto => return,
            OutlineStyle::BorderStyle(style) => style,
        };
        if matches!(style, BorderStyle::None | BorderStyle::Hidden)
            || self.frame.outline_width <= 0.0
        {
            return;
        }
        let offset = (outline.outline_offset.to_f64_px() * self.scale).trunc();
        if style == BorderStyle::Solid {
            let path = self.frame.outline(offset);
            scene.fill(Fill::EvenOdd, self.transform, color, None, &path);
            return;
        }
        let Some(frame) = self.frame.outline_frame(offset) else {
            return;
        };
        let ring = ElementCx {
            frame,
            style: Arc::clone(&self.style),
            ..*self
        };
        ring.draw_sides(scene, [style; 4], [color; 4]);
    }
}
