use super::ElementCx;
use super::pixels::{Pixels, Span, device_space, fill_pixels};
use super::ring::ring_path;
use crate::color::{Color, ToColorColor as _};
use anyrender::PaintScene;
use kurbo::{Rect, RoundedRectRadii};
use pageframe_dom::{CopyStyleGetters, local_name};
use peniko::Fill;
use style::computed_value_flags::ComputedValueFlags;
use style::values::computed::{Appearance, OutlineStyle};

const BORDER: Color = Color::from_rgba8(0x76, 0x76, 0x76, 0xff);
const DISABLED_BORDER: Color = Color::from_rgba8(0x76, 0x76, 0x76, 79);
const DISABLED_BACKGROUND: Color = Color::from_rgba8(0xf8, 0xf8, 0xf8, 0xff);
const FOCUS_RING_OUTER: Color = Color::WHITE;
const BORDER_WIDTH: f64 = 1.0;
const BORDER_RADIUS: f64 = 2.0;
const FOCUS_RING_WIDTH: f64 = 3.0;

impl ElementCx<'_, '_> {
    pub(super) fn has_native_text_field(&self) -> bool {
        self.text_input.is_some()
            && self.style.get_box().appearance != Appearance::None
            && !self
                .style
                .flags
                .contains(ComputedValueFlags::HAS_AUTHOR_OR_USER_SPECIFIED_BORDER_BACKGROUND)
    }

    pub(super) fn draw_native_text_field(&self, scene: &mut impl PaintScene) {
        let (transform, rect) = device_space(self.transform, self.frame.border_box);
        let width = zoomed_width(BORDER_WIDTH, self.scale);
        let radius = native_radius(self.scale);
        let (background, border) = if self.node.attr(local_name!("disabled")).is_some() {
            (DISABLED_BACKGROUND, DISABLED_BORDER)
        } else {
            (Color::WHITE, BORDER)
        };

        let half = width / 2.0;
        let centre = rect.inset(-half);
        let inner = rect.inset(-width);
        let inner_radius = (radius - half).max(0.0);
        let mut fill = Pixels::new();
        let mut stroke = Pixels::new();
        let mut row = rect.y0.floor();
        while row < rect.y1.ceil() {
            let y = row + 0.5;
            if let Some((start, end)) = rrect_columns(centre, radius, y) {
                fill.push(Span { row, start, end });
            }
            if let Some((start, end)) = rrect_columns(rect, radius + half, y) {
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

        let outline_offset = (outline.outline_offset.to_f64_px() * self.scale).trunc();
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
        if self.has_native_text_field() {
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

fn rrect_columns(rect: Rect, radius: f64, y: f64) -> Option<(f64, f64)> {
    if rect.width() <= 0.0 || rect.height() <= 0.0 || y < rect.y0 || y >= rect.y1 {
        return None;
    }
    let radius = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let dy = if y < rect.y0 + radius {
        rect.y0 + radius - y
    } else if y > rect.y1 - radius {
        y - (rect.y1 - radius)
    } else {
        0.0
    };
    let dx = radius - (radius * radius - dy * dy).max(0.0).sqrt();
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
