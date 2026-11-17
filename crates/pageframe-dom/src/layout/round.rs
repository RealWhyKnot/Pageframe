#[cfg(not(feature = "writing-mode"))]
use super::LayoutPassState;
#[cfg(not(feature = "writing-mode"))]
use taffy::{NodeId, Rect, RoundTree, ScrollableOverflow, TraversePartialTree};

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
        (value * self.scale).round() / self.scale
    }

    pub(crate) fn shift(self, start: f32, offset: f32) -> f32 {
        (((start + offset) * self.scale).round() - (start * self.scale).round()) / self.scale
    }
}

#[cfg(not(feature = "writing-mode"))]
impl DeviceSnap {
    fn span(self, start: f32, length: f32) -> f32 {
        self.round(start + length) - self.round(start)
    }

    fn inset_from_end(self, start: f32, size: f32, inset: f32) -> f32 {
        self.round(start + size) - self.round(start + size - inset)
    }

    fn edges(self, x: f32, y: f32, size: taffy::Size<f32>, rect: Rect<f32>) -> Rect<f32> {
        Rect {
            left: self.span(x, rect.left),
            right: self.inset_from_end(x, size.width, rect.right),
            top: self.span(y, rect.top),
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
    let x = parent_x + unrounded.location.x;
    let y = parent_y + unrounded.location.y;
    let size = unrounded.size;

    let mut layout = unrounded;
    layout.location.x = snap.round(x) - snap.round(parent_x);
    layout.location.y = snap.round(y) - snap.round(parent_y);
    layout.size.width = snap.span(x, size.width);
    layout.size.height = snap.span(y, size.height);
    layout.scrollbar_size = unrounded.scrollbar_size.map(|length| snap.round(length));
    layout.border = snap.edges(x, y, size, unrounded.border);
    layout.padding = snap.edges(x, y, size, unrounded.padding);

    let overflow = unrounded.scrollable_overflow_rect.rect();
    let overflow_edge = |offset: f32, edge: f32| {
        let rounded = snap.span(offset, edge);
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

    for index in 0..tree.child_count(node_id) {
        let child = tree.get_child_id(node_id, index);
        if !tree.is_out_of_flow(child) {
            round_inner(tree, child, snap, x, y);
        }
    }
    for index in 0..tree.hoisted_child_count(node_id) {
        let child = tree.get_hoisted_child_id(node_id, index);
        round_inner(tree, child, snap, x, y);
    }
}
