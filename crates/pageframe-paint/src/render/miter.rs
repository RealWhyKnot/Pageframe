use crate::kurbo_css::{CssBox, Edge, EdgeEnd};
use kurbo::{Affine, BezPath, Point, Rect, Shape as _, Vec2};
use peniko::Color;
use style::values::computed::BorderStyle;

pub(super) type Side = (Edge, BorderStyle, Color);

#[derive(Clone, Copy)]
struct CornerAt {
    left: bool,
    top: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Miter {
    Full,
    Soft,
    Hard,
}

pub(super) fn paint_order(&(edge, style, color): &Side) -> f32 {
    let style_rank = match style {
        BorderStyle::None | BorderStyle::Hidden => 0.0,
        BorderStyle::Dotted | BorderStyle::Dashed => 1.0,
        BorderStyle::Inset
        | BorderStyle::Outset
        | BorderStyle::Groove
        | BorderStyle::Ridge
        | BorderStyle::Double => 2.0,
        BorderStyle::Solid => 3.0,
    };
    let side_rank = match edge {
        Edge::Top => 0.0,
        Edge::Bottom => 1.0,
        Edge::Right => 2.0,
        Edge::Left => 3.0,
    };
    (color.components[3] * 255.0).round() * 16.0 + style_rank * 4.0 + side_rank
}

pub(super) struct SideShape {
    ends: [EdgeEnd; 2],
    hard: [bool; 2],
    offset: Vec2,
}

impl SideShape {
    pub(super) fn new(frame: &CssBox, transform: Affine, sides: &[Side], index: usize) -> Self {
        let (edge, style, color) = sides[index];
        let offset = translation(transform);
        let corners = corner_flags(edge);
        let mut ends = [EdgeEnd::Mitered; 2];
        let mut hard = [false; 2];
        for (end, neighbour) in corner_neighbours(edge).into_iter().enumerate() {
            let overdrawn = sides[index + 1..]
                .iter()
                .any(|&(other, other_style, other_color)| {
                    other == neighbour
                        && fills_border_area(other_style)
                        && other_color.components[3] >= 1.0
                });
            let miter = if overdrawn {
                Miter::Full
            } else {
                match sides[..index].iter().find(|side| side.0 == neighbour) {
                    Some(&(_, other_style, other_color))
                        if other_color == color && !unmatched_at_corner(style, edge, neighbour) =>
                    {
                        if styles_require_miter(style, other_style) {
                            Miter::Hard
                        } else {
                            Miter::Full
                        }
                    }
                    _ => Miter::Soft,
                }
            };
            let pixel_exact = offset.is_some() && is_sharp(frame, corners[end]);
            ends[end] = match miter {
                Miter::Full => EdgeEnd::Full,
                Miter::Hard if pixel_exact => {
                    hard[end] = true;
                    EdgeEnd::Butt
                }
                Miter::Soft | Miter::Hard => EdgeEnd::Mitered,
            };
        }
        Self {
            ends,
            hard,
            offset: offset.unwrap_or(Vec2::ZERO),
        }
    }

    pub(super) fn path(&self, frame: &CssBox, edge: Edge) -> BezPath {
        let mut path = frame.border_edge_shape_with_ends(edge, self.ends);
        for (end, corner) in corner_flags(edge).into_iter().enumerate() {
            if self.hard[end] {
                hard_corner(&mut path, frame, edge, corner, self.offset);
            }
        }
        path
    }
}

fn translation(transform: Affine) -> Option<Vec2> {
    let [a, b, c, d, e, f] = transform.as_coeffs();
    (a == 1.0 && b == 0.0 && c == 0.0 && d == 1.0).then_some(Vec2::new(e, f))
}

fn corner_neighbours(edge: Edge) -> [Edge; 2] {
    match edge {
        Edge::Top => [Edge::Left, Edge::Right],
        Edge::Right => [Edge::Top, Edge::Bottom],
        Edge::Bottom => [Edge::Right, Edge::Left],
        Edge::Left => [Edge::Bottom, Edge::Top],
    }
}

fn corner_flags(edge: Edge) -> [CornerAt; 2] {
    let at = |left, top| CornerAt { left, top };
    match edge {
        Edge::Top => [at(true, true), at(false, true)],
        Edge::Right => [at(false, true), at(false, false)],
        Edge::Bottom => [at(false, false), at(true, false)],
        Edge::Left => [at(true, false), at(true, true)],
    }
}

fn fills_border_area(style: BorderStyle) -> bool {
    !matches!(
        style,
        BorderStyle::Dotted | BorderStyle::Dashed | BorderStyle::Double
    )
}

fn is_beveled(style: BorderStyle) -> bool {
    matches!(
        style,
        BorderStyle::Inset | BorderStyle::Outset | BorderStyle::Groove | BorderStyle::Ridge
    )
}

fn is_dotted_or_dashed(style: BorderStyle) -> bool {
    matches!(style, BorderStyle::Dotted | BorderStyle::Dashed)
}

fn unmatched_at_corner(style: BorderStyle, edge: Edge, neighbour: Edge) -> bool {
    is_beveled(style)
        && matches!(
            (edge, neighbour),
            (Edge::Top, Edge::Right)
                | (Edge::Right, Edge::Top)
                | (Edge::Bottom, Edge::Left)
                | (Edge::Left, Edge::Bottom)
        )
}

fn styles_require_miter(style: BorderStyle, neighbour_style: BorderStyle) -> bool {
    style == BorderStyle::Double
        || matches!(
            neighbour_style,
            BorderStyle::Double | BorderStyle::Groove | BorderStyle::Ridge
        )
        || is_dotted_or_dashed(style) != is_dotted_or_dashed(neighbour_style)
        || style != neighbour_style
}

fn is_sharp(frame: &CssBox, corner: CornerAt) -> bool {
    let radii = &frame.border_radii;
    let radius = match (corner.left, corner.top) {
        (true, true) => radii.top_left,
        (false, true) => radii.top_right,
        (false, false) => radii.bottom_right,
        (true, false) => radii.bottom_left,
    };
    radius.x == 0.0 || radius.y == 0.0
}

fn hard_corner(path: &mut BezPath, frame: &CssBox, edge: Edge, corner: CornerAt, offset: Vec2) {
    let pick = |rect: Rect| {
        Point::new(
            if corner.left { rect.x0 } else { rect.x1 },
            if corner.top { rect.y0 } else { rect.y1 },
        ) + offset
    };
    let outer = pick(frame.border_box);
    let inner = pick(frame.padding_box);
    if outer.y == inner.y || outer.x == inner.x {
        return;
    }
    let x_min = outer.x.min(inner.x).round();
    let x_max = outer.x.max(inner.x).round();
    let take_corner_side = matches!(edge, Edge::Left | Edge::Right);
    let take_low = take_corner_side == corner.left;
    let mut row = outer.y.min(inner.y).round();
    let last = outer.y.max(inner.y).round();
    while row < last {
        let x = outer.x + (row + 0.5 - outer.y) * (inner.x - outer.x) / (inner.y - outer.y);
        let split = (x + 0.5).floor().clamp(x_min, x_max);
        let (start, end) = if take_low {
            (x_min, split)
        } else {
            (split, x_max)
        };
        if end > start {
            let pixels = Rect::new(start, row, end, row + 1.0) - offset;
            path.extend(pixels.path_elements(0.1));
        }
        row += 1.0;
    }
}
