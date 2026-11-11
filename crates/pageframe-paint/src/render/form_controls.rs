use super::ElementCx;
use crate::color::{Color, ToColorColor as _, contrast_ratio};
use anyrender::PaintScene;
use kurbo::{Affine, BezPath, Cap, Join, Rect, Stroke};
use pageframe_dom::CopyStyleGetters as _;
use pageframe_dom::local_name;
use peniko::Fill;
use style::dom::TElement as _;
use style::values::generics::color::ColorOrAuto;

const DEFAULT_ACCENT: Color = Color::from_rgba8(0x00, 0x75, 0xff, 0xff);
const BORDER: Color = Color::from_rgba8(0x76, 0x76, 0x76, 0xff);
const DISABLED_ACCENT: Color = Color::from_rgba8(0xd1, 0xd1, 0xd1, 0xff);
const DISABLED_BACKGROUND: Color = Color::from_rgba8(0xf8, 0xf8, 0xf8, 0xff);
const DISABLED_CHECKMARK: Color = Color::from_rgba8(0xed, 0xed, 0xed, 0xff);
const DARK_FOREGROUND: Color = Color::from_rgba8(0x3b, 0x3b, 0x3b, 0xff);
const MIN_FOREGROUND_CONTRAST: f32 = 3.0;
const CHECKBOX_RADIUS: f64 = 2.0;
const BORDER_WIDTH: f64 = 1.0;
const ARROW_INSET: f64 = 8.0;
const ARROW_HEIGHT: f64 = 7.0;
const ARROW_REACH: f64 = 4.0;
const ARROW_TOP: f64 = 1.0;
const ARROW_STROKE: f64 = 2.0;

struct ControlColors {
    accent: Color,
    border: Color,
    background: Color,
    checked_foreground: Color,
    checked_background: Color,
}

impl ElementCx<'_, '_> {
    pub(super) fn draw_input(&self, scene: &mut impl PaintScene) {
        if self.node.local_name() != "input" {
            return;
        }
        let Some(checked) = self.element.checkbox_input_checked() else {
            return;
        };
        let is_checkbox = match self.element.input_type().as_deref() {
            Some("checkbox") => true,
            Some("radio") => false,
            _ => return,
        };

        let colors = self.control_colors();
        let transform = self.transform * Affine::scale(self.scale);
        let border_box = self.frame.border_box;
        let width = (border_box.width() / self.scale).round();
        let height = (border_box.height() / self.scale).round();
        let side = width.min(height);
        let x = border_box.x0 / self.scale + ((width - side) / 2.0).trunc();
        let y = border_box.y0 / self.scale + ((height - side) / 2.0).trunc();
        let rect = Rect::new(x, y, x + side, y + side);

        if side <= 2.0 {
            scene.fill(Fill::NonZero, transform, colors.border, None, &rect);
            return;
        }

        if is_checkbox {
            draw_checkbox(scene, transform, rect, checked, &colors);
        } else {
            draw_radio(scene, transform, rect, checked, &colors);
        }
    }

    pub(super) fn draw_select_arrow(&self, scene: &mut impl PaintScene) {
        if self.node.local_name() != "select" || self.node.attr(local_name!("multiple")).is_some() {
            return;
        }
        let border_box = self.frame.border_box;
        let padding_box = self.frame.padding_box;
        let height = border_box.height() / self.scale;
        let middle = padding_box.x1 / self.scale - ARROW_INSET;
        let top = border_box.y0 / self.scale + ((height - ARROW_HEIGHT) / 2.0 + 0.5).floor();
        let mut arrow = BezPath::new();
        arrow.move_to((middle - ARROW_REACH, top + ARROW_TOP));
        arrow.line_to((middle, top + ARROW_TOP + ARROW_REACH));
        arrow.line_to((middle + ARROW_REACH, top + ARROW_TOP));
        let color = self.style.clone_color().as_srgb_color();
        let transform = self.transform * Affine::scale(self.scale);
        let stroke = Stroke::new(ARROW_STROKE)
            .with_caps(Cap::Butt)
            .with_join(Join::Miter);
        scene.stroke(&stroke, transform, color, None, &arrow);
    }

    fn control_colors(&self) -> ControlColors {
        if self.node.attr(local_name!("disabled")).is_some() {
            return ControlColors {
                accent: DISABLED_ACCENT,
                border: DISABLED_ACCENT,
                background: DISABLED_BACKGROUND,
                checked_foreground: DISABLED_CHECKMARK,
                checked_background: DISABLED_BACKGROUND,
            };
        }
        let accent = match &self.style.get_inherited_ui().accent_color {
            ColorOrAuto::Auto => DEFAULT_ACCENT,
            ColorOrAuto::Color(color) => color
                .resolve_to_absolute(&self.style.get_inherited_text().color)
                .as_srgb_color(),
        };
        let checked_foreground = if contrast_ratio(accent, Color::WHITE) < MIN_FOREGROUND_CONTRAST {
            DARK_FOREGROUND
        } else {
            Color::WHITE
        };
        ControlColors {
            accent,
            border: BORDER,
            background: Color::WHITE,
            checked_foreground,
            checked_background: if checked_foreground == DARK_FOREGROUND {
                DARK_FOREGROUND
            } else {
                Color::WHITE
            },
        }
    }
}

fn draw_checkbox(
    scene: &mut impl PaintScene,
    transform: Affine,
    rect: Rect,
    checked: bool,
    colors: &ControlColors,
) {
    if !checked {
        let background = rect
            .inset(-0.2 * BORDER_WIDTH)
            .to_rounded_rect(CHECKBOX_RADIUS);
        scene.fill(
            Fill::NonZero,
            transform,
            colors.background,
            None,
            &background,
        );
        let border = rect
            .inset(-BORDER_WIDTH / 2.0)
            .to_rounded_rect(CHECKBOX_RADIUS);
        scene.stroke(
            &Stroke::new(BORDER_WIDTH),
            transform,
            colors.border,
            None,
            &border,
        );
        return;
    }

    let background = rect.to_rounded_rect(CHECKBOX_RADIUS);
    scene.fill(Fill::NonZero, transform, colors.accent, None, &background);

    let (w, h) = (rect.width(), rect.height());
    let mut check = BezPath::new();
    check.move_to((rect.x0 + w * 0.2, rect.center().y));
    check.line_to((rect.x0 + w * 0.4, rect.center().y + h * 0.2));
    check.line_to((rect.x1 - w * 0.2, rect.y0 + h * 0.2));
    let stroke = Stroke::new(h * 0.16)
        .with_caps(Cap::Butt)
        .with_join(Join::Miter)
        .with_miter_limit(4.0);
    scene.stroke(&stroke, transform, colors.checked_foreground, None, &check);
}

fn draw_radio(
    scene: &mut impl PaintScene,
    transform: Affine,
    rect: Rect,
    checked: bool,
    colors: &ControlColors,
) {
    let (background, border) = if checked {
        (colors.checked_background, colors.accent)
    } else {
        (colors.background, colors.border)
    };
    let fill = rect.inset(-0.2 * BORDER_WIDTH).to_ellipse();
    scene.fill(Fill::NonZero, transform, background, None, &fill);
    let ring = rect.inset(-BORDER_WIDTH / 2.0).to_ellipse();
    scene.stroke(&Stroke::new(BORDER_WIDTH), transform, border, None, &ring);

    if checked {
        let dot = rect.inset(-rect.width() * 0.2).to_ellipse();
        scene.fill(Fill::NonZero, transform, colors.accent, None, &dot);
    }
}
