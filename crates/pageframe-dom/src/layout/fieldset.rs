use markup5ever::local_name;
use pageframe_traits::node_id::NodeId;
use style::computed_values::float::T as Float;
use stylo_taffy::CopyStyleGetters;
use taffy::{
    BoxSizing, CoreStyle, LayoutInput, LayoutOutput, MaybeResolve, ResolveOrZero, RunMode,
};

use crate::{BaseDocument, layout::resolve_calc_value};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LegendGap {
    pub border_inset: f32,
    pub left: f32,
    pub right: f32,
    pub depth: f32,
}

impl BaseDocument {
    pub fn rendered_legend(&self, fieldset: NodeId) -> Option<NodeId> {
        let node = &self.nodes[fieldset];
        if !node.data.is_element_with_tag_name(&local_name!("fieldset")) {
            return None;
        }
        let children = node.layout_children.borrow();
        children.as_ref()?.iter().copied().find(|&child_id| {
            let child = &self.nodes[child_id];
            child.data.is_element_with_tag_name(&local_name!("legend"))
                && !child.is_out_of_flow()
                && child
                    .primary_styles()
                    .is_some_and(|style| style.clone_float() == Float::None)
        })
    }

    pub fn legend_gap(&self, fieldset: NodeId) -> Option<LegendGap> {
        let legend_id = self.rendered_legend(fieldset)?;
        let border_top = self.nodes[fieldset].final_layout().border.top;
        let legend = self.nodes[legend_id].final_layout();
        let legend_height = legend.margin.top + legend.size.height + legend.margin.bottom;
        let band = border_top.max(legend_height);
        Some(LegendGap {
            border_inset: (band - border_top) / 2.0,
            left: legend.location.x,
            right: legend.location.x + legend.size.width,
            depth: band,
        })
    }

    pub(crate) fn needs_legend_pass(&self, fieldset: NodeId, inputs: &LayoutInput) -> bool {
        let size_known =
            inputs.known_dimensions.width.is_some() && inputs.known_dimensions.height.is_some();
        let passes = match inputs.run_mode {
            RunMode::PerformLayout => true,
            RunMode::ComputeSize => !size_known,
            RunMode::PerformHiddenLayout => false,
        };
        passes && self.rendered_legend(fieldset).is_some()
    }

    pub(crate) fn place_rendered_legend(
        &mut self,
        fieldset: NodeId,
        inputs: &LayoutInput,
        output: &mut LayoutOutput,
    ) {
        let Some(legend_id) = self.rendered_legend(fieldset) else {
            return;
        };
        let node = &self.nodes[fieldset];
        let style = node.layout_style();
        let parent_size = inputs.parent_size;
        let border = style
            .border()
            .resolve_or_zero(parent_size.width, resolve_calc_value);
        let padding = style
            .padding()
            .resolve_or_zero(parent_size.width, resolve_calc_value);
        let edges = border.top + border.bottom + padding.top + padding.bottom;
        let to_border_box = |height: f32| match style.box_sizing() {
            BoxSizing::ContentBox => height + edges,
            BoxSizing::BorderBox => height,
        };
        let height_is_auto = inputs.known_dimensions.height.is_none()
            && style
                .size()
                .maybe_resolve(parent_size, resolve_calc_value)
                .height
                .is_none();
        let min_height = style
            .min_size()
            .maybe_resolve(parent_size, resolve_calc_value)
            .height
            .map_or(0.0, to_border_box);
        let max_height = style
            .max_size()
            .maybe_resolve(parent_size, resolve_calc_value)
            .height
            .map(to_border_box);
        drop(style);

        let legend = *self.nodes[legend_id].unrounded_layout();
        let legend_height = legend.margin.top + legend.size.height + legend.margin.bottom;
        let band = border.top.max(legend_height);
        let shift = border.top.min(legend_height);
        if inputs.run_mode == RunMode::PerformLayout {
            let children = self.nodes[fieldset]
                .layout_children
                .borrow()
                .clone()
                .unwrap_or_default();
            for child_id in children {
                if child_id == legend_id {
                    self.nodes[child_id].unrounded_layout_mut().location.y =
                        (band - legend_height) / 2.0 + legend.margin.top;
                } else if !self.nodes[child_id].is_out_of_flow() {
                    self.nodes[child_id].unrounded_layout_mut().location.y -= shift;
                }
            }
        }
        output.baselines.first = output.baselines.first.map(|baseline| baseline - shift);
        output.baselines.last = output.baselines.last.map(|baseline| baseline - shift);
        let clamped = max_height.is_some_and(|max| output.size.height >= max);
        if height_is_auto && !clamped {
            output.size.height = (output.size.height - shift).max(min_height);
        }
    }
}
