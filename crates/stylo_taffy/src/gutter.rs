use crate::CopyStyleGetters;
use crate::convert::layout_unit_px;
use style::logical_geometry::PhysicalSide;
use style::properties::ComputedValues;
use style::properties::generated::longhands::box_sizing::computed_value::T as BoxSizing;
use style::values::specified::box_::{Overflow, ScrollbarGutter};
use style::values::specified::ui::ScrollbarWidth;
use taffy::CompactLength;

pub fn width(style: &ComputedValues) -> f32 {
    if !style
        .clone_scrollbar_gutter()
        .contains(ScrollbarGutter::STABLE)
    {
        return 0.0;
    }
    let scrolls = |overflow: Overflow| {
        matches!(
            overflow,
            Overflow::Hidden | Overflow::Scroll | Overflow::Auto
        )
    };
    if !scrolls(style.clone_overflow_x()) && !scrolls(style.clone_overflow_y()) {
        return 0.0;
    }
    match style.clone_scrollbar_width() {
        ScrollbarWidth::Auto => 15.0,
        ScrollbarWidth::Thin => 10.0,
        ScrollbarWidth::None => 0.0,
    }
}

pub fn both_edges(style: &ComputedValues) -> bool {
    style
        .clone_scrollbar_gutter()
        .contains(ScrollbarGutter::BOTH_EDGES)
}

fn taffy_reserves_end(style: &ComputedValues) -> bool {
    matches!(
        style.writing_mode.inline_end_physical_side(),
        PhysicalSide::Right | PhysicalSide::Bottom
    )
}

pub fn taffy_width(style: &ComputedValues) -> f32 {
    if taffy_reserves_end(style) {
        width(style)
    } else {
        0.0
    }
}

pub fn total(style: &ComputedValues) -> f32 {
    let gutter = width(style);
    if both_edges(style) {
        gutter * 2.0
    } else {
        gutter
    }
}

fn padding_px(style: &ComputedValues) -> taffy::Rect<f32> {
    let mut extra = taffy::Rect {
        left: 0.0,
        right: 0.0,
        top: 0.0,
        bottom: 0.0,
    };
    let gutter = width(style);
    if gutter <= 0.0 {
        return extra;
    }
    let mut add = |side: PhysicalSide| match side {
        PhysicalSide::Left => extra.left += gutter,
        PhysicalSide::Right => extra.right += gutter,
        PhysicalSide::Top => extra.top += gutter,
        PhysicalSide::Bottom => extra.bottom += gutter,
    };
    let wm = style.writing_mode;
    let (end, start) = if wm.is_vertical() {
        (PhysicalSide::Bottom, PhysicalSide::Top)
    } else {
        (
            wm.inline_end_physical_side(),
            wm.inline_start_physical_side(),
        )
    };
    if !taffy_reserves_end(style) {
        add(end);
    }
    if both_edges(style) {
        add(start);
    }
    extra
}

pub fn overflow(style: &ComputedValues) -> taffy::Point<taffy::Overflow> {
    let settle = |overflow: Overflow| match crate::convert::overflow(overflow) {
        taffy::Overflow::Scroll => taffy::Overflow::Hidden,
        other => other,
    };
    let mut overflow = taffy::Point {
        x: settle(style.clone_overflow_x()),
        y: settle(style.clone_overflow_y()),
    };
    if taffy_width(style) > 0.0 {
        if style.writing_mode.is_vertical() {
            overflow.x = taffy::Overflow::Scroll;
        } else {
            overflow.y = taffy::Overflow::Scroll;
        }
    }
    overflow
}

pub fn pad(
    padding: taffy::Rect<taffy::LengthPercentage>,
    style: &ComputedValues,
    scale: f32,
) -> taffy::Rect<taffy::LengthPercentage> {
    let extra = padding_px(style);
    let add = |value: taffy::LengthPercentage, px: f32| {
        let raw = value.into_raw();
        if px <= 0.0 || raw.tag() != CompactLength::LENGTH_TAG {
            return value;
        }
        taffy::LengthPercentage::length(raw.value() + layout_unit_px(px, scale))
    };
    taffy::Rect {
        left: add(padding.left, extra.left),
        right: add(padding.right, extra.right),
        top: add(padding.top, extra.top),
        bottom: add(padding.bottom, extra.bottom),
    }
}

fn content_box_shrink(style: &ComputedValues, scale: f32) -> taffy::Size<f32> {
    if style.get_position().box_sizing != BoxSizing::ContentBox {
        return taffy::Size {
            width: 0.0,
            height: 0.0,
        };
    }
    let extra = padding_px(style);
    taffy::Size {
        width: layout_unit_px(extra.left + extra.right, scale),
        height: layout_unit_px(extra.top + extra.bottom, scale),
    }
}

fn shrink<T>(value: T, raw: CompactLength, by: f32, length: fn(f32) -> T) -> T {
    if by <= 0.0 || raw.tag() != CompactLength::LENGTH_TAG {
        return value;
    }
    length((raw.value() - by).max(0.0))
}

pub fn shrink_size(
    size: taffy::Size<taffy::Dimension>,
    style: &ComputedValues,
    scale: f32,
) -> taffy::Size<taffy::Dimension> {
    let by = content_box_shrink(style, scale);
    taffy::Size {
        width: shrink(
            size.width,
            size.width.into_raw(),
            by.width,
            taffy::Dimension::length,
        ),
        height: shrink(
            size.height,
            size.height.into_raw(),
            by.height,
            taffy::Dimension::length,
        ),
    }
}

pub fn shrink_bounds(
    size: taffy::Size<taffy::LengthPercentageAuto>,
    style: &ComputedValues,
    scale: f32,
) -> taffy::Size<taffy::LengthPercentageAuto> {
    let by = content_box_shrink(style, scale);
    taffy::Size {
        width: shrink(
            size.width,
            size.width.into_raw(),
            by.width,
            taffy::LengthPercentageAuto::length,
        ),
        height: shrink(
            size.height,
            size.height.into_raw(),
            by.height,
            taffy::LengthPercentageAuto::length,
        ),
    }
}
