use super::{AxisTiling, BoxModelBox, compute_space_count_and_stride, extend, tiled_area};
use crate::color::border_side_color;
use crate::render::ElementCx;
use kurbo::Rect;
use pageframe_dom::CopyStyleGetters as _;
use peniko::Extend;
use style::values::computed::{BorderStyle, Image};
use style::values::specified::background::BackgroundRepeatKeyword;
use style::values::specified::box_::{Display, DisplayInside};

const TILE_EPSILON: f64 = 1e-3;

pub(super) struct RasterAxis {
    pub(super) tiling: AxisTiling,
    pub(super) ratio: f64,
    pub(super) extend: Extend,
}

pub(super) struct AxisSpan {
    pub(super) start: f64,
    pub(super) len: f64,
}

pub(super) struct RasterAxisInput {
    pub(super) repeat: BackgroundRepeatKeyword,
    pub(super) origin: AxisSpan,
    pub(super) clip: AxisSpan,
    pub(super) dest: AxisSpan,
    pub(super) bg_pos: f64,
    pub(super) tile_len: f64,
    pub(super) image_len: f64,
    pub(super) device_offset: f64,
}

pub(super) fn raster_axis(input: RasterAxisInput) -> RasterAxis {
    use BackgroundRepeatKeyword::*;

    let RasterAxisInput {
        repeat,
        origin,
        clip,
        dest,
        bg_pos,
        tile_len,
        image_len,
        device_offset,
    } = input;
    let ratio = tile_len / image_len;
    match repeat {
        Repeat | Round => {
            let (area_start, area_len) =
                tiled_area((origin.start, origin.len), (clip.start, clip.len));
            let extend_len = extend((origin.start - area_start) + bg_pos, tile_len);
            let phase = origin.start + bg_pos;
            let first = phase + ((dest.start - phase) / tile_len).floor() * tile_len;
            if dest.start + dest.len <= first + tile_len + TILE_EPSILON {
                return RasterAxis {
                    tiling: AxisTiling {
                        translate: first,
                        rect_len: image_len,
                        count: 1,
                        stride: 0.0,
                    },
                    ratio,
                    extend: Extend::Pad,
                };
            }
            RasterAxis {
                tiling: AxisTiling {
                    translate: area_start - extend_len,
                    rect_len: (area_len + extend_len) / ratio,
                    count: 1,
                    stride: 0.0,
                },
                ratio,
                extend: Extend::Repeat,
            }
        }
        Space => {
            let (count, stride) = compute_space_count_and_stride(origin.len, tile_len);
            RasterAxis {
                tiling: AxisTiling {
                    translate: origin.start + if count == 1 { bg_pos } else { 0.0 },
                    rect_len: image_len,
                    count,
                    stride,
                },
                ratio,
                extend: Extend::Pad,
            }
        }
        NoRepeat => {
            let start = origin.start + bg_pos;
            let (start, len) = if start > dest.start {
                (
                    layout_round(start + device_offset) - device_offset,
                    layout_round(tile_len),
                )
            } else {
                (start, tile_len)
            };
            RasterAxis {
                tiling: AxisTiling {
                    translate: start,
                    rect_len: image_len,
                    count: 1,
                    stride: 0.0,
                },
                ratio: len / image_len,
                extend: Extend::Pad,
            }
        }
    }
}

fn layout_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

impl ElementCx<'_, '_> {
    pub(super) fn background_dest_rect(&self, clip: BoxModelBox, clip_rect: Rect) -> Rect {
        if clip != BoxModelBox::BorderBox || self.disallows_border_derived_adjustment() {
            return clip_rect;
        }
        let border = self.style.get_border();
        let width = self.frame.border_width;
        let obscures = |line: BorderStyle, color, width: f64| {
            let style = if line == BorderStyle::Double && width < 3.0 {
                BorderStyle::Solid
            } else {
                line
            };
            width > 0.0
                && !matches!(
                    style,
                    BorderStyle::None
                        | BorderStyle::Hidden
                        | BorderStyle::Dotted
                        | BorderStyle::Dashed
                        | BorderStyle::Double
                )
                && border_side_color(&self.style, color, line).components[3] >= 1.0
        };
        let inset = |line, color, width: f64| {
            if obscures(line, color, width) {
                width
            } else {
                0.0
            }
        };
        Rect::new(
            clip_rect.x0
                + inset(
                    border.border_left_style,
                    &border.border_left_color,
                    width.x0,
                ),
            clip_rect.y0 + inset(border.border_top_style, &border.border_top_color, width.y0),
            clip_rect.x1
                - inset(
                    border.border_right_style,
                    &border.border_right_color,
                    width.x1,
                ),
            clip_rect.y1
                - inset(
                    border.border_bottom_style,
                    &border.border_bottom_color,
                    width.y1,
                ),
        )
    }

    fn disallows_border_derived_adjustment(&self) -> bool {
        let display = self.style.clone_display();
        let paints_view = [self.context.root_element_id, self.context.canvas_source]
            .contains(&Some(self.node.id));
        paints_view
            || display == Display::Inline
            || display.inside() == DisplayInside::TableCell
            || self.has_collapsed_borders()
            || !matches!(self.style.get_border().border_image_source, Image::None)
    }
}
