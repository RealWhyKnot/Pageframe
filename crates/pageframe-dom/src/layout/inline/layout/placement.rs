use super::LayoutPassState;
use super::measure::inline_box_inputs;
use crate::layout::resolve_calc_value;
use crate::node::TextLayout;
use pageframe_traits::node_id::NodeId;
use style::values::specified::box_::DisplayOutside;
use stylo_taffy::WritingModeExt;
use taffy::{
    AxisStaticPosition, CoreStyle as _, Direction, LayoutInput, LayoutPartialTree as _,
    MaybeResolve as _, OofCandidate, OofCandidates, OofItemStyle, ResolveOrZero as _, Size,
};
#[cfg(feature = "floats")]
use taffy::{BlockItemStyle as _, Float};

pub(super) struct BoxPlacement {
    pub(super) node_id: NodeId,
    pub(super) child_inputs: LayoutInput,
    pub(super) final_size: Size<f32>,
    pub(super) container_pb: taffy::Rect<f32>,
    pub(super) content_box_inset: taffy::Rect<f32>,
}

impl LayoutPassState<'_> {
    pub(super) fn position_inline_boxes(
        &mut self,
        inline_layout: &mut TextLayout,
        oof_candidates: &mut OofCandidates,
        placement: BoxPlacement,
    ) {
        let BoxPlacement {
            node_id,
            child_inputs,
            final_size,
            container_pb,
            content_box_inset,
        } = placement;
        let scale = self.viewport.scale();
        let container_direction = self.nodes[node_id].layout_style().direction();
        let block_offset = inline_layout.block_offset;
        let line_box_top = container_pb.top + block_offset;
        let content_block_size = final_size.height - content_box_inset.vertical_axis_sum();
        let writing_mode = self.nodes[node_id]
            .primary_styles()
            .map(|style| style.writing_mode);
        let is_sideways_lr = writing_mode.is_some_and(WritingModeExt::line_left_is_bottom);
        let mirrors_lines =
            writing_mode.is_some_and(|mode| mode.is_vertical_lr() && !mode.is_sideways());
        let inline_far_edge = final_size.width - container_pb.right;
        let inline_position = |start: f32, length: f32| {
            if is_sideways_lr {
                inline_far_edge - start - length
            } else {
                start + container_pb.left
            }
        };
        if mirrors_lines {
            inline_layout
                .layout
                .mirror_lines_in_block_axis((content_block_size - 2.0 * block_offset) * scale);
        }

        let mut ibox_order: u32 = 0;
        let has_inline_boxes = inline_layout.layout.inline_boxes().len() > 0;
        let lines = has_inline_boxes.then(|| inline_layout.layout.lines());
        for line in lines.into_iter().flatten() {
            let mut line_has_in_flow_content = false;
            for item in line.items() {
                if let parley::layout::PositionedLayoutItem::GlyphRun(run) = &item {
                    line_has_in_flow_content |= run.advance() > 0.0;
                }
                if let parley::layout::PositionedLayoutItem::InlineBox(ibox) = item {
                    let order = ibox_order;
                    ibox_order += 1;
                    let node = &self.nodes[NodeId::from_u64(ibox.id)];
                    let (
                        padding,
                        border,
                        margin,
                        is_floated,
                        position,
                        is_absolute,
                        item_direction,
                        justify_self,
                        align_self,
                        is_inline_level,
                        inset,
                        is_block_level,
                        auto_margins,
                        box_inputs,
                    ) = {
                        let style = self.child_layout_style(node);
                        let padding = style
                            .padding()
                            .resolve_or_zero(child_inputs.parent_size, resolve_calc_value);
                        let border = style
                            .border()
                            .resolve_or_zero(child_inputs.parent_size, resolve_calc_value);
                        let margin = style
                            .margin()
                            .resolve_or_zero(child_inputs.parent_size, resolve_calc_value);

                        #[cfg(feature = "floats")]
                        let is_floated = style.float() != Float::None;
                        #[cfg(not(feature = "floats"))]
                        let is_floated = false;

                        let position = style.position();
                        let is_absolute = position.is_out_of_flow();
                        let item_direction = style.direction();
                        let justify_self =
                            OofItemStyle::justify_self(&style).unwrap_or(taffy::AlignItems::NORMAL);
                        let align_self =
                            OofItemStyle::align_self(&style).unwrap_or(taffy::AlignItems::NORMAL);

                        let is_inline_level = style.style.get_box().original_display.outside()
                            == DisplayOutside::Inline;

                        let container_content_size = final_size - content_box_inset.sum_axes();
                        let inset_style = style.inset();
                        let inset = taffy::Rect {
                            left: inset_style
                                .left
                                .maybe_resolve(container_content_size.width, resolve_calc_value),
                            right: inset_style
                                .right
                                .maybe_resolve(container_content_size.width, resolve_calc_value),
                            top: inset_style
                                .top
                                .maybe_resolve(container_content_size.height, resolve_calc_value),
                            bottom: inset_style
                                .bottom
                                .maybe_resolve(container_content_size.height, resolve_calc_value),
                        };
                        let is_block_level = !is_floated
                            && !is_absolute
                            && style.effective_display().outside() == DisplayOutside::Block;
                        let auto_margins = [
                            style.margin().left.is_auto(),
                            style.margin().right.is_auto(),
                        ];
                        let box_inputs = inline_box_inputs(
                            style.size().width,
                            margin,
                            child_inputs,
                            is_block_level,
                        );
                        (
                            padding,
                            border,
                            margin,
                            is_floated,
                            position,
                            is_absolute,
                            item_direction,
                            justify_self,
                            align_self,
                            is_inline_level,
                            inset,
                            is_block_level,
                            auto_margins,
                            box_inputs,
                        )
                    };

                    if is_absolute {
                        let line_metrics = line.metrics();
                        let (line_top, line_bottom) = if mirrors_lines {
                            let far_edge = container_pb.top + content_block_size - block_offset;
                            (
                                far_edge - line_metrics.block_max_coord / scale,
                                far_edge - line_metrics.block_min_coord / scale,
                            )
                        } else {
                            (
                                (line_metrics.block_min_coord / scale) + line_box_top,
                                (line_metrics.block_max_coord / scale) + line_box_top,
                            )
                        };
                        let (inline_area, block_area) = if is_inline_level {
                            let x = inline_position(ibox.x / scale, 0.0);
                            (
                                taffy::Line { start: x, end: x },
                                taffy::Line {
                                    start: line_top,
                                    end: line_bottom,
                                },
                            )
                        } else {
                            let y = if line_has_in_flow_content {
                                line_bottom
                            } else {
                                line_top
                            };
                            (
                                taffy::Line {
                                    start: container_pb.left,
                                    end: final_size.width - container_pb.right,
                                },
                                taffy::Line { start: y, end: y },
                            )
                        };

                        oof_candidates.push(OofCandidate {
                            node: taffy::NodeId::from(ibox.id),
                            order,
                            position,
                            static_position: taffy::Point {
                                x: AxisStaticPosition::from_alignment(
                                    justify_self.resolve_self_relative(
                                        item_direction,
                                        container_direction,
                                        true,
                                    ),
                                    inline_area,
                                    container_direction.is_rtl(),
                                ),
                                y: AxisStaticPosition::from_alignment(
                                    align_self.resolve_self_relative(
                                        item_direction,
                                        container_direction,
                                        false,
                                    ),
                                    block_area,
                                    false,
                                ),
                            },
                        });
                    } else if is_floated {
                        let layout = self.nodes[NodeId::from_u64(ibox.id)].unrounded_layout_mut();
                        layout.padding = padding;
                        layout.border = border;
                        layout.margin = margin;
                    } else {
                        line_has_in_flow_content = true;
                        let mut output =
                            self.compute_child_layout(taffy::NodeId::from(ibox.id), box_inputs);
                        let size = output.size;
                        let line_margin = self.placed_ruby_margin(
                            NodeId::from_u64(ibox.id),
                            margin,
                            child_inputs.parent_size,
                        );
                        let (span_x, span_y) = self.nodes[NodeId::from_u64(ibox.id)]
                            .parent
                            .map_or((0.0, 0.0), |parent| {
                                self.nodes[parent].inline_relative_shift()
                            });
                        let node = &mut self.nodes[NodeId::from_u64(ibox.id)];

                        let is_relative = position == taffy::Position::Relative;
                        let inset_offset = if is_relative {
                            taffy::Point {
                                x: if container_direction == Direction::Rtl {
                                    inset.right.map(|x| -x).or(inset.left).unwrap_or(0.0)
                                } else {
                                    inset.left.or(inset.right.map(|x| -x)).unwrap_or(0.0)
                                },
                                y: inset.top.or(inset.bottom.map(|x| -x)).unwrap_or(0.0),
                            }
                        } else {
                            taffy::Point::ZERO
                        };

                        let free =
                            (ibox.width / scale - margin.left - margin.right - size.width).max(0.0);
                        let block_shift = match auto_margins {
                            _ if !is_block_level => 0.0,
                            [true, true] => free / 2.0,
                            [true, false] => free,
                            [false, false] if container_direction == Direction::Rtl => free,
                            [_, _] => 0.0,
                        };

                        let layout = node.unrounded_layout_mut();
                        layout.size = size;
                        layout.scrollable_overflow_rect = output.scrollable_overflow_rect;
                        layout.location.x = if is_sideways_lr {
                            inline_position(ibox.x / scale, margin.right + size.width)
                        } else {
                            inline_position(ibox.x / scale, 0.0) + margin.left + block_shift
                        } + inset_offset.x
                            + span_x;
                        let margin_box_top = ibox.y / scale;
                        layout.location.y = if mirrors_lines {
                            container_pb.top + content_block_size
                                - block_offset
                                - margin_box_top
                                - line_margin.top
                                - size.height
                                + inset_offset.y
                        } else {
                            margin_box_top + line_margin.top + line_box_top + inset_offset.y
                        } + span_y;
                        layout.padding = padding;
                        layout.border = border;
                        layout.margin = margin;

                        if !output.oof_candidates.is_empty() {
                            let location = layout.location;
                            output.oof_candidates.translate(location);
                            oof_candidates.append(&mut output.oof_candidates);
                        }
                    }
                }
            }
        }
    }
}
