use markup5ever::local_name;
use pageframe_traits::node_id::NodeId;
use style::computed_values::float::T as Float;
use stylo_taffy::CopyStyleGetters;
use taffy::{
    AvailableSpace, BoxSizing, CoreStyle, Display, LayoutInput, LayoutOutput, LayoutPartialTree,
    Line, MaybeResolve, RequestedAxis, ResolveOrZero, RunMode, Size, SizingMode,
};

use crate::{
    BaseDocument,
    layout::{LayoutPassState, resolve_calc_value, round::DeviceSnap},
    taffy_node_id,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LegendGap {
    pub border_inset: f32,
    pub left: f32,
    pub right: f32,
    pub depth: f32,
}

struct FieldsetEdges {
    border_top: f32,
    horizontal: f32,
    vertical: f32,
}

impl BaseDocument {
    pub(crate) fn is_rendered_legend(&self, node_id: NodeId) -> bool {
        let node = &self.nodes[node_id];
        node.data.is_element_with_tag_name(&local_name!("legend"))
            && node
                .layout_parent
                .get()
                .is_some_and(|parent| self.rendered_legend(parent) == Some(node_id))
    }

    pub fn rendered_legend(&self, fieldset: NodeId) -> Option<NodeId> {
        let node = &self.nodes[fieldset];
        if !node.data.is_element_with_tag_name(&local_name!("fieldset"))
            || node.flags.is_inline_root()
            || !matches!(node.taffy_display(), Display::Block | Display::FlowRoot)
            || node
                .primary_styles()
                .is_none_or(|style| style.is_multicol())
        {
            return None;
        }
        let legend_id = node.children.iter().copied().find(|&child_id| {
            self.nodes[child_id]
                .data
                .is_element_with_tag_name(&local_name!("legend"))
        })?;
        let legend = &self.nodes[legend_id];
        let is_layout_child = node
            .layout_children
            .borrow()
            .as_ref()
            .is_some_and(|children| children.contains(&legend_id));
        let renders = is_layout_child
            && legend.taffy_display() != Display::None
            && !legend.is_out_of_flow()
            && legend
                .primary_styles()
                .is_some_and(|style| style.clone_float() == Float::None);
        renders.then_some(legend_id)
    }

    pub fn legend_gap(&self, fieldset: NodeId) -> Option<LegendGap> {
        let legend_id = self.rendered_legend(fieldset)?;
        let border_top = self.nodes[fieldset].final_layout().border.top;
        let legend = self.nodes[legend_id].final_layout();
        let legend_height = legend.margin.top + legend.size.height + legend.margin.bottom;
        let band = border_top.max(legend_height);
        Some(LegendGap {
            border_inset: self.snapped_border_inset(fieldset, (band - border_top) / 2.0),
            left: legend.location.x,
            right: legend.location.x + legend.size.width,
            depth: band,
        })
    }

    fn snapped_border_inset(&self, fieldset: NodeId, inset: f32) -> f32 {
        let snap = DeviceSnap::new(self.viewport.scale());
        let mut top = 0.0;
        let mut current = Some(fieldset);
        while let Some(id) = current {
            let node = &self.nodes[id];
            top += node.unrounded_layout().location.y;
            current = node.containing_block();
        }
        snap.round(top + inset) - snap.round(top)
    }
}

impl LayoutPassState<'_> {
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
        let edges = FieldsetEdges {
            border_top: border.top,
            horizontal: border.left + border.right + padding.left + padding.right,
            vertical: border.top + border.bottom + padding.top + padding.bottom,
        };
        let box_sizing = style.box_sizing();
        let to_border_box = |length: f32, edge: f32| match box_sizing {
            BoxSizing::ContentBox => length + edge,
            BoxSizing::BorderBox => length,
        };
        let size = style.size().maybe_resolve(parent_size, resolve_calc_value);
        let height_is_auto = inputs.known_dimensions.height.is_none() && size.height.is_none();
        let min_height = style
            .min_size()
            .maybe_resolve(parent_size, resolve_calc_value)
            .height
            .map_or(0.0, |height| to_border_box(height, edges.vertical));
        let max_height = style
            .max_size()
            .maybe_resolve(parent_size, resolve_calc_value)
            .height
            .map(|height| to_border_box(height, edges.vertical));
        let margin = style
            .margin()
            .resolve_or_zero(parent_size.width, resolve_calc_value);
        drop(style);

        let border_box_width = inputs.known_dimensions.width.or(size
            .width
            .map(|width| to_border_box(width, edges.horizontal)));
        let content_width = match (border_box_width, inputs.available_space.width) {
            (Some(width), _) => AvailableSpace::Definite((width - edges.horizontal).max(0.0)),
            (None, AvailableSpace::Definite(space)) => AvailableSpace::Definite(
                (space - margin.left - margin.right - edges.horizontal).max(0.0),
            ),
            (None, space) => space,
        };

        let legend_height = self.legend_margin_box_height(legend_id, inputs, content_width);
        let band = edges.border_top.max(legend_height);
        let shift = edges.border_top.min(legend_height);
        if inputs.run_mode == RunMode::PerformLayout {
            let legend_top = self.legend_margin_top(legend_id, content_width);
            let children = self.nodes[fieldset]
                .layout_children
                .borrow()
                .clone()
                .unwrap_or_default();
            for child_id in children {
                if child_id == legend_id {
                    self.nodes[child_id].unrounded_layout_mut().location.y =
                        (band - legend_height) / 2.0 + legend_top;
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

    fn legend_margin_top(&self, legend_id: NodeId, content_width: AvailableSpace) -> f32 {
        self.nodes[legend_id]
            .layout_style()
            .margin()
            .resolve_or_zero(content_width.into_option(), resolve_calc_value)
            .top
    }

    fn legend_margin_box_height(
        &mut self,
        legend_id: NodeId,
        inputs: &LayoutInput,
        content_width: AvailableSpace,
    ) -> f32 {
        let margin = self.nodes[legend_id]
            .layout_style()
            .margin()
            .resolve_or_zero(content_width.into_option(), resolve_calc_value);
        let height = if inputs.run_mode == RunMode::PerformLayout {
            self.nodes[legend_id].unrounded_layout().size.height
        } else {
            let measure = LayoutInput {
                run_mode: RunMode::ComputeSize,
                sizing_mode: SizingMode::InherentSize,
                axis: RequestedAxis::Both,
                known_dimensions: Size::NONE,
                known_dimensions_are_definite: Size {
                    width: true,
                    height: true,
                },
                parent_size: Size {
                    width: content_width.into_option(),
                    height: None,
                },
                available_space: Size {
                    width: content_width,
                    height: AvailableSpace::MaxContent,
                },
                vertical_margins_are_collapsible: Line::FALSE,
            };
            self.compute_child_layout(taffy_node_id(legend_id), measure)
                .size
                .height
        };
        margin.top + height + margin.bottom
    }
}
