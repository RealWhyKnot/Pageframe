use super::LayoutPassState;
use style::values::computed::CSSPixelLength;
#[cfg(not(feature = "writing-mode"))]
use taffy::TraversePartialTree;
use taffy::{Layout, NodeId, Point, Rect, RoundTree, ScrollableOverflow};

const LAYOUT_UNITS: f32 = 64.0;
const UNIT_SLACK: f32 = 1.0 / 32.0;
const NOISE_ULPS: f32 = 4.0;

#[derive(Clone, Copy)]
pub(crate) struct DeviceSnap {
    scale: f32,
}

impl DeviceSnap {
    pub(crate) fn new(scale: f32) -> Self {
        Self {
            scale: if scale > 0.0 { scale } else { 1.0 },
        }
    }

    pub(crate) fn round(self, value: f32) -> f32 {
        self.device(value) / self.scale
    }

    pub(crate) fn shift(self, start: f32, offset: f32) -> f32 {
        self.between(start, start + offset)
    }

    fn between(self, from: f32, to: f32) -> f32 {
        (self.device(to) - self.device(from)) / self.scale
    }

    fn device(self, value: f32) -> f32 {
        let units = value * self.scale * LAYOUT_UNITS;
        let slack = (units.abs() * NOISE_ULPS * f32::EPSILON).max(UNIT_SLACK);
        let truncated = (units.abs() + slack).floor().copysign(units);
        ((truncated + LAYOUT_UNITS / 2.0) / LAYOUT_UNITS).floor()
    }
}

impl DeviceSnap {
    fn inset_from_end(self, start: f32, size: f32, inset: f32) -> f32 {
        self.between(start + size - inset, start + size)
    }

    fn edges(self, x: f32, y: f32, size: taffy::Size<f32>, rect: Rect<f32>) -> Rect<f32> {
        Rect {
            left: self.shift(x, rect.left),
            right: self.inset_from_end(x, size.width, rect.right),
            top: self.shift(y, rect.top),
            bottom: self.inset_from_end(y, size.height, rect.bottom),
        }
    }
}

#[cfg(not(feature = "writing-mode"))]
pub(crate) fn round_layout(tree: &mut LayoutPassState<'_>, root: NodeId, snap: DeviceSnap) {
    round_inner(tree, root, snap, 0.0, 0.0);
}

#[cfg(not(feature = "writing-mode"))]
fn round_inner(
    tree: &mut LayoutPassState<'_>,
    node_id: NodeId,
    snap: DeviceSnap,
    parent_x: f32,
    parent_y: f32,
) {
    let unrounded = tree.get_unrounded_layout(node_id);
    let position = snap_layout(
        tree,
        node_id,
        unrounded,
        Point {
            x: parent_x,
            y: parent_y,
        },
        snap,
    );
    for index in 0..tree.child_count(node_id) {
        let child = tree.get_child_id(node_id, index);
        if !tree.is_out_of_flow(child) {
            round_inner(tree, child, snap, position.x, position.y);
        }
    }
    for index in 0..tree.hoisted_child_count(node_id) {
        let child = tree.get_hoisted_child_id(node_id, index);
        round_inner(tree, child, snap, position.x, position.y);
    }
}

pub(crate) fn snap_layout(
    tree: &mut LayoutPassState<'_>,
    node_id: NodeId,
    unrounded: Layout,
    parent: Point<f32>,
    snap: DeviceSnap,
) -> Point<f32> {
    let size = unrounded.size;
    let x = parent.x + unrounded.location.x;
    let y = parent.y + unrounded.location.y;
    let (x, y) = if starts_a_snap_root(tree.node_from_id(node_id), size) {
        (snap.round(x), snap.round(y))
    } else {
        (x, y)
    };

    let mut layout = unrounded;
    layout.location.x = snap.between(parent.x, x);
    layout.location.y = snap.between(parent.y, y);
    layout.size.width = snap.shift(x, size.width);
    layout.size.height = snap.shift(y, size.height);
    layout.scrollbar_size = unrounded.scrollbar_size.map(|length| snap.round(length));
    layout.border = snap.edges(x, y, size, unrounded.border);
    layout.padding = snap.edges(x, y, size, unrounded.padding);

    let overflow = unrounded.scrollable_overflow_rect.rect();
    let overflow_edge = |offset: f32, edge: f32| {
        let rounded = snap.shift(offset, edge);
        if rounded == 0.0 {
            rounded.copysign(edge)
        } else {
            rounded
        }
    };
    layout.scrollable_overflow_rect = ScrollableOverflow::new(
        Rect {
            left: overflow_edge(x, overflow.left),
            right: overflow_edge(x, overflow.right),
            top: overflow_edge(y, overflow.top),
            bottom: overflow_edge(y, overflow.bottom),
        },
        unrounded.scrollable_overflow_rect.is_reversed(),
    );

    tree.set_final_layout(node_id, &layout);
    tree.node_from_id_mut(node_id).layout_data_mut().unsnapped = crate::node::UnsnappedBox {
        offset: taffy::Point {
            x: x - snap.round(x),
            y: y - snap.round(y),
        },
        size,
        border: unrounded.border,
        padding: unrounded.padding,
    };
    let offset = |start: f32, edge: f32, padding: f32, rounded_edge: f32, rounded_padding: f32| {
        start + edge + padding - (snap.round(start) + rounded_edge + rounded_padding)
    };
    tree.set_content_box_subpixel_offset(
        node_id,
        taffy::Point {
            x: offset(
                x,
                unrounded.border.left,
                unrounded.padding.left,
                layout.border.left,
                layout.padding.left,
            ),
            y: offset(
                y,
                unrounded.border.top,
                unrounded.padding.top,
                layout.border.top,
                layout.padding.top,
            ),
        },
    );
    Point { x, y }
}

fn starts_a_snap_root(node: &crate::Node, size: taffy::Size<f32>) -> bool {
    let Some(styles) = node.primary_styles() else {
        return false;
    };
    let reference_box = euclid::default::Rect::new(
        euclid::default::Point2D::new(CSSPixelLength::new(0.0), CSSPixelLength::new(0.0)),
        euclid::default::Size2D::new(
            CSSPixelLength::new(size.width),
            CSSPixelLength::new(size.height),
        ),
    );
    crate::transform_goes_beyond_a_shift(styles.get_box(), reference_box)
}

#[cfg(test)]
mod tests {
    use super::DeviceSnap;

    #[test]
    fn negative_offsets_truncate_toward_zero_then_round_half_up() {
        let snap = DeviceSnap::new(1.0);
        assert_eq!(snap.round(-7.49), -7.0);
        assert_eq!(snap.round(-7.5), -7.0);
        assert_eq!(snap.round(-7.52), -8.0);
        assert_eq!(snap.round(7.5), 8.0);
        assert_eq!(snap.round(7.49), 7.0);
    }

    #[test]
    fn float_noise_below_a_half_pixel_rounds_up_far_down_the_page() {
        let snap = DeviceSnap::new(1.75);
        for whole in [4.0_f32, 2_340.0, 8_000.0, 16_384.0] {
            let css = (whole + 0.5) / 1.75;
            for noisy in [css, css.next_down(), css.next_down().next_down()] {
                let device = snap.round(noisy) * 1.75;
                assert!((device - whole - 1.0).abs() < 1e-2, "{whole}: {device}");
            }
        }
    }
}
