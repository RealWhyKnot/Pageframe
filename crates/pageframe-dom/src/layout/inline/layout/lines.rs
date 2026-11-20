use super::LayoutPassState;
use super::balance::{balanced_line_advances, balanced_line_box};
use crate::layout::resolve_calc_value;
use crate::node::TextLayout;
use crate::stylo_to_parley;
use crate::util::inline_clone_insets;
use pageframe_traits::node_id::NodeId;
use parley::{BreakReason, BreakerState, LineBreakData, YieldData};
use style::values::computed::TextWrapStyle;
use stylo_taffy::CopyStyleGetters;
use taffy::{
    BlockContext, BlockItemStyle as _, Clear, ContentSlot, CoreStyle as _, Float, LayoutInput,
    LayoutOutput, LayoutPartialTree as _, OofCandidates, ResolveOrZero as _, Size,
};

pub(super) struct LineLayoutOptions {
    pub(super) node_id: NodeId,
    pub(super) parent_size: Size<Option<f32>>,
    pub(super) float_child_inputs: LayoutInput,
    pub(super) container_pb: taffy::Rect<f32>,
    pub(super) max_advance: f32,
    pub(super) slack: f32,
    pub(super) alignment: parley::layout::Alignment,
}

struct PendingFloat {
    node_id: NodeId,
    output: LayoutOutput,
    margin: taffy::Rect<f32>,
    direction: taffy::FloatDirection,
    clear: Clear,
}

const FIT_TOLERANCE: f32 = 0.001;

const LINE_FIT_EPSILON: f32 = 1.0 / 64.0;

const CLONE_EDGE_PASSES: usize = 16;

const CLONE_FIT_SLACK: f32 = 0.01;

#[derive(Clone, Copy, Default)]
struct CloneEdge {
    start: f32,
    end: f32,
    width: f32,
    opening: f32,
    is_rtl: bool,
}

impl CloneEdge {
    fn add_closing(&mut self, inset: f32, span_is_rtl: bool) {
        if span_is_rtl == self.is_rtl {
            self.end += inset;
        } else {
            self.start += inset;
        }
    }

    fn add_opening(&mut self, inset: f32, span_is_rtl: bool) {
        self.opening += inset;
        if span_is_rtl == self.is_rtl {
            self.start += inset;
        } else {
            self.end += inset;
        }
    }
}

fn reserve_clone_edges(state: &mut BreakerState, edges: &[CloneEdge], line: usize) {
    let Some(edge) = edges.get(line) else {
        return;
    };
    let room = state.line_max_advance() - edge.start;
    let kept = (edge.width + CLONE_FIT_SLACK).min(state.line_max_advance() - edge.opening);
    let advance = (room - edge.end).max(kept).max(0.0);
    let shift = if edge.is_rtl {
        room - advance
    } else {
        edge.start
    };
    state.set_line_x(state.line_x() + shift);
    state.set_line_max_advance(advance);
}

fn indent_clone_line(state: &mut BreakerState, edges: &[CloneEdge], line: usize, max_advance: f32) {
    let opening = edges.get(line).map_or(0.0, |edge| edge.opening);
    state.set_line_x(opening);
    state.set_line_max_advance((max_advance - opening).max(0.0));
}

fn clone_line_edges(
    nodes: &crate::NodeTree,
    inline_layout: &mut crate::node::TextLayout,
    root: NodeId,
    max_advance: f32,
    scale: f32,
) -> Vec<CloneEdge> {
    let mut edges: Vec<CloneEdge> = Vec::new();
    if !inline_layout.has_clone_spans {
        return edges;
    }
    let basis = inline_layout.percent_basis;
    let layout = &mut inline_layout.layout;
    for _ in 0..CLONE_EDGE_PASSES {
        let mut breaker = layout.break_lines();
        let state = breaker.state_mut();
        state.set_layout_max_advance(max_advance);
        indent_clone_line(state, &edges, 0, max_advance);
        let mut line = 0;
        while let Some(yield_data) = breaker.break_next() {
            if let YieldData::LineBreak(_) = yield_data {
                line += 1;
                indent_clone_line(breaker.state_mut(), &edges, line, max_advance);
            }
        }
        breaker.finish();
        let next = continued_clone_edges(nodes, layout, root, basis, scale);
        let is_settled = next
            .iter()
            .enumerate()
            .all(|(line, edge)| edge.opening == edges.get(line).map_or(0.0, |old| old.opening));
        edges = next;
        if is_settled {
            break;
        }
    }
    edges
}

fn continued_clone_edges(
    nodes: &crate::NodeTree,
    layout: &parley::layout::Layout<crate::node::TextBrush>,
    root: NodeId,
    basis: f32,
    scale: f32,
) -> Vec<CloneEdge> {
    let ends: Vec<(Option<NodeId>, Option<NodeId>)> = layout
        .lines()
        .map(|line| {
            let mut first = None;
            let mut last = None;
            for item in line.items() {
                let (start, end, is_box, node) = match item {
                    parley::PositionedLayoutItem::GlyphRun(run) => {
                        let range = run.text_range();
                        (range.start, range.end, false, Some(run.style().brush.id))
                    }
                    parley::PositionedLayoutItem::InlineBox(inline_box) => {
                        let node = nodes[NodeId::from_u64(inline_box.id)].parent;
                        (inline_box.index, inline_box.index, true, node)
                    }
                };
                let first_key = (start, !is_box);
                let last_key = (end, is_box);
                if first.is_none_or(|(key, _)| first_key < key) {
                    first = Some((first_key, node));
                }
                if last.is_none_or(|(key, _)| last_key >= key) {
                    last = Some((last_key, node));
                }
            }
            (
                first.and_then(|(_, node)| node),
                last.and_then(|(_, node)| node),
            )
        })
        .collect();
    let mut edges: Vec<CloneEdge> = layout
        .lines()
        .map(|line| {
            let metrics = line.metrics();
            CloneEdge {
                width: metrics.advance - metrics.hanging_advance + metrics.indent,
                is_rtl: line.is_rtl(),
                ..CloneEdge::default()
            }
        })
        .collect();
    for line in 1..ends.len() {
        let (Some(before), Some(after)) = (ends[line - 1].1, ends[line].0) else {
            continue;
        };
        let mut current = Some(before);
        while let Some(id) = current.filter(|id| *id != root) {
            let node = &nodes[id];
            current = node.parent;
            let Some(insets) = inline_clone_insets(node, basis) else {
                continue;
            };
            let mut inner = Some(after);
            while let Some(inner_id) = inner.filter(|inner_id| *inner_id != root) {
                if inner_id == id {
                    edges[line - 1].add_closing(insets.outer_end() * scale, insets.is_rtl);
                    edges[line].add_opening(insets.outer_start() * scale, insets.is_rtl);
                    break;
                }
                inner = nodes[inner_id].parent;
            }
        }
    }
    edges
}

fn set_line_slot(state: &mut BreakerState, slot: &ContentSlot, scale: f32, slack: f32) {
    let max_advance = (slot.width * scale + slack).min(state.layout_max_advance());
    state.set_line_max_advance(max_advance);
    state.set_line_x(slot.x * scale);
    state.set_line_y((slot.y * scale) as f64);
}

fn retry_line_slot(
    block_ctx: &BlockContext,
    slot: &ContentSlot,
    line: &LineBreakData,
    wraps: bool,
    scale: f32,
) -> Option<ContentSlot> {
    let segment_id = slot.segment_id?;
    let line_y = (line.line_y_start / scale as f64) as f32;
    let line_height = ((line.line_y_end - line.line_y_start) / scale as f64) as f32;
    let advance = (line.metrics.advance - line.metrics.hanging_advance) / scale;

    if wraps && advance > 0.0 && advance > slot.width + FIT_TOLERANCE + LINE_FIT_EPSILON / scale {
        let next_slot =
            block_ctx.find_content_slot(line_y, line_height, Clear::None, Some(segment_id));
        if next_slot.segment_id.is_some() {
            return Some(next_slot);
        }
        return Some(block_ctx.find_content_slot(line_y, line_height, Clear::Both, None));
    }

    if line_height <= slot.height {
        return None;
    }
    let narrowed_slot = block_ctx.find_content_slot(line_y, line_height, Clear::None, None);
    let narrowed = narrowed_slot.x > slot.x + FIT_TOLERANCE
        || narrowed_slot.width < slot.width - FIT_TOLERANCE;
    narrowed.then_some(narrowed_slot)
}

impl LayoutPassState<'_> {
    fn place_inline_float(
        &mut self,
        block_ctx: &mut BlockContext,
        float: PendingFloat,
        min_y: f32,
        container_pb: taffy::Rect<f32>,
        oof_candidates: &mut OofCandidates,
    ) {
        let PendingFloat {
            node_id,
            mut output,
            margin,
            direction,
            clear,
        } = float;

        let pos = block_ctx.place_floated_box(
            crate::taffy_node_id(node_id).into(),
            output.size + margin.sum_axes(),
            min_y,
            direction,
            clear,
            false,
        );

        let location = taffy::Point {
            x: pos.x + margin.left + container_pb.left,
            y: pos.y + margin.top + container_pb.top,
        };
        let layout = self.nodes[node_id].unrounded_layout_mut();
        layout.size = output.size;
        layout.location = location;
        layout.margin = margin;

        if !output.oof_candidates.is_empty() {
            output.oof_candidates.translate(location);
            oof_candidates.append(&mut output.oof_candidates);
        }
    }
    pub(super) fn break_inline_lines(
        &mut self,
        inline_layout: &mut TextLayout,
        block_ctx: &mut BlockContext<'_>,
        oof_candidates: &mut OofCandidates,
        options: LineLayoutOptions,
    ) -> ([f64; 2], usize) {
        let LineLayoutOptions {
            node_id,
            parent_size,
            float_child_inputs,
            container_pb,
            max_advance,
            slack,
            alignment,
        } = options;
        let scale = self.viewport.scale();
        let mut line_bottoms = [0.0; 2];
        let mut line_count_so_far = 0;
        let wraps = self.nodes[node_id]
            .primary_styles()
            .is_none_or(|s| s.clone_text_wrap_mode() == stylo_to_parley::stylo::TextWrapMode::Wrap);
        let balances = wraps
            && !inline_layout.has_clone_spans
            && self.nodes[node_id]
                .primary_styles()
                .is_some_and(|s| s.slow_clone_text_wrap_style() == TextWrapStyle::Balance)
            && !inline_layout
                .layout
                .inline_boxes()
                .any(|item| item.kind == parley::InlineBoxKind::CustomOutOfFlow)
            && block_ctx
                .find_content_slot(0.0, 0.0, Clear::None, None)
                .segment_id
                .is_none();
        let balance = balances
            .then(|| balanced_line_advances(&mut inline_layout.layout, max_advance))
            .flatten();
        let mut balanced_advances = balance.as_ref().map(|(advances, _)| advances.as_slice());
        let clone_edges = clone_line_edges(&self.nodes, inline_layout, node_id, max_advance, scale);
        let is_rtl = balance.as_ref().is_some_and(|(_, is_rtl)| *is_rtl);
        let end_share = match alignment {
            parley::layout::Alignment::Center => 0.5,
            parley::layout::Alignment::Right => 1.0,
            parley::layout::Alignment::Left => 0.0,
            parley::layout::Alignment::End if is_rtl => 0.0,
            parley::layout::Alignment::End => 1.0,
            parley::layout::Alignment::Start | parley::layout::Alignment::Justify if is_rtl => 1.0,
            parley::layout::Alignment::Start | parley::layout::Alignment::Justify => 0.0,
        };
        let mut breaker = inline_layout.layout.break_lines();
        let mut line_slot = block_ctx.find_content_slot(0.0, 0.0, Clear::None, None);
        let mut has_active_floats = line_slot.segment_id.is_some();
        let mut pending_floats: Vec<PendingFloat> = Vec::new();
        let state = breaker.state_mut();
        state.set_layout_max_advance(max_advance);
        set_line_slot(state, &line_slot, scale, slack);
        if let Some((x, advance)) = balanced_line_box(balanced_advances, 0, max_advance, end_share)
        {
            state.set_line_x(x);
            state.set_line_max_advance(advance);
        }
        reserve_clone_edges(state, &clone_edges, 0);

        let mut line_start_state = has_active_floats.then(|| breaker.state().clone());

        while let Some(yield_data) = breaker.break_next() {
            match yield_data {
                YieldData::LineBreak(line_break_data) => {
                    if let Some(saved_state) = line_start_state.take()
                        && let Some(retry_slot) =
                            retry_line_slot(block_ctx, &line_slot, &line_break_data, wraps, scale)
                    {
                        pending_floats.clear();
                        line_slot = retry_slot;
                        has_active_floats = line_slot.segment_id.is_some();
                        breaker.revert_to(saved_state);
                        set_line_slot(breaker.state_mut(), &line_slot, scale, slack);
                        reserve_clone_edges(breaker.state_mut(), &clone_edges, line_count_so_far);
                        line_start_state = has_active_floats.then(|| breaker.state().clone());
                        continue;
                    }
                    line_bottoms[line_count_so_far % 2] = line_break_data.line_y_end;
                    line_count_so_far += 1;
                    if line_break_data.reason == BreakReason::Explicit {
                        balanced_advances = None;
                    }

                    let state = breaker.state_mut();

                    if !pending_floats.is_empty() {
                        let min_y = state.line_y() as f32 / scale;
                        for float in pending_floats.drain(..) {
                            self.place_inline_float(
                                block_ctx,
                                float,
                                min_y,
                                container_pb,
                                oof_candidates,
                            );
                        }
                        has_active_floats = true;
                    }

                    if has_active_floats {
                        let min_y = state.line_y() / scale as f64;
                        line_slot =
                            block_ctx.find_content_slot(min_y as f32, 0.0, Clear::None, None);
                        has_active_floats = line_slot.segment_id.is_some();
                        set_line_slot(state, &line_slot, scale, slack);
                    } else {
                        let (x, advance) = balanced_line_box(
                            balanced_advances,
                            line_count_so_far,
                            max_advance,
                            end_share,
                        )
                        .unwrap_or((0.0, max_advance));
                        state.set_line_x(x);
                        state.set_line_max_advance(advance);
                    }
                    reserve_clone_edges(state, &clone_edges, line_count_so_far);

                    line_start_state = has_active_floats.then(|| breaker.state().clone());
                    continue;
                }
                YieldData::MaxHeightExceeded(_data) => {
                    continue;
                }
                YieldData::InlineBoxBreak(box_break_data) => {
                    let state = breaker.state_mut();
                    let node_id = NodeId::from_u64(box_break_data.inline_box_id);

                    let (direction, clear, margin) = {
                        let style = self.child_layout_style(&self.nodes[node_id]);
                        let direction = match style.float() {
                            Float::Left => taffy::FloatDirection::Left,
                            Float::Right => taffy::FloatDirection::Right,
                            Float::None => unreachable!(),
                        };
                        (
                            direction,
                            style.clear(),
                            style
                                .margin()
                                .resolve_or_zero(parent_size, resolve_calc_value),
                        )
                    };

                    let output = self
                        .compute_child_layout(crate::taffy_node_id(node_id), float_child_inputs);
                    let float_width = (output.size.width + margin.left + margin.right) * scale;
                    let float = PendingFloat {
                        node_id,
                        output,
                        margin,
                        direction,
                        clear,
                    };

                    let opens_line = pending_floats.is_empty() && box_break_data.advance <= 0.0;
                    let fits_on_line = opens_line
                        || (pending_floats.is_empty()
                            && box_break_data.advance + float_width
                                <= state.line_max_advance() + 0.001 * scale);
                    if !fits_on_line {
                        pending_floats.push(float);
                    } else {
                        let min_y = state.line_y() as f32 / scale;
                        self.place_inline_float(
                            block_ctx,
                            float,
                            min_y,
                            container_pb,
                            oof_candidates,
                        );

                        let min_y = state.line_y() / scale as f64;
                        line_slot =
                            block_ctx.find_content_slot(min_y as f32, 0.0, Clear::None, None);
                        has_active_floats = line_slot.segment_id.is_some();
                        set_line_slot(state, &line_slot, scale, slack);

                        if !opens_line {
                            line_start_state = None;
                        }
                    }

                    state.append_inline_box_to_line(
                        box_break_data.advance,
                        f32::NEG_INFINITY,
                        f32::NEG_INFINITY,
                    );
                    if opens_line {
                        line_start_state = has_active_floats.then(|| breaker.state().clone());
                    }
                }
            }
        }
        breaker.finish();
        (line_bottoms, line_count_so_far)
    }
}
