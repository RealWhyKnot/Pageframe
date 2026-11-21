use super::ElementCx;
use super::pixels::{Pixels, Span, device_space, fill_pixels};
use super::ring::ring_path;
use crate::color::{Color, ToColorColor as _};
use anyrender::PaintScene;
use kurbo::{Affine, Insets, Rect, RoundedRect, RoundedRectRadii, Shape as _, Vec2};
use pageframe_dom::CopyStyleGetters;
use pageframe_dom::local_name;
use pageframe_dom::node::SpecialElementData;
use peniko::Fill;
use style::computed_value_flags::ComputedValueFlags;
use style::values::computed::{Appearance, OutlineStyle};

const BORDER: Color = Color::from_rgba8(0x76, 0x76, 0x76, 0xff);
const DISABLED_BORDER: Color = Color::from_rgba8(0x76, 0x76, 0x76, 79);
const DISABLED_BACKGROUND: Color = Color::from_rgba8(0xf8, 0xf8, 0xf8, 0xff);
const BUTTON_FACE: Color = Color::from_rgba8(0xef, 0xef, 0xef, 0xff);
const DISABLED_BUTTON_FACE: Color = Color::from_rgba8(216, 216, 216, 112);
const DISABLED_BUTTON_BORDER: Color = Color::from_rgba8(153, 153, 153, 117);
const FOCUS_RING_OUTER: Color = Color::WHITE;
const BORDER_WIDTH: f64 = 1.0;
const BORDER_RADIUS: f64 = 2.0;
const FOCUS_RING_WIDTH: f64 = 3.0;
const MAX_DEVICE_ROWS: f64 = 4096.0;

#[derive(Clone, Copy)]
enum NativeFrame {
    TextField,
    Button,
    Dropdown,
}

impl ElementCx<'_, '_> {
    fn native_frame(&self) -> Option<NativeFrame> {
        if self.style.get_box().appearance == Appearance::None
            || self
                .style
                .flags
                .contains(ComputedValueFlags::HAS_AUTHOR_OR_USER_SPECIFIED_BORDER_BACKGROUND)
        {
            return None;
        }
        if self.text_input.is_some()
            || matches!(self.element.special_data, SpecialElementData::DateField(_))
        {
            return Some(NativeFrame::TextField);
        }
        if self.element.name.local == local_name!("button") || self.element.is_button_input() {
            return Some(NativeFrame::Button);
        }
        if self.element.name.local == local_name!("select")
            && pageframe_dom::select_list_rows(self.element).is_none()
        {
            return Some(NativeFrame::Dropdown);
        }
        None
    }

    pub(super) fn has_native_frame(&self) -> bool {
        self.native_frame().is_some()
    }

    pub(super) fn draw_native_frame(&self, scene: &mut impl PaintScene) {
        let Some(kind) = self.native_frame() else {
            return;
        };
        let width = zoomed_width(BORDER_WIDTH, self.scale);
        let radius = native_radius(self.scale);
        let (background, border) = match (kind, self.node.is_disabled()) {
            (NativeFrame::TextField, false) => (Color::WHITE, BORDER),
            (NativeFrame::TextField, true) => (DISABLED_BACKGROUND, DISABLED_BORDER),
            (NativeFrame::Button, false) => (BUTTON_FACE, BORDER),
            (NativeFrame::Button, true) => (DISABLED_BUTTON_FACE, DISABLED_BUTTON_BORDER),
            (NativeFrame::Dropdown, _) => {
                let current = self.style.clone_color();
                (
                    self.style
                        .get_background()
                        .background_color
                        .resolve_to_absolute(&current)
                        .as_srgb_color(),
                    self.style
                        .get_border()
                        .border_top_color
                        .resolve_to_absolute(&current)
                        .as_srgb_color(),
                )
            }
        };

        let half = width / 2.0;
        if matches!(kind, NativeFrame::Button) {
            let (transform, rect) = device_space(self.transform, self.frame.border_box);
            let outer = RoundedRect::from_rect(rect, radius + half);
            let inner = RoundedRect::from_rect(rect.inset(-width), (radius - half).max(0.0));
            scene.fill(Fill::NonZero, transform, background, None, &inner);
            let mut ring = outer.to_path(0.1);
            ring.extend(inner.to_path(0.1));
            scene.fill(Fill::EvenOdd, transform, border, None, &ring);
            return;
        }

        let (transform, rect, axes) = frame_space(self.transform, self.frame.border_box);
        let width = axes * width;
        let half = width / 2.0;
        let radius = axes * radius;
        let centre = rect.inset(-Insets::uniform_xy(half.x, half.y));
        let inner = rect.inset(-Insets::uniform_xy(width.x, width.y));
        let outer_radius = radius + half;
        let inner_radius = Vec2::new((radius.x - half.x).max(0.0), (radius.y - half.y).max(0.0));
        let mut fill = Pixels::new();
        let mut stroke = Pixels::new();
        let mut row = rect.y0.floor();
        while row < rect.y1.ceil() {
            let y = row + 0.5;
            if let Some((start, end)) = rrect_columns(centre, radius, y) {
                fill.push(Span { row, start, end });
            }
            if let Some((start, end)) = rrect_columns(rect, outer_radius, y) {
                match rrect_columns(inner, inner_radius, y) {
                    Some((hole_start, hole_end)) => {
                        push_span(&mut stroke, row, start, hole_start.max(start));
                        push_span(&mut stroke, row, hole_end.min(end), end);
                    }
                    None => push_span(&mut stroke, row, start, end),
                }
            }
            row += 1.0;
        }
        fill_pixels(scene, transform, &fill, background);
        fill_pixels(scene, transform, &stroke, border);
    }

    pub(super) fn draw_focus_ring(&self, scene: &mut impl PaintScene) {
        let outline = self.style.get_outline();
        if !matches!(outline.outline_style, OutlineStyle::Auto) {
            return;
        }
        let color = outline
            .outline_color
            .resolve_to_absolute(&self.style.clone_color())
            .as_srgb_color();

        let stroke_width = if self.scale >= 1.0 {
            zoomed_width(FOCUS_RING_WIDTH, self.scale)
        } else {
            FOCUS_RING_WIDTH
        };
        let outer_width = stroke_width / 3.0 * 2.0;
        let inner_width = stroke_width / 3.0;

        let outline_offset = (outline.outline_offset.to_f64_px() * self.scale).round();
        let max_inside = zoomed_width(BORDER_WIDTH, self.scale);
        let borders = self.frame.border_width;
        let min_border = borders.x0.min(borders.y0).min(borders.x1).min(borders.y1);
        let offset = if min_border >= max_inside {
            outline_offset - max_inside
        } else {
            outline_offset
        };

        let radii = self.focus_ring_radii(stroke_width, outline_offset);
        let (transform, rect) = device_space(self.transform, self.frame.border_box);
        let rings = [
            (offset + inner_width.ceil(), FOCUS_RING_OUTER),
            (offset, color),
        ];
        for (ring_offset, ring_color) in rings {
            let ring = ring_path(rect, radii, ring_offset, outer_width);
            scene.fill(Fill::EvenOdd, transform, ring_color, None, &ring);
        }
    }

    fn focus_ring_radii(&self, stroke_width: f64, outline_offset: f64) -> RoundedRectRadii {
        let radii = self.frame.border_radii;
        let corners = [
            radii.top_left,
            radii.top_right,
            radii.bottom_right,
            radii.bottom_left,
        ];
        if corners
            .iter()
            .any(|corner| corner.x > 0.0 && corner.y > 0.0)
        {
            let grow = |radius: f64| {
                if radius > 0.0 {
                    (radius + outline_offset).max(stroke_width)
                } else {
                    stroke_width
                }
            };
            return RoundedRectRadii::new(
                grow(radii.top_left.x),
                grow(radii.top_right.x),
                grow(radii.bottom_right.x),
                grow(radii.bottom_left.x),
            );
        }
        if self.has_native_frame() {
            return RoundedRectRadii::from_single_radius(native_radius(self.scale));
        }
        RoundedRectRadii::from_single_radius(stroke_width)
    }
}

fn zoomed_width(width: f64, scale: f64) -> f64 {
    (width * scale).floor().max(1.0)
}

fn native_radius(scale: f64) -> f64 {
    if scale > 1.0 {
        (BORDER_RADIUS * scale).floor().max(1.0)
    } else {
        BORDER_RADIUS
    }
}

fn frame_space(transform: Affine, rect: Rect) -> (Affine, Rect, Vec2) {
    let [a, b, c, d, e, f] = transform.as_coeffs();
    let device = Rect::new(
        rect.x0 * a + e,
        rect.y0 * d + f,
        rect.x1 * a + e,
        rect.y1 * d + f,
    );
    let is_axis_scale = b == 0.0 && c == 0.0 && a > 0.0 && d > 0.0;
    if !is_axis_scale || !device.is_finite() || device.height() > MAX_DEVICE_ROWS {
        let (transform, rect) = device_space(transform, rect);
        return (transform, rect, Vec2::new(1.0, 1.0));
    }
    let (_, snapped) = device_space(Affine::IDENTITY, device);
    (Affine::IDENTITY, snapped, Vec2::new(a, d))
}

fn rrect_columns(rect: Rect, radius: Vec2, y: f64) -> Option<(f64, f64)> {
    if rect.width() <= 0.0 || rect.height() <= 0.0 || y < rect.y0 || y >= rect.y1 {
        return None;
    }
    let fit = (rect.width() / (2.0 * radius.x))
        .min(rect.height() / (2.0 * radius.y))
        .min(1.0);
    let rx = radius.x * fit;
    let ry = radius.y * fit;
    let dy = if y < rect.y0 + ry {
        rect.y0 + ry - y
    } else if y > rect.y1 - ry {
        y - (rect.y1 - ry)
    } else {
        0.0
    };
    let dx = if ry > 0.0 {
        rx - rx * (1.0 - (dy / ry).powi(2)).max(0.0).sqrt()
    } else {
        0.0
    };
    let start = (rect.x0 + dx - 0.5).ceil();
    let end = (rect.x1 - dx - 0.5).ceil();
    (end > start).then_some((start, end))
}

fn push_span(out: &mut Pixels, row: f64, start: f64, end: f64) {
    if end > start {
        out.push(Span { row, start, end });
    }
}

pub(super) fn caret_rect(
    layout: &parley::Layout<pageframe_dom::node::TextBrush>,
    cursor: &parley::BoundingBox,
    scale: f64,
    origin: kurbo::Vec2,
    centred: bool,
) -> Rect {
    let middle = (cursor.y0 + cursor.y1) / 2.0;
    let (top, bottom) = layout
        .lines()
        .map(|line| *line.metrics())
        .find(|metrics| {
            f64::from(metrics.block_min_coord) <= middle
                && middle <= f64::from(metrics.block_max_coord)
        })
        .map_or((cursor.y0, cursor.y1), |metrics| {
            let ascent = (metrics.baseline - metrics.content_block_min_coord).round();
            let descent = (metrics.content_block_max_coord - metrics.baseline).round();
            let height = ascent + descent;
            let leading = metrics.block_max_coord - metrics.block_min_coord - height;
            let half = leading / 2.0;
            let top = metrics.block_min_coord + if centred { half } else { half.floor() };
            (f64::from(top), f64::from(top + height))
        });
    let snap = |value: f64, offset: f64| (value + offset).round() - offset;
    Rect::new(
        snap(cursor.x0, origin.x),
        snap(top, origin.y),
        snap(cursor.x0 + scale, origin.x),
        snap(bottom, origin.y),
    )
}
