use style::values::specified::box_::{Display, DisplayInside, DisplayOutside};
use taffy::{
    AbsoluteAxis, BoxSizing, CoreStyle, LayoutInput, MaybeMath, MaybeResolve, NodeId,
    RequestedAxis, ResolveOrZero, RunMode, Size,
};

use super::round::LAYOUT_UNITS;
use super::{LayoutPassState, resolve_calc_value};
use crate::dom_node_id;

const UNIT_SLACK: f32 = 1.0 / 32.0;

struct Transfer {
    axis: AbsoluteAxis,
    size: f32,
    max: Option<f32>,
    content_floor: bool,
}

fn set<T>(size: &mut Size<T>, axis: AbsoluteAxis, value: T) {
    match axis {
        AbsoluteAxis::Horizontal => size.width = value,
        AbsoluteAxis::Vertical => size.height = value,
    }
}

fn is_ratio_box(display: Display) -> bool {
    let boxed = matches!(
        display.inside(),
        DisplayInside::Flow | DisplayInside::FlowRoot | DisplayInside::Flex | DisplayInside::Grid
    );
    let internal = matches!(
        display.outside(),
        DisplayOutside::TableCaption | DisplayOutside::InternalTable
    );
    boxed && !internal && !display.is_inline_flow()
}

impl LayoutPassState<'_> {
    pub(super) fn apply_aspect_ratio(
        &mut self,
        node_id: NodeId,
        inputs: LayoutInput,
    ) -> LayoutInput {
        if self.ratio_free == Some(dom_node_id(node_id)) {
            return inputs;
        }
        let Some(transfer) = self.ratio_transfer(node_id, &inputs) else {
            return inputs;
        };
        let axis = transfer.axis;
        let size = match transfer.content_floor {
            true => {
                let content = self.measure_without_ratio(node_id, inputs, axis);
                transfer.size.max(content.maybe_min(transfer.max))
            }
            false => transfer.size,
        };
        let mut known_dimensions = inputs.known_dimensions;
        set(&mut known_dimensions, axis, Some(size));
        let mut definite = inputs.known_dimensions_are_definite;
        let known_is_definite = definite.get_abs(axis.other_axis());
        set(&mut definite, axis, known_is_definite);
        LayoutInput {
            known_dimensions,
            known_dimensions_are_definite: definite,
            ..inputs
        }
    }

    fn ratio_transfer(&self, node_id: NodeId, inputs: &LayoutInput) -> Option<Transfer> {
        let (axis, known) = match (
            inputs.known_dimensions.width,
            inputs.known_dimensions.height,
        ) {
            (Some(width), None) => (AbsoluteAxis::Vertical, width),
            (None, Some(height)) => (AbsoluteAxis::Horizontal, height),
            _ => return None,
        };
        let node = &self.nodes[dom_node_id(node_id)];
        node.element_data()?;
        if !is_ratio_box(node.primary_styles()?.get_box().display) {
            return None;
        }
        let style = self.child_layout_style(node);
        if style.is_replaced() || style.is_compressible_replaced() {
            return None;
        }
        let ratio = style.aspect_ratio()?;
        let parent_size = inputs.parent_size;
        let resolve =
            |size: Size<Option<f32>>, adjust: Size<f32>| size.maybe_add(adjust).get_abs(axis);
        let style_size = style.size().maybe_resolve(parent_size, resolve_calc_value);
        if resolve(style_size, Size::ZERO).is_some() {
            return None;
        }
        let padding = style
            .padding()
            .resolve_or_zero(parent_size.width, resolve_calc_value);
        let border = style
            .border()
            .resolve_or_zero(parent_size.width, resolve_calc_value);
        let frame = (padding + border).sum_axes();
        let adjust = match style.box_sizing() {
            BoxSizing::ContentBox => frame,
            BoxSizing::BorderBox => Size::ZERO,
        };
        let inner = (known - adjust.get_abs(axis.other_axis())).max(0.0);
        let ratio_size = match axis {
            AbsoluteAxis::Vertical => inner / ratio,
            AbsoluteAxis::Horizontal => inner * ratio,
        };
        let derived = self.floor_to_layout_unit(ratio_size) + adjust.get_abs(axis);
        let min = resolve(
            style
                .min_size()
                .maybe_resolve(parent_size, resolve_calc_value),
            adjust,
        );
        let max = resolve(
            style
                .max_size()
                .maybe_resolve(parent_size, resolve_calc_value),
            adjust,
        );
        let size = derived
            .maybe_min(max)
            .maybe_max(min)
            .max(frame.get_abs(axis));
        let overflow = style.overflow();
        let content_floor = style.min_size().get_abs(axis).is_auto()
            && !overflow.x.is_scroll_container()
            && !overflow.y.is_scroll_container();
        Some(Transfer {
            axis,
            size,
            max,
            content_floor,
        })
    }

    fn floor_to_layout_unit(&self, value: f32) -> f32 {
        let units = self.doc.viewport.scale() * LAYOUT_UNITS;
        (value * units + UNIT_SLACK).floor() / units
    }

    fn measure_without_ratio(
        &mut self,
        node_id: NodeId,
        inputs: LayoutInput,
        axis: AbsoluteAxis,
    ) -> f32 {
        let measure = LayoutInput {
            run_mode: RunMode::ComputeSize,
            axis: match axis {
                AbsoluteAxis::Horizontal => RequestedAxis::Horizontal,
                AbsoluteAxis::Vertical => RequestedAxis::Vertical,
            },
            ..inputs
        };
        let previous = self.ratio_free.replace(dom_node_id(node_id));
        let output = self.compute_child_layout_in_current_wm(node_id, measure, None);
        self.ratio_free = previous;
        output.size.get_abs(axis)
    }
}
