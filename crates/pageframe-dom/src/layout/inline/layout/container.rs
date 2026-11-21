use super::LayoutPassState;
use crate::layout::resolve_calc_value;
use pageframe_traits::node_id::NodeId;
use taffy::{
    BoxSizing, CoreStyle as _, LayoutInput, MaybeMath as _, MaybeResolve as _, Overflow, Point,
    Rect, ResolveOrZero as _, Size, SizingMode,
};

pub(super) struct ContainerStyle {
    pub(super) padding: Rect<f32>,
    pub(super) border: Rect<f32>,
    pub(super) container_pb: Rect<f32>,
    pub(super) scrollbar_gutter: Point<f32>,
    pub(super) content_box_inset: Rect<f32>,
    pub(super) has_styles_preventing_being_collapsed_through: bool,
    pub(super) node_size: Size<Option<f32>>,
    pub(super) node_min_size: Size<Option<f32>>,
    pub(super) node_max_size: Size<Option<f32>>,
    pub(super) aspect_ratio: Option<f32>,
    pub(super) suppresses_baseline: bool,
    pub(super) is_scroll_container: bool,
}

impl LayoutPassState<'_> {
    pub(super) fn container_style(&self, node_id: NodeId, inputs: LayoutInput) -> ContainerStyle {
        let LayoutInput {
            known_dimensions,
            parent_size,
            sizing_mode,
            ..
        } = inputs;
        let mut style = self.nodes[node_id].layout_style();
        style.flags.set(
            stylo_taffy::StyleFlags::IGNORE_ASPECT_RATIO,
            self.ratio_free == Some(node_id),
        );

        let padding = style
            .padding()
            .resolve_or_zero(parent_size.width, resolve_calc_value);
        let border = style
            .border()
            .resolve_or_zero(parent_size.width, resolve_calc_value);
        let container_pb = padding + border;
        let pb_sum = container_pb.sum_axes();
        let box_sizing_adjustment = if style.box_sizing() == BoxSizing::ContentBox {
            pb_sum
        } else {
            Size::ZERO
        };

        let scrollbar_gutter = style.overflow().transpose().map(|overflow| match overflow {
            Overflow::Scroll => style.scrollbar_width(),
            _ => 0.0,
        });
        let mut content_box_inset = container_pb;
        content_box_inset.right += scrollbar_gutter.x;
        content_box_inset.bottom += scrollbar_gutter.y;

        let has_styles_preventing_being_collapsed_through = !style.is_block()
            || style.overflow().x.is_scroll_container()
            || style.overflow().y.is_scroll_container()
            || style.position().is_out_of_flow()
            || padding.top > 0.0
            || padding.bottom > 0.0
            || border.top > 0.0
            || border.bottom > 0.0;

        let (node_size, node_min_size, node_max_size, aspect_ratio) = match sizing_mode {
            SizingMode::ContentSize => {
                let node_size = known_dimensions;
                let node_min_size = Size::NONE;
                let node_max_size = Size::NONE;
                (node_size, node_min_size, node_max_size, None)
            }
            SizingMode::InherentSize => {
                let aspect_ratio = style.aspect_ratio();
                let style_size = style
                    .size()
                    .maybe_resolve(parent_size, resolve_calc_value)
                    .maybe_apply_aspect_ratio(aspect_ratio)
                    .maybe_add(box_sizing_adjustment);
                let style_min_size = style
                    .min_size()
                    .maybe_resolve(parent_size, resolve_calc_value)
                    .maybe_apply_aspect_ratio(aspect_ratio)
                    .maybe_add(box_sizing_adjustment);
                let style_max_size = style
                    .max_size()
                    .maybe_resolve(parent_size, resolve_calc_value)
                    .maybe_add(box_sizing_adjustment);

                let node_size =
                    known_dimensions.or(style_size.maybe_clamp(style_min_size, style_max_size));
                (node_size, style_min_size, style_max_size, aspect_ratio)
            }
        };

        let suppresses_baseline = style.contain().suppresses_baseline();
        let overflow = style.overflow();
        let is_scroll_container =
            overflow.x.is_scroll_container() || overflow.y.is_scroll_container();
        ContainerStyle {
            padding,
            border,
            container_pb,
            scrollbar_gutter,
            content_box_inset,
            has_styles_preventing_being_collapsed_through,
            node_size,
            node_min_size,
            node_max_size,
            aspect_ratio,
            suppresses_baseline,
            is_scroll_container,
        }
    }
}
