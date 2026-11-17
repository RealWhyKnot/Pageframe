use super::ElementCx;
use super::pixels::device_space;
use crate::color::{Color, ToColorColor as _, contrast_ratio};
use anyrender::PaintScene;
use kurbo::{Affine, BezPath, Cap, Join, Rect, Stroke};
use pageframe_dom::CopyStyleGetters as _;
use pageframe_dom::{GaugeRegion, local_name};
use peniko::Fill;
use style::dom::TElement as _;
use style::values::computed::Appearance;
use style::values::generics::color::ColorOrAuto;

const UNZOOM_SLACK: f64 = 1.0 / 1024.0;

const DEFAULT_ACCENT: Color = Color::from_rgba8(0x00, 0x75, 0xff, 0xff);
const BORDER: Color = Color::from_rgba8(0x76, 0x76, 0x76, 0xff);
const DISABLED_ACCENT: Color = Color::from_rgba8(0xd1, 0xd1, 0xd1, 0xff);
const DISABLED_BACKGROUND: Color = Color::from_rgba8(0xf8, 0xf8, 0xf8, 0xff);
const DISABLED_CHECKMARK: Color = Color::from_rgba8(0xed, 0xed, 0xed, 0xff);
const DARK_FOREGROUND: Color = Color::from_rgba8(0x3b, 0x3b, 0x3b, 0xff);
const MIN_FOREGROUND_CONTRAST: f32 = 3.0;
const CHECKBOX_RADIUS: f64 = 2.0;
const BORDER_WIDTH: f64 = 1.0;
const ARROW_BOX: f64 = 15.0;
const ARROW_SIZE: f64 = 8.0;
const ARROW_STROKE: f64 = 2.0;
const SWATCH_INSET_X: f64 = 2.0;
const SWATCH_INSET_Y: f64 = 4.0;
const SWATCH_BORDER: Color = Color::from_rgba8(0x77, 0x77, 0x77, 0xff);
const SLIDER_TRACK_THICKNESS: f64 = 8.0;
const SLIDER_THUMB_SIZE: f64 = 16.0;
const THUMB_INSET: f64 = 0.5;
const TRACK_FILL: Color = Color::from_rgba8(0xef, 0xef, 0xef, 0xff);
const TRACK_BORDER: Color = Color::from_rgba8(0x76, 0x76, 0x76, 0x80);
const DISABLED_SLIDER: Color = Color::from_rgba8(0xcb, 0xcb, 0xcb, 0xff);
const DISABLED_TRACK_FILL: Color = Color::from_rgba8(0xfa, 0xfa, 0xfa, 0xff);
const DISABLED_TRACK_BORDER: Color = Color::from_rgba8(0x76, 0x76, 0x76, 0x4d);
const PROGRESS_RADIUS: f64 = 40.0;
const PROGRESS_ACTIVITY_BLOCKS: f64 = 5.0;
const METER_RADIUS: f64 = 20.0;
const METER_BORDER: Color = Color::from_rgba8(0x76, 0x76, 0x76, 0x4d);
const METER_OPTIMUM: Color = Color::from_rgba8(0x10, 0x7c, 0x10, 0xff);
const METER_SUBOPTIMUM: Color = Color::from_rgba8(0xff, 0xb9, 0x00, 0xff);
const METER_EVEN_LESS_GOOD: Color = Color::from_rgba8(0xd8, 0x3b, 0x01, 0xff);

struct ControlColors {
    accent: Color,
    border: Color,
    background: Color,
    checked_foreground: Color,
    checked_background: Color,
}

impl ElementCx<'_, '_> {
    pub(super) fn draw_input(&self, scene: &mut impl PaintScene) {
        match self.node.local_name().as_ref() {
            "input" => {}
            "progress" => return self.draw_progress(scene),
            "meter" => return self.draw_meter(scene),
            _ => return,
        }
        match self.element.input_type().as_deref() {
            Some("color") => return self.draw_color_swatch(scene),
            Some("range") => return self.draw_range(scene),
            Some("date" | "time" | "datetime-local" | "month" | "week") => {
                return self.draw_date_field(scene);
            }
            _ => {}
        }
        let Some(checked) = self.element.checkbox_input_checked() else {
            return;
        };
        let is_checkbox = match self.element.input_type().as_deref() {
            Some("checkbox") => true,
            Some("radio") => false,
            _ => return,
        };
        if self.style.get_box().appearance == Appearance::None {
            return;
        }

        let colors = self.control_colors();
        let (device_transform, border_box) = device_space(self.transform, self.frame.border_box);
        let transform = device_transform * Affine::scale(self.scale);
        let unzoomed = |length: f64| (length / self.scale + UNZOOM_SLACK).trunc();
        let width = unzoomed(border_box.width());
        let height = unzoomed(border_box.height());
        let side = width.min(height);
        let x = border_box.x0 / self.scale + (width - side) / 2.0;
        let y = border_box.y0 / self.scale + (height - side) / 2.0;
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

    fn draw_color_swatch(&self, scene: &mut impl PaintScene) {
        let content = self.frame.content_box;
        let (inset_x, inset_y) = (SWATCH_INSET_X * self.scale, SWATCH_INSET_Y * self.scale);
        let outer = Rect::new(
            content.x0 + inset_x,
            content.y0 + inset_y,
            content.x1 - inset_x,
            content.y1 - inset_y,
        );
        if outer.width() <= 0.0 || outer.height() <= 0.0 {
            return;
        }
        let fill = self
            .node
            .attr(local_name!("value"))
            .and_then(simple_color)
            .unwrap_or(Color::BLACK);
        scene.fill(Fill::NonZero, self.transform, SWATCH_BORDER, None, &outer);
        let inner = outer.inset(-BORDER_WIDTH * self.scale);
        if inner.width() > 0.0 && inner.height() > 0.0 {
            scene.fill(Fill::NonZero, self.transform, fill, None, &inner);
        }
    }

    fn draw_range(&self, scene: &mut impl PaintScene) {
        let (track_fill, value_fill, track_border) = if self.node.is_disabled() {
            (DISABLED_TRACK_FILL, DISABLED_SLIDER, DISABLED_TRACK_BORDER)
        } else {
            (TRACK_FILL, self.control_colors().accent, TRACK_BORDER)
        };
        let border_box = self.frame.border_box;
        let content = self.frame.content_box;
        let half_track = SLIDER_TRACK_THICKNESS * self.scale / 2.0;
        let center = border_box.center().y;
        let track = Rect::new(
            border_box.x0,
            border_box.y0.max(center - half_track),
            border_box.x1,
            border_box.y1.min(center + half_track),
        );
        let thumb_size = SLIDER_THUMB_SIZE * self.scale;
        let fraction = self.element.range_input_fraction();
        let thumb_left = content.x0 + ((content.width() - thumb_size).max(0.0) * fraction).round();
        let thumb_top = content.y0 + ((content.height() - thumb_size) / 2.0).round();

        let radius = track.height().min(track.width()) / 2.0;
        let shape = Rect::new(track.x0 + 1.0, track.y0, track.x1 - 1.0, track.y1);
        scene.fill(
            Fill::NonZero,
            self.transform,
            track_fill,
            None,
            &shape.to_rounded_rect(radius),
        );
        let value_end = thumb_left + half_track;
        let value_clip = Rect::new(track.x0, track.y0, value_end, track.y1);
        scene.push_clip_layer(Fill::NonZero, self.transform, &value_clip);
        scene.fill(
            Fill::NonZero,
            self.transform,
            value_fill,
            None,
            &shape.to_rounded_rect(radius),
        );
        scene.pop_layer();
        let border_width = self.scale.floor().max(BORDER_WIDTH);
        let outline = shape.inset(-border_width / 2.0).to_rounded_rect(radius);
        scene.stroke(
            &Stroke::new(border_width),
            self.transform,
            track_border,
            None,
            &outline,
        );

        let thumb = Rect::new(
            thumb_left,
            thumb_top,
            thumb_left + thumb_size,
            thumb_top + thumb_size,
        );
        let thumb = thumb.inset(-THUMB_INSET).to_ellipse();
        scene.fill(Fill::NonZero, self.transform, value_fill, None, &thumb);
    }

    fn draw_progress(&self, scene: &mut impl PaintScene) {
        let rect = self.frame.border_box.round();
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }
        let half_track = rect.height() * SLIDER_TRACK_THICKNESS / SLIDER_THUMB_SIZE / 2.0;
        let center = rect.center().y;
        let track = Rect::new(
            rect.x0,
            rect.y0.max(center - half_track),
            rect.x1,
            rect.y1.min(center + half_track),
        );
        let radius = PROGRESS_RADIUS
            .min(track.width() / 2.0)
            .min(track.height() / 2.0);
        let shape = track.to_rounded_rect(radius);
        scene.fill(Fill::NonZero, self.transform, TRACK_FILL, None, &shape);
        let accent = self.control_colors().accent;
        let position = self.element.progress_position();
        let length = match position {
            Some(position) => (rect.width() * position).trunc(),
            None => (rect.width() / PROGRESS_ACTIVITY_BLOCKS).trunc(),
        };
        let length = if length > 0.0 { length.max(2.0) } else { 0.0 };
        let value = Rect::new(track.x0, track.y0, track.x0 + length, track.y1);
        if position.is_some() {
            scene.push_clip_layer(Fill::NonZero, self.transform, &shape);
            scene.fill(Fill::NonZero, self.transform, accent, None, &value);
            scene.pop_layer();
        } else if length < rect.width() {
            let value_radius = PROGRESS_RADIUS
                .min(value.width() / 2.0)
                .min(value.height() / 2.0);
            let value = value.to_rounded_rect(value_radius);
            scene.fill(Fill::NonZero, self.transform, accent, None, &value);
        }
        let border_width = self.scale.floor().max(BORDER_WIDTH);
        let outline = track.inset(-border_width / 2.0).to_rounded_rect(radius);
        if position.is_some() {
            scene.push_clip_layer(Fill::NonZero, self.transform, &shape);
        }
        scene.stroke(
            &Stroke::new(border_width),
            self.transform,
            TRACK_BORDER,
            None,
            &outline,
        );
        if position.is_some() {
            scene.pop_layer();
        }
    }

    fn draw_meter(&self, scene: &mut impl PaintScene) {
        let content = self.frame.content_box;
        let quarter = content.height() / 4.0;
        let bar = Rect::new(
            content.x0,
            content.y0 + quarter,
            content.x1,
            content.y1 - quarter,
        )
        .round();
        if bar.width() <= 0.0 || bar.height() <= 0.0 {
            return;
        }
        let border = self.scale.floor().max(BORDER_WIDTH);
        let radius = METER_RADIUS * self.scale;
        let fit = (bar.width() / (2.0 * radius))
            .min(bar.height() / (2.0 * radius))
            .min(1.0);
        let outer_radius = radius * fit;
        scene.fill(
            Fill::NonZero,
            self.transform,
            TRACK_FILL,
            None,
            &bar.to_rounded_rect(outer_radius),
        );
        let ring = bar
            .inset(-border / 2.0)
            .to_rounded_rect((outer_radius - border / 2.0).max(0.0));
        scene.stroke(
            &Stroke::new(border),
            self.transform,
            METER_BORDER,
            None,
            &ring,
        );
        let (ratio, region) = self.element.meter_gauge();
        let fill = match region {
            GaugeRegion::Optimum => METER_OPTIMUM,
            GaugeRegion::Suboptimum => METER_SUBOPTIMUM,
            GaugeRegion::EvenLessGood => METER_EVEN_LESS_GOOD,
        };
        let inner = bar.inset(-border);
        if inner.width() > 0.0 && inner.height() > 0.0 {
            let inner_shape = inner.to_rounded_rect((outer_radius - border).max(0.0));
            let value_end = (inner.x0 + inner.width() * ratio).round();
            let value = Rect::new(inner.x0, inner.y0, value_end, inner.y1);
            scene.push_clip_layer(Fill::NonZero, self.transform, &inner_shape);
            scene.fill(Fill::NonZero, self.transform, fill, None, &value);
            scene.pop_layer();
        }
    }

    pub(super) fn draw_select_arrow(&self, scene: &mut impl PaintScene) {
        let list = self
            .node
            .element_data()
            .and_then(pageframe_dom::select_list_rows)
            .is_some();
        if self.node.local_name() != "select"
            || list
            || self.style.get_box().appearance == Appearance::None
        {
            return;
        }
        let (transform, rect) = device_space(self.transform, self.frame.border_box);
        let borders = self.frame.border_width;
        let arrow_box = ARROW_BOX * self.scale;
        let size = ARROW_SIZE * self.scale;
        let width = size.trunc();
        let height = (size / 2.0).trunc();
        let left = if self.is_rtl() {
            (rect.x0 + borders.x0.round() + (arrow_box - size) / 2.0).trunc()
        } else {
            (rect.x1 - borders.x1.round() - (arrow_box + size) / 2.0).trunc()
        };
        let top = rect.y0 + (rect.height() / 2.0).floor() - (height / 2.0).floor();
        let bounds = Rect::new(left, top, left + width, top + height);
        if bounds.intersect(rect) != bounds {
            return;
        }
        let mut arrow = BezPath::new();
        arrow.move_to((bounds.x0, bounds.y0));
        arrow.line_to((bounds.x0 + width / 2.0, bounds.y1));
        arrow.line_to((bounds.x1, bounds.y0));
        let color = self.style.clone_color().as_srgb_color();
        let stroke = Stroke::new(ARROW_STROKE)
            .with_caps(Cap::Butt)
            .with_join(Join::Miter);
        scene.stroke(&stroke, transform, color, None, &arrow);
    }

    fn control_colors(&self) -> ControlColors {
        if self.node.is_disabled() {
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

fn simple_color(value: &str) -> Option<Color> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |start: usize| u8::from_str_radix(&hex[start..start + 2], 16).ok();
    Some(Color::from_rgba8(
        channel(0)?,
        channel(2)?,
        channel(4)?,
        0xff,
    ))
}
