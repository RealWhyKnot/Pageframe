use blitz_traits::node_id::NodeId;
use kurbo::Rect as KurboRect;
use style::values::computed::{Overflow, Resize};
use stylo_taffy::CopyStyleGetters;

use super::{Node, ScrollbarRef};

pub const RESIZER_SIZE: f64 = 15.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowControl {
    Scrollbar(ScrollbarRef),
    Resizer(NodeId),
}

impl OverflowControl {
    pub fn scrollbar(self) -> Option<ScrollbarRef> {
        match self {
            Self::Scrollbar(scrollbar) => Some(scrollbar),
            Self::Resizer(_) => None,
        }
    }

    pub fn resizer(self) -> Option<NodeId> {
        match self {
            Self::Resizer(node_id) => Some(node_id),
            Self::Scrollbar(_) => None,
        }
    }
}

pub fn resizer_size(scale: f64) -> f64 {
    (RESIZER_SIZE * scale).round()
}

impl Node {
    pub fn resize(&self) -> Resize {
        let Some(style) = self.primary_styles() else {
            return Resize::None;
        };
        let resize = style.clone_resize();
        if resize == Resize::None {
            return Resize::None;
        }
        let is_scroll_container = |overflow| {
            matches!(
                overflow,
                Overflow::Hidden | Overflow::Scroll | Overflow::Auto
            )
        };
        if is_scroll_container(style.clone_overflow_x())
            || is_scroll_container(style.clone_overflow_y())
        {
            resize
        } else {
            Resize::None
        }
    }

    pub fn resizer_on_left(&self) -> bool {
        self.primary_styles().is_some_and(|style| {
            style.clone_direction() == style::computed_values::direction::T::Rtl
        })
    }

    pub fn resizer_corner(&self, scale: f64) -> Option<KurboRect> {
        if self.resize() == Resize::None {
            return None;
        }
        let layout = self.final_layout();
        let size = resizer_size(scale);
        let device = |v: f32| (f64::from(v) * scale).round();
        let x0 = if self.resizer_on_left() {
            device(layout.border.left)
        } else {
            device(layout.size.width) - size - device(layout.border.right)
        };
        let y0 = device(layout.size.height) - size - device(layout.border.bottom);
        Some(KurboRect::new(x0, y0, x0 + size, y0 + size))
    }

    pub(crate) fn resizer_at_local(&self, x: f64, y: f64, scale: f64) -> Option<NodeId> {
        let corner = self.resizer_corner(scale)?;
        corner
            .contains(kurbo::Point::new((x * scale).floor(), (y * scale).floor()))
            .then_some(self.id)
    }
}
