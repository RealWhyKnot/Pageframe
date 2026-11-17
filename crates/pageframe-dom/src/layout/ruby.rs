use markup5ever::local_name;
use pageframe_traits::node_id::NodeId;
use style::computed_values::direction::T as Direction;
use style::values::specified::box_::DisplayOutside;
use stylo_taffy::CopyStyleGetters as _;
use taffy::{
    AvailableSpace, Baselines, CoreStyle, Layout, LayoutInput, LayoutOutput, LayoutPartialTree,
    Line, Point, Rect, RequestedAxis, ResolveOrZero, RunMode, ScrollableOverflow, Size, SizingMode,
};

use crate::{
    layout::{LayoutPassState, resolve_calc_value},
    node::NodeData,
    taffy_node_id,
};

#[derive(Clone, Copy)]
struct Column {
    base: Option<NodeId>,
    annotation: Option<NodeId>,
}

struct Measured {
    node: NodeId,
    size: Size<f32>,
    baseline: Option<f32>,
}

struct Placed {
    measured: Measured,
    location: Point<f32>,
}

fn pair_columns(children: &[NodeId], is_annotation: impl Fn(NodeId) -> bool) -> Vec<Column> {
    let mut columns = Vec::new();
    let mut bases = Vec::new();
    let mut annotations = Vec::new();
    let mut flush = |bases: &mut Vec<NodeId>, annotations: &mut Vec<NodeId>| {
        for index in 0..bases.len().max(annotations.len()) {
            columns.push(Column {
                base: bases.get(index).copied(),
                annotation: annotations.get(index).copied(),
            });
        }
        bases.clear();
        annotations.clear();
    };
    for &child in children {
        if is_annotation(child) {
            annotations.push(child);
        } else {
            if !annotations.is_empty() {
                flush(&mut bases, &mut annotations);
            }
            bases.push(child);
        }
    }
    flush(&mut bases, &mut annotations);
    columns
}

fn child_input(run_mode: RunMode, known_dimensions: Size<Option<f32>>) -> LayoutInput {
    LayoutInput {
        run_mode,
        sizing_mode: SizingMode::InherentSize,
        axis: RequestedAxis::Both,
        known_dimensions,
        known_dimensions_are_definite: Size {
            width: true,
            height: true,
        },
        parent_size: Size::NONE,
        available_space: Size {
            width: AvailableSpace::MaxContent,
            height: AvailableSpace::MaxContent,
        },
        vertical_margins_are_collapsible: Line::FALSE,
    }
}

impl LayoutPassState<'_> {
    fn ruby_child_size(&mut self, node: NodeId) -> Size<f32> {
        self.compute_child_layout(
            taffy_node_id(node),
            child_input(RunMode::ComputeSize, Size::NONE),
        )
        .size
    }

    fn measure_ruby_child(&mut self, node: NodeId, size: Size<f32>) -> Measured {
        let output = self.compute_child_layout(
            taffy_node_id(node),
            child_input(RunMode::PerformLayout, size.map(Some)),
        );
        Measured {
            node,
            size: output.size,
            baseline: output.baselines.first,
        }
    }

    fn em_above_baseline(&self, node: NodeId) -> Option<f32> {
        let style = self.nodes[node].primary_styles()?;
        let font_size = style.slow_clone_font_size().used_size().px();
        let scale = self.viewport.scale();
        let metrics = crate::font_metrics::form_control_metrics(
            &mut *self.font_ctx.lock().ok()?,
            style.get_font(),
            font_size,
            scale,
        )?;
        Some((font_size * scale - metrics.descent.round()) / scale)
    }

    fn font_size(&self, node: NodeId) -> Option<f32> {
        self.nodes[node]
            .primary_styles()
            .map(|style| style.slow_clone_font_size().used_size().px())
    }

    fn ruby_neighbours(&self, ruby: NodeId) -> (Option<NodeId>, Option<NodeId>) {
        let is_inline = self.nodes[ruby]
            .display_style()
            .is_some_and(|display| display.outside() == DisplayOutside::Inline);
        let (true, Some(parent)) = (is_inline, self.nodes[ruby].parent) else {
            return (None, None);
        };
        let siblings: Vec<NodeId> = self.nodes[parent]
            .children
            .iter()
            .copied()
            .filter(|&sibling| !matches!(self.nodes[sibling].data, NodeData::Comment { .. }))
            .collect();
        let Some(index) = siblings.iter().position(|&sibling| sibling == ruby) else {
            return (None, None);
        };
        let is_blank = |sibling: NodeId| match &self.nodes[sibling].data {
            NodeData::Text(text) => text.content.trim().is_empty(),
            _ => false,
        };
        let previous = index
            .checked_sub(1)
            .and_then(|before| siblings.get(before).copied())
            .filter(|&before| index > 1 || !is_blank(before));
        let next = siblings
            .get(index + 1)
            .copied()
            .filter(|&after| index + 2 < siblings.len() || !is_blank(after));
        (previous, next)
    }

    fn overhangs_into(&self, ruby: NodeId, neighbour: NodeId) -> bool {
        let node = &self.nodes[neighbour];
        let neighbour_size = match &node.data {
            NodeData::Text(text) if !text.content.is_empty() => {
                node.parent.and_then(|parent| self.font_size(parent))
            }
            NodeData::Element(_) if node.is_non_atomic_inline() => self.font_size(neighbour),
            _ => None,
        };
        neighbour_size
            .zip(self.font_size(ruby))
            .is_some_and(|(neighbour_size, ruby_size)| neighbour_size <= ruby_size)
    }

    fn edge_overhang(
        &self,
        ruby: NodeId,
        neighbour: Option<NodeId>,
        blank: Option<f32>,
        annotation: Option<&Placed>,
    ) -> f32 {
        let (Some(neighbour), Some(blank), Some(annotation)) = (neighbour, blank, annotation)
        else {
            return 0.0;
        };
        if !self.overhangs_into(ruby, neighbour) {
            return 0.0;
        }
        let scale = self.viewport.scale();
        self.font_size(annotation.measured.node)
            .map_or(0.0, |size| {
                ((size * scale / 2.0).floor() / scale).min(blank).max(0.0)
            })
    }

    pub(crate) fn compute_ruby_layout(
        &mut self,
        node_id: NodeId,
        inputs: LayoutInput,
    ) -> LayoutOutput {
        let children = self.nodes[node_id]
            .layout_children
            .borrow()
            .as_ref()
            .map(|children| children.to_vec())
            .unwrap_or_default();
        let columns = pair_columns(&children, |child| {
            self.nodes[child]
                .data
                .is_element_with_tag_name(&local_name!("rt"))
        });

        let mut bases = Vec::new();
        let mut annotations = Vec::new();
        let mut inline_end = 0.0_f32;
        for column in columns {
            let base_size = column.base.map(|base| self.ruby_child_size(base));
            let annotation_size = column
                .annotation
                .map(|annotation| self.ruby_child_size(annotation));
            let base_width = base_size.map_or(0.0, |size| size.width);
            let width = base_width.max(annotation_size.map_or(0.0, |size| size.width));
            let base = column
                .base
                .zip(base_size)
                .map(|(base, size)| self.measure_ruby_child(base, size));
            let annotation = column
                .annotation
                .zip(annotation_size)
                .map(|(annotation, size)| {
                    self.measure_ruby_child(annotation, Size { width, ..size })
                });
            bases.push(base.map(|measured| Placed {
                location: Point {
                    x: inline_end + (width - base_width) / 2.0,
                    y: 0.0,
                },
                measured,
            }));
            annotations.push(annotation.map(|measured| Placed {
                location: Point {
                    x: inline_end,
                    y: 0.0,
                },
                measured,
            }));
            inline_end += width;
        }

        let (previous, next) = self.ruby_neighbours(node_id);
        let start_overhang = self.edge_overhang(
            node_id,
            previous,
            bases
                .first()
                .and_then(Option::as_ref)
                .map(|base| base.location.x),
            annotations.first().and_then(Option::as_ref),
        );
        let end_overhang = self.edge_overhang(
            node_id,
            next,
            bases
                .last()
                .and_then(Option::as_ref)
                .map(|base| inline_end - base.location.x - base.measured.size.width),
            annotations.last().and_then(Option::as_ref),
        );
        let is_rtl = self.nodes[node_id]
            .primary_styles()
            .is_some_and(|style| style.clone_direction() == Direction::Rtl);
        let (left_overhang, right_overhang) = if is_rtl {
            for placed in bases.iter_mut().chain(annotations.iter_mut()).flatten() {
                placed.location.x = inline_end - placed.location.x - placed.measured.size.width;
            }
            (end_overhang, start_overhang)
        } else {
            (start_overhang, end_overhang)
        };
        let baseline = bases
            .iter()
            .flatten()
            .filter_map(|base| base.measured.baseline)
            .reduce(f32::max);
        let shared_baseline = baseline.unwrap_or(0.0);
        let mut em_tops = Vec::with_capacity(bases.len());
        for base in bases.iter_mut() {
            em_tops.push(base.as_mut().map(|base| {
                let own_baseline = base.measured.baseline.unwrap_or(base.measured.size.height);
                base.location.y = shared_baseline - own_baseline;
                let em_above_baseline = self
                    .em_above_baseline(base.measured.node)
                    .or_else(|| self.em_above_baseline(node_id))
                    .unwrap_or(own_baseline);
                base.location.y + own_baseline - em_above_baseline
            }));
        }
        let fallback_em_top = em_tops
            .iter()
            .flatten()
            .copied()
            .reduce(f32::min)
            .unwrap_or(0.0);
        let mut block_start = 0.0_f32;
        for (annotation, em_top) in annotations.iter_mut().zip(&em_tops) {
            if let Some(annotation) = annotation {
                let em_top = em_top.unwrap_or(fallback_em_top);
                annotation.location.y = em_top - annotation.measured.size.height;
                let font_size = self
                    .font_size(annotation.measured.node)
                    .unwrap_or(annotation.measured.size.height);
                block_start = block_start.min(em_top - font_size);
            }
        }

        let placed = || bases.iter().chain(&annotations).flatten();
        let block_start = bases
            .iter()
            .flatten()
            .map(|base| base.location.y)
            .fold(block_start, f32::min);
        let overflow_top = annotations
            .iter()
            .flatten()
            .map(|annotation| annotation.location.y - block_start)
            .fold(0.0_f32, f32::min);
        let block_end = placed()
            .map(|placed| placed.location.y + placed.measured.size.height)
            .fold(0.0_f32, f32::max);

        let parent_width = inputs.parent_size.width;
        let frame = {
            let style = self.nodes[node_id].layout_style();
            style
                .padding()
                .resolve_or_zero(parent_width, resolve_calc_value)
                + style
                    .border()
                    .resolve_or_zero(parent_width, resolve_calc_value)
        };
        let size = Size {
            width: inputs.known_dimensions.width.unwrap_or(
                inline_end + frame.horizontal_axis_sum() - left_overhang - right_overhang,
            ),
            height: inputs
                .known_dimensions
                .height
                .unwrap_or(block_end - block_start + frame.vertical_axis_sum()),
        };
        let offset = Point {
            x: frame.left - left_overhang,
            y: frame.top - block_start,
        };

        if inputs.run_mode == RunMode::PerformLayout {
            for (order, placed) in placed().enumerate() {
                let child = placed.measured.node;
                let mut layout = Layout::with_order(u32::try_from(order).unwrap_or(u32::MAX));
                layout.location = Point {
                    x: placed.location.x + offset.x,
                    y: placed.location.y + offset.y,
                };
                layout.size = placed.measured.size;
                let style = self.nodes[child].layout_style();
                layout.padding = style
                    .padding()
                    .resolve_or_zero(Some(size.width), resolve_calc_value);
                layout.border = style
                    .border()
                    .resolve_or_zero(Some(size.width), resolve_calc_value);
                drop(style);
                self.set_unrounded_layout(taffy_node_id(child), &layout);
            }
        }

        let baseline = baseline.map(|baseline| baseline + offset.y);
        LayoutOutput::from_sizes_and_baselines(
            size,
            ScrollableOverflow::new(
                Rect {
                    left: -left_overhang,
                    top: overflow_top,
                    right: size.width + right_overhang,
                    bottom: size.height,
                },
                Point { x: false, y: false },
            ),
            Baselines {
                first: baseline,
                last: baseline,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotations_pair_with_the_bases_before_them_by_index() {
        let children = [1, 2, 3, 4, 5, 6, 7].map(NodeId::from_u64);
        let annotations = [3, 4, 6].map(NodeId::from_u64);
        let columns = pair_columns(&children, |child| annotations.contains(&child));
        let pairs: Vec<_> = columns
            .iter()
            .map(|column| (column.base, column.annotation))
            .collect();
        let id = |raw: u64| Some(NodeId::from_u64(raw));
        assert_eq!(
            pairs,
            [
                (id(1), id(3)),
                (id(2), id(4)),
                (id(5), id(6)),
                (id(7), None)
            ]
        );
    }
}
