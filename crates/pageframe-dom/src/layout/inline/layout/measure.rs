use super::LayoutPassState;
use crate::layout::resolve_calc_value;
use crate::node::TextLayout;
use pageframe_traits::node_id::NodeId;
use style::values::computed::{BaselineSource, Contain};
use style::values::specified::box_::DisplayInside;
use stylo_taffy::StyleFlags;
use taffy::{
    AvailableSpace, BlockContext, BlockFormattingContext, BoxSizing, CompactLength, CoreStyle as _,
    LayoutInput, LayoutOutput, LayoutPartialTree as _, MaybeMath as _, MaybeResolve as _, Overflow,
    RequestedAxis, ResolveOrZero as _, RunMode, Size, SizingMode,
};
#[cfg(feature = "floats")]
use taffy::{BlockItemStyle as _, Clear, Float};

enum LastBaseline {
    At(f32),
    Missing,
    Unknown,
}

fn subtract_margins(mut child_inputs: LayoutInput, margin: taffy::Rect<f32>) -> LayoutInput {
    child_inputs.available_space = child_inputs
        .available_space
        .maybe_sub(margin.sum_axes())
        .maybe_max(Size::ZERO);
    child_inputs
}

pub(super) fn inline_box_inputs(
    width_style: taffy::Dimension,
    margin: taffy::Rect<f32>,
    child_inputs: LayoutInput,
) -> LayoutInput {
    let mut inputs = subtract_margins(child_inputs, margin);
    let stretch_width = inputs.available_space.width.into_option();
    let percent_basis = child_inputs.parent_size.width;

    match width_style.tag() {
        CompactLength::MIN_CONTENT_TAG => inputs.available_space.width = AvailableSpace::MinContent,
        CompactLength::MAX_CONTENT_TAG => inputs.available_space.width = AvailableSpace::MaxContent,
        CompactLength::FIT_CONTENT_PX_TAG => {
            inputs.available_space.width = AvailableSpace::Definite(width_style.value())
        }
        CompactLength::FIT_CONTENT_PERCENT_TAG => {
            if let Some(basis) = percent_basis {
                inputs.available_space.width =
                    AvailableSpace::Definite(basis * width_style.value());
            }
        }
        CompactLength::FIT_CONTENT_KEYWORD_TAG => {
            if let Some(width) = stretch_width {
                inputs.available_space.width = AvailableSpace::Definite(width);
            }
        }
        _ if width_style.is_fit_content_calc() => {
            if let Some(basis) = percent_basis {
                inputs.available_space.width =
                    AvailableSpace::Definite(resolve_calc_value(width_style.calc_value(), basis));
            }
        }
        CompactLength::STRETCH_TAG => {
            if let Some(width) = stretch_width {
                inputs.known_dimensions.width = Some(width);
                inputs.available_space.width = AvailableSpace::Definite(width);
            }
        }
        _ => {}
    }
    inputs.run_mode = RunMode::PerformLayout;
    inputs.axis = RequestedAxis::Both;
    inputs
}

impl LayoutPassState<'_> {
    pub(crate) fn compute_inline_layout(
        &mut self,
        node_id: NodeId,
        inputs: taffy::tree::LayoutInput,
        block_ctx: Option<&mut BlockContext<'_>>,
    ) -> taffy::LayoutOutput {
        let LayoutInput {
            known_dimensions,
            parent_size,
            run_mode,
            ..
        } = inputs;
        let style = self.nodes[node_id].layout_style();

        let is_scroll_container =
            style.overflow().x.is_scroll_container() || style.overflow().y.is_scroll_container();
        let padding = style
            .padding()
            .resolve_or_zero(parent_size.width, resolve_calc_value);
        let border = style
            .border()
            .resolve_or_zero(parent_size.width, resolve_calc_value);
        let padding_border_size = (padding + border).sum_axes();
        let box_sizing_adjustment = if style.box_sizing() == BoxSizing::ContentBox {
            padding_border_size
        } else {
            Size::ZERO
        };

        let (clamped_style_size, min_size, max_size, _aspect_ratio) = match inputs.sizing_mode {
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

        let min_max_definite_size = min_size.zip_map(max_size, |min, max| match (min, max) {
            (Some(min), Some(max)) if max <= min => Some(min),
            _ => None,
        });

        let styled_based_known_dimensions = known_dimensions
            .or(min_max_definite_size)
            .or(clamped_style_size)
            .maybe_max(padding_border_size);

        if run_mode == RunMode::ComputeSize {
            if let Size {
                width: Some(width),
                height: Some(height),
            } = styled_based_known_dimensions
            {
                return LayoutOutput::from_outer_size(Size { width, height });
            }
        }

        drop(style);

        match block_ctx {
            Some(inherited_bfc) if !is_scroll_container => self.compute_inline_layout_inner(
                node_id,
                LayoutInput {
                    known_dimensions: styled_based_known_dimensions,
                    ..inputs
                },
                inherited_bfc,
            ),
            _ => {
                let mut root_bfc = BlockFormattingContext::new();
                let mut root_ctx = root_bfc.root_block_context();
                self.compute_inline_layout_inner(
                    node_id,
                    LayoutInput {
                        known_dimensions: styled_based_known_dimensions,
                        ..inputs
                    },
                    &mut root_ctx,
                )
            }
        }
    }

    fn last_line_baseline(&self, box_id: NodeId) -> LastBaseline {
        let mut pending = vec![(box_id, 0.0)];
        let mut is_box = true;
        while let Some((id, top)) = pending.pop() {
            let node = &self.nodes[id];
            if !std::mem::take(&mut is_box) {
                let style = self.child_layout_style(node);
                if style.style.get_box().display.is_none() {
                    continue;
                }
                #[cfg(feature = "floats")]
                let is_floated = style.float().is_floated();
                #[cfg(not(feature = "floats"))]
                let is_floated = false;
                if is_floated
                    || style.position().is_out_of_flow()
                    || node.flags.is_table_root()
                    || self.is_rendered_legend(id)
                {
                    continue;
                }
                let is_block = matches!(
                    style.effective_display().inside(),
                    DisplayInside::Flow | DisplayInside::FlowRoot
                );
                if !is_block {
                    return LastBaseline::Unknown;
                }
            }
            let top = top + node.unrounded_layout().location.y;
            if node.flags.is_inline_root() {
                let baseline = node
                    .element_data()
                    .and_then(|element| element.inline_layout_data.as_deref())
                    .and_then(|layout| layout.last_baseline);
                if let Some(baseline) = baseline {
                    return LastBaseline::At(top + baseline);
                }
                continue;
            }
            if let Some(children) = node.layout_children.borrow().as_ref() {
                pending.extend(children.iter().map(|child_id| (*child_id, top)));
            }
        }
        LastBaseline::Missing
    }

    pub(super) fn measure_inline_boxes(
        &mut self,
        inline_layout: &mut TextLayout,
        child_inputs: LayoutInput,
        parent_size: Size<Option<f32>>,
    ) {
        let scale = self.viewport.scale();
        for ibox in inline_layout.layout.inline_boxes_mut() {
            if ibox.kind == parley::InlineBoxKind::EmptySpan {
                continue;
            }
            let style = self.child_layout_style(&self.nodes[NodeId::from_u64(ibox.id)]);
            let margin = style
                .margin()
                .resolve_or_zero(parent_size, resolve_calc_value);

            #[cfg(feature = "floats")]
            let is_floated = style.float().is_floated();
            #[cfg(not(feature = "floats"))]
            let is_floated = false;

            let is_out_of_flow = style.position().is_out_of_flow();
            let overflow = style.overflow();
            let box_style = style.style.get_box();
            let is_flow = matches!(
                style.effective_display().inside(),
                DisplayInside::Flow | DisplayInside::FlowRoot
            );
            let baseline_source = box_style.slow_clone_baseline_source();
            let uses_last_baseline = match baseline_source {
                BaselineSource::Auto => is_flow,
                BaselineSource::First => false,
                BaselineSource::Last => true,
            };
            let is_scroll_container = !matches!(overflow.y, Overflow::Visible | Overflow::Clip);
            let is_block_axis_scroll_container = is_flow
                && is_scroll_container
                && !style
                    .flags
                    .intersects(StyleFlags::IS_BUTTON | StyleFlags::IS_BUTTON_INPUT);
            let contain_layout = box_style.slow_clone_contain().contains(Contain::LAYOUT);
            let exports_baseline = !(contain_layout
                || (baseline_source == BaselineSource::Auto && is_block_axis_scroll_container));
            let box_inputs = inline_box_inputs(style.size().width, margin, child_inputs);
            drop(style);
            let box_id = NodeId::from_u64(ibox.id);
            let margin = self.ruby_line_margin(box_id, margin, parent_size);
            let walks_blocks = is_flow
                && uses_last_baseline
                && exports_baseline
                && !self.nodes[box_id].flags.is_inline_root()
                && !self.nodes[box_id].is_ruby_box()
                && self.nodes[box_id]
                    .layout_children
                    .borrow()
                    .as_ref()
                    .is_some_and(|children| !children.is_empty());

            if is_out_of_flow || is_floated {
                ibox.width = 0.0;
                ibox.height = 0.0;
                ibox.baseline = None;
            } else {
                let output = self.compute_child_layout(taffy::NodeId::from(ibox.id), box_inputs);
                ibox.width = (margin.left + margin.right + output.size.width) * scale;
                let last_baseline = match walks_blocks {
                    true => self.last_line_baseline(box_id),
                    false => LastBaseline::Unknown,
                };
                ibox.baseline = if exports_baseline {
                    let baseline = match last_baseline {
                        LastBaseline::At(baseline) => {
                            Some(baseline - self.nodes[box_id].unrounded_layout().location.y)
                        }
                        LastBaseline::Missing => None,
                        LastBaseline::Unknown if uses_last_baseline => {
                            output.baselines.last.or(output.baselines.first)
                        }
                        LastBaseline::Unknown => output.baselines.first.or(output.baselines.last),
                    };
                    baseline.map(|baseline| {
                        let baseline = if is_scroll_container {
                            baseline.clamp(0.0, output.size.height)
                        } else {
                            baseline
                        };
                        (margin.top + baseline) * scale
                    })
                } else {
                    None
                };
                let margin_box_height = margin.top + margin.bottom + output.size.height;
                ibox.height = if ibox.baseline.is_some() {
                    (margin_box_height * scale).min(f32::MAX)
                } else {
                    (margin_box_height.max(0.0) * scale).min(f32::MAX)
                };
            }
        }
    }

    #[cfg(feature = "floats")]
    pub(super) fn intrinsic_float_width(
        &mut self,
        inline_layout: &mut TextLayout,
        child_inputs: LayoutInput,
        parent_size: Size<Option<f32>>,
    ) -> f32 {
        let scale = self.viewport.scale();
        match child_inputs.available_space.width {
            AvailableSpace::MinContent => {
                let mut width: f32 = 0.0;
                for ibox in inline_layout.layout.inline_boxes_mut() {
                    let (is_floated, margin) = {
                        let style = self.child_layout_style(&self.nodes[NodeId::from_u64(ibox.id)]);
                        (
                            style.float().is_floated(),
                            style
                                .margin()
                                .resolve_or_zero(parent_size, resolve_calc_value),
                        )
                    };

                    if is_floated {
                        let output = self.compute_child_layout(
                            taffy::NodeId::from(ibox.id),
                            subtract_margins(child_inputs, margin),
                        );
                        width = width.max(output.size.width + margin.left + margin.right);
                    }
                }

                width * scale
            }
            AvailableSpace::MaxContent | AvailableSpace::Definite(_) => {
                let mut left_band: f32 = 0.0;
                let mut right_band: f32 = 0.0;
                let mut width: f32 = 0.0;
                for ibox in inline_layout.layout.inline_boxes_mut() {
                    let (float, clear, margin) = {
                        let style = self.child_layout_style(&self.nodes[NodeId::from_u64(ibox.id)]);
                        (
                            style.float(),
                            style.clear(),
                            style
                                .margin()
                                .resolve_or_zero(parent_size, resolve_calc_value),
                        )
                    };

                    if float.is_floated() {
                        if matches!(clear, Clear::Left | Clear::Both) {
                            left_band = 0.0;
                        }
                        if matches!(clear, Clear::Right | Clear::Both) {
                            right_band = 0.0;
                        }

                        let output = self.compute_child_layout(
                            taffy::NodeId::from(ibox.id),
                            subtract_margins(child_inputs, margin),
                        );
                        let box_width = output.size.width + margin.left + margin.right;

                        match float {
                            Float::Left => left_band += box_width,
                            Float::Right => right_band += box_width,
                            Float::None => {}
                        }
                        width = width.max(left_band + right_band);
                    }
                }

                width * scale
            }
        }
    }
}
