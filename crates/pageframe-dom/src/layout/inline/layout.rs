#[cfg(feature = "floats")]
mod balance;
mod containing_block;
#[cfg(feature = "floats")]
mod lines;
mod measure;
mod placement;

use pageframe_traits::node_id::NodeId;
use parley::{AlignmentOptions, BreakReason, IndentOptions};
use style::values::{computed::CSSPixelLength, generics::text::GenericTextIndent};
use taffy::{
    AvailableSpace, BlockContainerStyle, BlockContext, BoxSizing, CollapsibleMarginSet,
    CoreStyle as _, LayoutInput, LayoutOutput, MaybeMath as _, MaybeResolve as _, OofCandidates,
    OofPositioningArea, Overflow, Point, RequestedAxis, ResolveOrZero as _, RunMode, Size,
    SizingMode,
};
#[cfg(feature = "floats")]
use taffy::{FloatInsets, prelude::TaffyMaxContent};

use crate::layout::{LayoutPassState, resolve_calc_value};
use crate::stylo_to_parley;
use stylo_taffy::CopyStyleGetters;

impl LayoutPassState<'_> {
    fn compute_inline_layout_inner(
        &mut self,
        node_id: NodeId,
        inputs: taffy::tree::LayoutInput,
        block_ctx: &mut BlockContext<'_>,
    ) -> taffy::LayoutOutput {
        let scale = self.viewport.scale();
        let LayoutInput {
            known_dimensions,
            parent_size,
            available_space,
            sizing_mode,
            ..
        } = inputs;

        let mut inline_layout = self.nodes[node_id]
            .data
            .downcast_element_mut()
            .unwrap()
            .take_inline_layout()
            .unwrap();

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
        drop(style);

        if !has_styles_preventing_being_collapsed_through
            && !inline_layout.has_text()
            && inline_layout.layout.inline_boxes().len() == 0
        {
            self.nodes[node_id]
                .data
                .downcast_element_mut()
                .unwrap()
                .inline_layout_data = Some(inline_layout);
            return LayoutOutput::from_outer_size(
                node_size
                    .or(node_min_size)
                    .unwrap_or(Size::ZERO)
                    .maybe_max(container_pb.sum_axes().map(Some)),
            );
        }

        let available_space = Size {
            width: known_dimensions
                .width
                .map(AvailableSpace::from)
                .unwrap_or(available_space.width)
                .maybe_set(known_dimensions.width)
                .maybe_set(node_size.width)
                .map_definite_value(|size| {
                    (size.maybe_clamp(node_min_size.width, node_max_size.width)
                        - content_box_inset.horizontal_axis_sum())
                    .max(0.0)
                }),
            height: known_dimensions
                .height
                .map(AvailableSpace::from)
                .unwrap_or(available_space.height)
                .maybe_set(known_dimensions.height)
                .maybe_set(node_size.height)
                .map_definite_value(|size| {
                    (size.maybe_clamp(node_min_size.height, node_max_size.height)
                        - content_box_inset.vertical_axis_sum())
                    .max(0.0)
                }),
        };

        let edge_basis = available_space.width.into_option().unwrap_or(0.0);
        if inline_layout.has_percent_edges
            && (edge_basis - inline_layout.percent_basis).abs() > f32::EPSILON
        {
            let document: &mut crate::BaseDocument = self;
            let font_ctx = std::sync::Arc::clone(&document.font_ctx);
            let mut font_ctx = font_ctx
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            inline_layout.percent_basis = edge_basis;
            inline_layout.content_widths = None;
            super::build_inline_layout_into(
                &document.nodes,
                &mut document.layout_ctx,
                &mut font_ctx,
                &mut inline_layout,
                scale,
                node_id,
            );
        }

        let child_inputs = taffy::tree::LayoutInput {
            known_dimensions: Size::NONE,
            available_space,
            sizing_mode: SizingMode::InherentSize,
            parent_size: Size {
                width: available_space.width.into_option(),
                height: if self.nodes[node_id].is_anonymous() {
                    parent_size.height
                } else {
                    available_space.height.into_option()
                },
            },
            vertical_margins_are_collapsible: taffy::Line::FALSE,
            ..inputs
        };
        #[cfg(feature = "floats")]
        let float_child_inputs = taffy::tree::LayoutInput {
            available_space: Size::MAX_CONTENT,
            ..child_inputs
        };

        self.measure_inline_boxes(&mut inline_layout, child_inputs, inputs.parent_size);
        let text_indent = self.nodes[node_id]
            .primary_styles()
            .map(|s| s.slow_clone_text_indent())
            .unwrap_or_else(GenericTextIndent::zero);
        let indent_options = IndentOptions {
            each_line: text_indent.each_line,
            hanging: text_indent.hanging,
        };
        inline_layout
            .layout
            .set_empty_boxes_open_lines(!self.in_quirks_mode);
        inline_layout
            .layout
            .set_strut_needs_text(self.in_quirks_mode);
        inline_layout.layout.set_text_indent(
            text_indent.length.resolve(CSSPixelLength::new(0.0)).px() * scale,
            indent_options,
        );

        let pbw = content_box_inset.horizontal_components().sum() * scale;
        let width = known_dimensions
            .width
            .map(|w| (w * scale) - pbw)
            .unwrap_or_else(|| {
                let content_sizes = inline_layout.layout.calculate_content_widths();
                let min_content_width = content_sizes.min;
                let max_content_width = content_sizes.max;

                #[cfg(feature = "floats")]
                let float_width = self.intrinsic_float_width(
                    &mut inline_layout,
                    child_inputs,
                    inputs.parent_size,
                );
                #[cfg(not(feature = "floats"))]
                let float_width = 0.0;

                let computed_width = match available_space.width {
                    AvailableSpace::MinContent => min_content_width.max(float_width),
                    AvailableSpace::MaxContent => max_content_width + float_width,
                    AvailableSpace::Definite(limit) => (limit * scale)
                        .min(max_content_width + float_width)
                        .max(min_content_width),
                };

                let style_width = node_size.width.map(|w| w * scale);
                let min_width = node_min_size.width.map(|w| w * scale);
                let max_width = node_max_size.width.map(|w| w * scale);

                (style_width)
                    .unwrap_or(computed_width + pbw)
                    .max(computed_width)
                    .maybe_clamp(min_width, max_width)
                    - pbw
            });

        #[cfg(not(feature = "floats"))]
        let _ = block_ctx;

        #[cfg(feature = "floats")]
        let is_bfc_root = block_ctx.is_bfc_root();
        #[cfg(feature = "floats")]
        if is_bfc_root {
            block_ctx.set_width((width + pbw) / scale);
        }

        #[cfg(feature = "floats")]
        let outer_block_ctx = block_ctx;
        #[cfg(feature = "floats")]
        let mut block_ctx = outer_block_ctx.sub_context(
            container_pb.top,
            FloatInsets::new(content_box_inset.left, content_box_inset.right),
        );

        if inputs.run_mode == taffy::RunMode::ComputeSize
            && inputs.axis == RequestedAxis::Horizontal
        {
            self.nodes[node_id]
                .data
                .downcast_element_mut()
                .unwrap()
                .inline_layout_data = Some(inline_layout);

            let measured_size = inputs.known_dimensions.unwrap_or(taffy::Size {
                width: width / scale,
                height: 0.0,
            });

            let clamped_size = inputs
                .known_dimensions
                .or(node_size)
                .unwrap_or(measured_size + content_box_inset.sum_axes())
                .maybe_clamp(node_min_size, node_max_size)
                .maybe_max(container_pb.sum_axes().map(Some));

            return LayoutOutput::from_outer_size(clamped_size);
        }

        let resolved_text_indent = text_indent
            .length
            .resolve(CSSPixelLength::new((width / scale).max(0.0)))
            .px();
        inline_layout
            .layout
            .set_text_indent(resolved_text_indent * scale, indent_options);

        let max_advance = {
            let max_content_width = inline_layout.layout.calculate_content_widths().max;
            let fit_width = width.max(max_content_width);
            let rounding_error = (fit_width + pbw).abs() * 8.0 * f32::EPSILON;
            if (max_content_width - width).abs() <= rounding_error {
                fit_width + rounding_error
            } else {
                width
            }
        };
        #[cfg(feature = "floats")]
        let slack = max_advance - width;

        #[cfg(not(feature = "floats"))]
        {
            inline_layout.layout.break_all_lines(Some(max_advance));
        }

        let mut oof_candidates = OofCandidates::new();

        let (alignment, last_line_alignment) = self.nodes[node_id]
            .primary_styles()
            .map(|s| {
                (
                    stylo_to_parley::text_align(s.clone_text_align()),
                    stylo_to_parley::text_align_last(s.clone_text_align_last()),
                )
            })
            .unwrap_or((parley::layout::Alignment::Start, None));

        #[cfg(feature = "floats")]
        let (line_bottoms, line_count_so_far) = self.break_inline_lines(
            &mut inline_layout,
            &mut block_ctx,
            &mut oof_candidates,
            lines::LineLayoutOptions {
                node_id,
                parent_size: inputs.parent_size,
                float_child_inputs,
                container_pb,
                max_advance,
                slack,
                alignment,
            },
        );
        #[cfg(feature = "floats")]
        let float_height_contribution = block_ctx.floated_content_height_contribution();
        #[cfg(feature = "floats")]
        outer_block_ctx.add_child_floated_content_height_contribution(
            container_pb.top + float_height_contribution,
        );

        inline_layout.layout.align(
            alignment,
            AlignmentOptions {
                align_when_overflowing: false,
                last_line_alignment,
            },
        );

        let has_inline_content =
            inline_layout.has_text() || inline_layout.layout.inline_boxes().len() > 0;

        let mut height = if has_inline_content {
            inline_layout.layout.height()
        } else {
            0.0
        };

        let line_count = inline_layout.layout.len();
        let mut trailing_empty_line = None;
        if line_count >= 2
            && let (Some(prev_line), Some(last_line)) = (
                inline_layout.layout.get(line_count - 2),
                inline_layout.layout.get(line_count - 1),
            )
            && prev_line.break_reason() == BreakReason::Explicit
            && last_line.text_range().is_empty()
            && last_line.items().next().is_none()
        {
            height -= last_line.metrics().line_height;
            trailing_empty_line = Some(line_count - 1);
        }

        #[cfg(feature = "floats")]
        {
            let counted_lines = match inline_layout.layout.get(line_count.wrapping_sub(1)) {
                Some(last_line) if last_line.is_empty() => line_count - 1,
                _ => trailing_empty_line.unwrap_or(line_count),
            };
            if has_inline_content
                && let Some(bottom) = counted_lines
                    .checked_sub(1)
                    .filter(|&last| last < line_count_so_far && last + 2 >= line_count_so_far)
                    .map(|last| line_bottoms[last % 2])
            {
                height = height.max(bottom as f32);
            }
        }

        #[cfg(feature = "floats")]
        {
            if is_bfc_root {
                height = height.max(float_height_contribution * scale)
            };
        }

        if let Some(max_lines) = self.nodes[node_id]
            .primary_styles()
            .and_then(|style| crate::util::clamped_line_count(&style, &inline_layout.layout))
            && let Some(last_line) = inline_layout.layout.get(max_lines - 1)
        {
            height = height.min(last_line.metrics().block_max_coord);
        }

        let measured_size = taffy::Size {
            width: width / scale,
            height: height / scale,
        };

        let content_width = f32_max(width, inline_layout.layout.width()) / scale;

        let clamped_size = inputs
            .known_dimensions
            .or(node_size)
            .unwrap_or(measured_size + content_box_inset.sum_axes())
            .maybe_clamp(node_min_size, node_max_size);
        let final_size = Size {
            width: clamped_size.width,
            height: f32_max(
                clamped_size.height,
                aspect_ratio
                    .map(|ratio| clamped_size.width / ratio)
                    .unwrap_or(0.0),
            ),
        }
        .maybe_max(container_pb.sum_axes().map(Some));

        let container_direction = self.nodes[node_id].layout_style().direction();

        let align_content = BlockContainerStyle::align_content(&self.nodes[node_id].layout_style());
        let block_offset = if align_content.keyword() == taffy::AlignContentKeyword::Normal {
            0.0
        } else {
            let free_space =
                final_size.height - content_box_inset.vertical_axis_sum() - measured_size.height;
            taffy::compute_block_align_content_offset(align_content, free_space)
        };
        inline_layout.block_offset = block_offset;
        let line_box_top = container_pb.top + block_offset;
        self.position_inline_boxes(
            &mut inline_layout,
            &mut oof_candidates,
            placement::BoxPlacement {
                node_id,
                child_inputs,
                final_size,
                container_pb,
                content_box_inset,
            },
        );
        let line_baseline =
            |line: parley::Line<'_, _>| (line.metrics().baseline / scale) + line_box_top;
        let exports_baseline = has_inline_content && !suppresses_baseline;
        let first_baseline = exports_baseline
            .then(|| inline_layout.layout.lines().next().map(line_baseline))
            .flatten();
        let last_line_index = trailing_empty_line.unwrap_or(line_count).checked_sub(1);
        let last_baseline = exports_baseline
            .then(|| last_line_index.and_then(|i| inline_layout.layout.get(i)))
            .flatten()
            .map(line_baseline);

        if inputs.run_mode == RunMode::PerformLayout {
            inline_layout.last_baseline = last_baseline;
        }

        self.nodes[node_id]
            .data
            .downcast_element_mut()
            .unwrap()
            .inline_layout_data = Some(inline_layout);

        let span_overflow = if inputs.run_mode == RunMode::PerformLayout {
            let placed = self.place_span_contained_boxes(
                node_id,
                &mut oof_candidates,
                final_size,
                padding,
                border,
            );
            if let Some(inline_layout) = self.nodes[node_id]
                .data
                .downcast_element_mut()
                .and_then(|element| element.inline_layout_data.as_deref_mut())
            {
                inline_layout.span_cb_flags = placed.fallback_flags;
                inline_layout.span_hoisted = placed.hoisted;
            }
            placed.overflow
        } else {
            None
        };

        let oof_position_inset = taffy::Rect {
            left: border.left,
            right: border.right + scrollbar_gutter.x,
            top: border.top,
            bottom: border.bottom + scrollbar_gutter.y,
        };

        LayoutOutput {
            size: final_size,
            scrollable_overflow_rect: {
                let content_extent = taffy::Size {
                    width: content_width,
                    height: measured_size.height,
                } + padding.sum_axes();
                let mut rect = taffy::Rect {
                    left: 0.0,
                    right: content_extent.width,
                    top: 0.0,
                    bottom: content_extent.height + block_offset.max(0.0),
                };
                let client_width =
                    final_size.width - border.left - border.right - scrollbar_gutter.x;
                let is_rtl = container_direction.is_rtl();
                if is_rtl {
                    rect.left = (client_width - rect.right).min(0.0);
                    rect.right = client_width.max(0.0);
                }
                if let Some(extra) = span_overflow {
                    rect.left = rect.left.min(extra.left);
                    rect.top = rect.top.min(extra.top);
                    rect.right = rect.right.max(extra.right);
                    rect.bottom = rect.bottom.max(extra.bottom);
                }
                if is_scroll_container {
                    rect.left = if is_rtl { rect.left } else { 0.0 };
                    rect.top = 0.0;
                }
                taffy::ScrollableOverflow::new(
                    rect,
                    taffy::Point {
                        x: is_scroll_container && is_rtl,
                        y: false,
                    },
                )
            },
            baselines: taffy::Baselines {
                first: first_baseline,
                last: last_baseline,
            },
            top_margin: CollapsibleMarginSet::ZERO,
            bottom_margin: CollapsibleMarginSet::ZERO,
            margins_can_collapse_through: !has_styles_preventing_being_collapsed_through
                && final_size.height == 0.0
                && measured_size.height == 0.0,
            oof_candidates,
            oof_positioning_area: Some(OofPositioningArea {
                size: final_size - oof_position_inset.sum_axes(),
                offset: Point {
                    x: oof_position_inset.left,
                    y: oof_position_inset.top,
                },
            }),
        }
    }
}

#[inline(always)]
fn f32_max(a: f32, b: f32) -> f32 {
    a.max(b)
}
