use anyrender::PaintScene;
use pageframe_dom::CopyStyleGetters;
use peniko::{Fill, Mix};
use style::values::computed::BorderStyle;

use crate::{
    color::ToColorColor as _,
    render::{ALMOST_OPAQUE, ElementCx},
};

impl ElementCx<'_, '_> {
    pub(crate) fn clips_dashed_background(&self) -> bool {
        if !self.frame.has_border_radius()
            || Some(self.node.id) == self.context.root_element_id
            || self.has_native_frame()
            || self.has_collapsed_borders()
        {
            return false;
        }
        let border = self.style.get_border();
        let width = self.frame.border_width;
        let current_color = self.style.clone_color();
        let color = border
            .border_top_color
            .resolve_to_absolute(&current_color)
            .as_srgb_color();
        let sides = [
            (border.border_top_style, &border.border_top_color, width.y0),
            (
                border.border_right_style,
                &border.border_right_color,
                width.x1,
            ),
            (
                border.border_bottom_style,
                &border.border_bottom_color,
                width.y1,
            ),
            (
                border.border_left_style,
                &border.border_left_color,
                width.x0,
            ),
        ];
        width.y0 > 0.0
            && color.components[3] > 0.0
            && sides.into_iter().all(|(line, side_color, side_width)| {
                line == BorderStyle::Dashed
                    && side_width == width.y0
                    && side_color
                        .resolve_to_absolute(&current_color)
                        .as_srgb_color()
                        == color
            })
    }

    pub(crate) fn begin_dashed_background(&self, scene: &mut impl PaintScene) {
        scene.push_layer(
            Fill::NonZero,
            Mix::Normal,
            ALMOST_OPAQUE,
            self.transform,
            &self.frame.border_box_path(),
            None,
            None,
        );
    }
}
