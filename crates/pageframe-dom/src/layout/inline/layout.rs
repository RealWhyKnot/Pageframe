use pageframe_traits::node_id::NodeId;
use parley::{AlignmentOptions, BreakReason, IndentOptions};
use style::values::computed::BorderSideWidth;
#[cfg(feature = "floats")]
use style::values::computed::TextWrapStyle;
use style::values::specified::border::BorderStyle;
use style::values::specified::box_::{DisplayInside, DisplayOutside};
use style::values::{
    computed::{BaselineSource, CSSPixelLength, Contain},
    generics::text::GenericTextIndent,
};
use stylo_taffy::CopyStyleGetters;
use stylo_taffy::WritingModeExt;
use taffy::{
    AvailableSpace, AxisStaticPosition, BlockContainerStyle, BlockContext, BlockFormattingContext,
    BoxSizing, CollapsibleMarginSet, CompactLength, CoreStyle as _, Direction, LayoutInput,
    LayoutOutput, LayoutPartialTree as _, MaybeMath as _, MaybeResolve as _, OofCandidate,
    OofCandidates, OofItemStyle, OofPositioningArea, Overflow, Point, RequestedAxis,
    ResolveOrZero as _, RunMode, Size, SizingMode,
};

#[cfg(feature = "floats")]
use parley::{BreakerState, LineBreakData, YieldData};
#[cfg(feature = "floats")]
use taffy::{
    BlockItemStyle as _, Clear, ContentSlot, Float, FloatInsets, prelude::TaffyMaxContent,
};

use stylo_taffy::StyleFlags;

use crate::layout::LayoutPassState;
use crate::layout::resolve_calc_value;
use crate::stylo_to_parley;

struct SpanContainingBlock {
    id: NodeId,
    claims: taffy::ContainingBlockClaims,
    direction: Direction,
}

#[derive(Default)]
struct SpanPlacement {
    hoisted: Vec<taffy::NodeId>,
    fallback_flags: StyleFlags,
    overflow: Option<taffy::Rect<f32>>,
}

enum LastBaseline {
    At(f32),
    Missing,
    Unknown,
}

/// Subtract a child's margins from the definite axes of the available space it is laid out in.
/// Taffy's convention is that the parent subtracts a child's margins from the available space
/// before passing it to the child, and that the child does not subtract them again.
fn subtract_margins(mut child_inputs: LayoutInput, margin: taffy::Rect<f32>) -> LayoutInput {
    child_inputs.available_space = child_inputs
        .available_space
        .maybe_sub(margin.sum_axes())
        .maybe_max(Size::ZERO);
    child_inputs
}

/// Layout inputs for an atomic inline box, with any sizing keyword on its `width` style
/// (`min-content`, `max-content`, `fit-content`, `fit-content(...)`, `stretch`) resolved
/// into the available space or known width the box is measured with.
fn inline_box_inputs(
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

/// A float met mid-line, laid out but not yet placed in the block formatting context
#[cfg(feature = "floats")]
struct PendingFloat {
    node_id: NodeId,
    output: LayoutOutput,
    margin: taffy::Rect<f32>,
    direction: taffy::FloatDirection,
    clear: Clear,
}

/// Tolerance (in CSS px) for content fitting into a slot
#[cfg(feature = "floats")]
const FIT_TOLERANCE: f32 = 0.001;

#[cfg(feature = "floats")]
const LINE_FIT_EPSILON: f32 = 1.0 / 64.0;

#[cfg(feature = "floats")]
const BALANCE_MAX_LINES: usize = 6;

#[cfg(feature = "floats")]
const BALANCE_PRECISION: f32 = 1.0 / 64.0;

#[cfg(feature = "floats")]
const BALANCE_MAX_SEGMENTS: usize = 1024;

#[cfg(feature = "floats")]
struct BalanceSegment {
    advance: f32,
    hang: f32,
    text_end: usize,
}

#[cfg(feature = "floats")]
fn balanced_line_advances(
    layout: &mut parley::Layout<crate::node::TextBrush>,
    max_advance: f32,
) -> Option<Vec<f32>> {
    layout.break_all_lines(Some(max_advance));
    let has_forced_break = layout
        .lines()
        .any(|line| line.break_reason() == BreakReason::Explicit);
    let line_count = layout.len();
    if has_forced_break || !(2..=BALANCE_MAX_LINES).contains(&line_count) {
        return None;
    }
    let advances = evenest_line_advances(layout, max_advance, line_count).unwrap_or_else(|| {
        vec![narrowest_max_advance(layout, max_advance, line_count); line_count]
    });
    Some(advances)
}

#[cfg(feature = "floats")]
fn narrowest_max_advance(
    layout: &mut parley::Layout<crate::node::TextBrush>,
    max_advance: f32,
    line_count: usize,
) -> f32 {
    let mut narrow = layout.calculate_content_widths().min.min(max_advance);
    let mut wide = max_advance;
    while wide - narrow > BALANCE_PRECISION {
        let middle = (narrow + wide) / 2.0;
        layout.break_all_lines(Some(middle));
        if layout.len() <= line_count {
            wide = middle;
        } else {
            narrow = middle;
        }
    }
    wide
}

#[cfg(feature = "floats")]
fn evenest_line_advances(
    layout: &mut parley::Layout<crate::node::TextBrush>,
    max_advance: f32,
    line_count: usize,
) -> Option<Vec<f32>> {
    layout.break_all_lines(Some(0.0));
    let breaks_inside_words = layout
        .lines()
        .any(|line| line.break_reason() == BreakReason::Emergency);
    if breaks_inside_words || !(line_count..=BALANCE_MAX_SEGMENTS).contains(&layout.len()) {
        return None;
    }
    let indent = layout
        .lines()
        .next()
        .map_or(0.0, |line| line.metrics().indent);
    let segments: Vec<BalanceSegment> = layout
        .lines()
        .map(|line| BalanceSegment {
            advance: line.metrics().advance,
            hang: line.metrics().hanging_advance,
            text_end: line.text_range().end,
        })
        .collect();

    let line_ends = evenest_line_ends(&segments, indent, max_advance, line_count)?;
    let mut line_start = 0;
    let mut advances = Vec::with_capacity(line_count);
    for line_end in &line_ends {
        let width = balance_line_width(&segments[line_start..*line_end], line_start, indent);
        advances.push((width + BALANCE_PRECISION).min(max_advance));
        line_start = *line_end;
    }

    let mut breaker = layout.break_lines();
    let state = breaker.state_mut();
    state.set_layout_max_advance(max_advance);
    state.set_line_max_advance(*advances.first()?);
    let mut next_line = 1;
    while let Some(yield_data) = breaker.break_next() {
        if let YieldData::LineBreak(_) = yield_data {
            let advance = advances.get(next_line).copied().unwrap_or(max_advance);
            breaker.state_mut().set_line_max_advance(advance);
            next_line += 1;
        }
    }
    breaker.finish();

    let breaks_as_planned = layout.len() == line_count
        && layout
            .lines()
            .zip(&line_ends)
            .all(|(line, line_end)| line.text_range().end == segments[*line_end - 1].text_end);
    breaks_as_planned.then_some(advances)
}

#[cfg(feature = "floats")]
const CLONE_EDGE_PASSES: usize = 16;

#[cfg(feature = "floats")]
const CLONE_FIT_SLACK: f32 = 0.01;

#[cfg(feature = "floats")]
#[derive(Clone, Copy, Default)]
struct CloneEdge {
    start: f32,
    end: f32,
    width: f32,
}

#[cfg(feature = "floats")]
fn reserve_clone_edges(state: &mut BreakerState, edges: &[CloneEdge], line: usize, is_rtl: bool) {
    let Some(edge) = edges.get(line) else {
        return;
    };
    let room = state.line_max_advance() - edge.start;
    let kept = (edge.width + CLONE_FIT_SLACK).min(room);
    let advance = (room - edge.end).max(kept).max(0.0);
    let shift = if is_rtl { room - advance } else { edge.start };
    state.set_line_x(state.line_x() + shift);
    state.set_line_max_advance(advance);
}

#[cfg(feature = "floats")]
fn indent_clone_line(state: &mut BreakerState, edges: &[CloneEdge], line: usize, max_advance: f32) {
    let start = edges.get(line).map_or(0.0, |edge| edge.start);
    state.set_line_x(start);
    state.set_line_max_advance((max_advance - start).max(0.0));
}

#[cfg(feature = "floats")]
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
            .all(|(line, edge)| edge.start == edges.get(line).map_or(0.0, |old| old.start));
        edges = next;
        if is_settled {
            break;
        }
    }
    edges
}

#[cfg(feature = "floats")]
fn continued_clone_edges(
    nodes: &crate::NodeTree,
    layout: &parley::layout::Layout<crate::node::TextBrush>,
    root: NodeId,
    basis: f32,
    scale: f32,
) -> Vec<CloneEdge> {
    let item_node = |item: parley::PositionedLayoutItem<'_, crate::node::TextBrush>| match item {
        parley::PositionedLayoutItem::GlyphRun(run) => Some(run.style().brush.id),
        parley::PositionedLayoutItem::InlineBox(inline_box) => {
            nodes[NodeId::from_u64(inline_box.id)].parent
        }
    };
    let ends: Vec<(Option<NodeId>, Option<NodeId>)> = layout
        .lines()
        .map(|line| {
            let first = line.items().next().and_then(item_node);
            let last = line.items().last().and_then(item_node);
            (first, last)
        })
        .collect();
    let mut edges: Vec<CloneEdge> = layout
        .lines()
        .map(|line| {
            let metrics = line.metrics();
            CloneEdge {
                width: metrics.advance - metrics.hanging_advance + metrics.indent,
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
            let Some(insets) = crate::util::inline_clone_insets(node, basis) else {
                continue;
            };
            let mut inner = Some(after);
            while let Some(inner_id) = inner.filter(|inner_id| *inner_id != root) {
                if inner_id == id {
                    edges[line - 1].end += insets.outer_end() * scale;
                    edges[line].start += insets.outer_start() * scale;
                    break;
                }
                inner = nodes[inner_id].parent;
            }
        }
    }
    edges
}

#[cfg(feature = "floats")]
fn balance_line_width(segments: &[BalanceSegment], line_start: usize, indent: f32) -> f32 {
    let lead = if line_start == 0 { indent } else { 0.0 };
    let advance: f32 = segments.iter().map(|segment| segment.advance).sum();
    let hang = segments.last().map_or(0.0, |segment| segment.hang);
    lead + advance - hang
}

#[cfg(feature = "floats")]
fn evenest_line_ends(
    segments: &[BalanceSegment],
    indent: f32,
    max_advance: f32,
    line_count: usize,
) -> Option<Vec<usize>> {
    let count = segments.len();
    let mut costs = vec![vec![f32::INFINITY; count + 1]; line_count + 1];
    let mut starts = vec![vec![0usize; count + 1]; line_count + 1];
    costs[0][0] = 0.0;
    for line in 1..=line_count {
        for line_end in line..=count {
            for line_start in (line - 1..line_end).rev() {
                let width = balance_line_width(&segments[line_start..line_end], line_start, indent);
                if width > max_advance + BALANCE_PRECISION && line_end - line_start > 1 {
                    break;
                }
                let cost = costs[line - 1][line_start] + (max_advance - width).powi(2);
                if cost < costs[line][line_end] {
                    costs[line][line_end] = cost;
                    starts[line][line_end] = line_start;
                }
            }
        }
    }
    if !costs[line_count][count].is_finite() {
        return None;
    }
    let mut line_ends = vec![0; line_count];
    let mut line_end = count;
    for line in (1..=line_count).rev() {
        line_ends[line - 1] = line_end;
        line_end = starts[line][line_end];
    }
    Some(line_ends)
}

#[cfg(feature = "floats")]
fn balanced_line_box(
    balanced_advances: Option<&[f32]>,
    line_index: usize,
    max_advance: f32,
    end_share: f32,
) -> Option<(f32, f32)> {
    let advance = *balanced_advances?.get(line_index)?;
    Some(((max_advance - advance).max(0.0) * end_share, advance))
}

/// Lay out the next line into `slot` (content-box relative, in CSS px)
#[cfg(feature = "floats")]
fn set_line_slot(state: &mut BreakerState, slot: &ContentSlot, scale: f32, slack: f32) {
    // With huge insets, f32 rounding can make the slot slightly wider than the layout
    let max_advance = (slot.width * scale + slack).min(state.layout_max_advance());
    state.set_line_max_advance(max_advance);
    state.set_line_x(slot.x * scale);
    state.set_line_y((slot.y * scale) as f64);
}

/// The slot to retry a just-broken line in if it doesn't fit the slot it was laid out into
/// (CSS 2.1 §9.5): a line taller than its slot is retried in the slot narrowed by the floats
/// beside its full height, and a line whose content overflows its slot is moved down to the
/// next slot (or below the floats if there is none). Like browsers, content that can't wrap
/// (`text-wrap-mode: nowrap`) is left to overflow its slot instead of being moved down.
#[cfg(feature = "floats")]
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
    /// Place a float laid out by inline layout at or below `min_y` (content-box relative)
    #[cfg(feature = "floats")]
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

        // Note: `pos` is content-box relative
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

        // Translate anchors from item-relative to container-relative
        // coordinates and collect candidates bubbled from the float's subtree
        if !output.oof_candidates.is_empty() {
            output.oof_candidates.translate(location);
            oof_candidates.append(&mut output.oof_candidates);
        }
    }

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

        // Pull these out earlier to avoid borrowing issues
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

        // Resolve node's preferred/min/max sizes (width/heights) against the available space (percentages resolve to pixel values)
        // For ContentSize mode, we pretend that the node has no size styles as these should be ignored.
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

        // If both min and max in a given axis are set and max <= min then this determines the size in that axis
        let min_max_definite_size = min_size.zip_map(max_size, |min, max| match (min, max) {
            (Some(min), Some(max)) if max <= min => Some(min),
            _ => None,
        });

        let styled_based_known_dimensions = known_dimensions
            .or(min_max_definite_size)
            .or(clamped_style_size)
            .maybe_max(padding_border_size);

        // Short-circuit layout if the container's size is fully determined by the container's size and the run mode
        // is ComputeSize (and thus the container's size is all that we're interested in)
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

        // Unwrap the block formatting context if one was passed, or else create a new one
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

    fn inline_span_containing_block(
        &self,
        root_id: NodeId,
        box_id: NodeId,
        position: taffy::Position,
    ) -> Option<SpanContainingBlock> {
        let root = &self.nodes[root_id];
        let root_parent = root.is_anonymous().then_some(root.parent).flatten();
        let mut current = self.nodes[box_id].parent;
        while let Some(id) = current.filter(|id| *id != root_id && Some(*id) != root_parent) {
            let ancestor = &self.nodes[id];
            let found = ancestor
                .primary_styles()
                .filter(|style| {
                    let display = style.clone_display();
                    display.outside() == DisplayOutside::Inline
                        && display.inside() == DisplayInside::Flow
                })
                .map(|style| SpanContainingBlock {
                    id,
                    claims: stylo_taffy::convert::inline_containing_block_claims(&style),
                    direction: if style.clone_direction()
                        == style::computed_values::direction::T::Rtl
                    {
                        Direction::Rtl
                    } else {
                        Direction::Ltr
                    },
                })
                .filter(|span| span.claims.for_position(position));
            if found.is_some() {
                return found;
            }
            current = ancestor.parent;
        }
        None
    }

    fn span_padding_box(&self, span: &SpanContainingBlock) -> Option<OofPositioningArea> {
        let node = &self.nodes[span.id];
        let style = node.primary_styles()?;
        let fragments: Vec<taffy::Rect<f32>> = node
            .inline_fragment_boxes(self.primary_font_extent(node))?
            .collect();
        let (first, last) = (fragments.first()?, fragments.last()?);
        let border = style.get_border();
        let side = |width: &BorderSideWidth, line: BorderStyle| {
            if line.none_or_hidden() {
                0.0
            } else {
                width.0.to_f32_px()
            }
        };
        let is_rtl = span.direction == Direction::Rtl;
        let left = if is_rtl { last.left } else { first.left }
            + side(&border.border_left_width, border.border_left_style);
        let right = if is_rtl { first.right } else { last.right }
            - side(&border.border_right_width, border.border_right_style);
        let top = first.top + side(&border.border_top_width, border.border_top_style);
        let bottom = last.bottom - side(&border.border_bottom_width, border.border_bottom_style);
        Some(OofPositioningArea {
            size: Size {
                width: (right - left).max(0.0),
                height: (bottom - top).max(0.0),
            },
            offset: Point { x: left, y: top },
        })
    }

    fn place_span_contained_boxes(
        &mut self,
        root_id: NodeId,
        candidates: &mut OofCandidates,
        final_size: Size<f32>,
        padding: taffy::Rect<f32>,
        border: taffy::Rect<f32>,
    ) -> SpanPlacement {
        let mut by_span: Vec<(SpanContainingBlock, OofCandidates)> = Vec::new();
        let mut rest = OofCandidates::new();
        for candidate in candidates.iter() {
            let span = self.inline_span_containing_block(
                root_id,
                crate::dom_node_id(candidate.node),
                candidate.position,
            );
            match span {
                Some(span) => match by_span.iter_mut().find(|(known, _)| known.id == span.id) {
                    Some((_, list)) => list.push(*candidate),
                    None => {
                        let mut list = OofCandidates::new();
                        list.push(*candidate);
                        by_span.push((span, list));
                    }
                },
                None => rest.push(*candidate),
            }
        }
        let mut placement = SpanPlacement::default();
        let is_horizontal = self.nodes[root_id]
            .primary_styles()
            .is_some_and(|style| style.writing_mode.is_horizontal());
        if !is_horizontal {
            for (span, _) in &by_span {
                if span.claims.absolute {
                    placement.fallback_flags |= StyleFlags::SPAN_ABSOLUTE_CB;
                }
                if span.claims.fixed {
                    placement.fallback_flags |= StyleFlags::SPAN_FIXED_CB;
                }
            }
            return placement;
        }
        if by_span.is_empty() {
            return placement;
        }
        *candidates = rest;
        let root_layout = self.nodes[root_id].unrounded_layout_mut();
        root_layout.size = final_size;
        root_layout.padding = padding;
        root_layout.border = border;
        let root_area = OofPositioningArea {
            size: Size {
                width: (final_size.width - border.left - border.right).max(0.0),
                height: (final_size.height - border.top - border.bottom).max(0.0),
            },
            offset: Point {
                x: border.left,
                y: border.top,
            },
        };
        for (span, mut list) in by_span {
            for candidate in list.as_mut_slice() {
                let node = &self.nodes[crate::dom_node_id(candidate.node)];
                if node.layout_parent.get() != Some(root_id) {
                    continue;
                }
                let (across, down) = node.parent.map_or((0.0, 0.0), |parent| {
                    self.nodes[parent].inline_relative_shift()
                });
                let axes = &mut candidate.static_position;
                axes.x.area.start += across;
                axes.x.area.end += across;
                axes.y.area.start += down;
                axes.y.area.end += down;
            }
            let area = self.span_padding_box(&span).unwrap_or(root_area);
            let mut result = taffy::compute_oof_layout_for_area(
                self,
                crate::taffy_node_id(root_id),
                list.as_slice(),
                area,
                span.direction,
                span.claims,
            );
            candidates.append(&mut result.unclaimed);
            placement.hoisted.extend(result.hoisted);
            let overflow = result.scrollable_overflow_rect;
            let shift = Point {
                x: area.offset.x - border.left,
                y: area.offset.y - border.top,
            };
            let moved = taffy::Rect {
                left: overflow.left + shift.x,
                right: overflow.right + shift.x,
                top: overflow.top + shift.y,
                bottom: overflow.bottom + shift.y,
            };
            placement.overflow = Some(match placement.overflow {
                Some(known) => taffy::Rect {
                    left: known.left.min(moved.left),
                    right: known.right.max(moved.right),
                    top: known.top.min(moved.top),
                    bottom: known.bottom.max(moved.bottom),
                },
                None => moved,
            });
        }
        placement
    }

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

        // Take inline layout to satisfy borrow checker
        let mut inline_layout = self.nodes[node_id]
            .data
            .downcast_element_mut()
            .unwrap()
            .take_inline_layout()
            .unwrap();

        let style = self.nodes[node_id].layout_style();

        // Note: both horizontal and vertical percentage padding/borders are resolved against the container's inline size (i.e. width).
        // CSS specifies this: https://developer.mozilla.org/en-US/docs/Web/CSS/padding#values
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

        // Scrollbar gutters are reserved when the `overflow` property is set to `Overflow::Scroll`.
        // However, the axis are switched (transposed) because a node that scrolls vertically needs
        // *horizontal* space to be reserved for a scrollbar
        let scrollbar_gutter = style.overflow().transpose().map(|overflow| match overflow {
            Overflow::Scroll => style.scrollbar_width(),
            _ => 0.0,
        });
        // TODO: make side configurable based on the `direction` property
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
        // || matches!(node_size.height, Some(h) if h > 0.0)
        // || matches!(node_min_size.height, Some(h) if h > 0.0)
        // || !inline_layout.text.is_empty();
        // || inline_layout.layout.inline_boxes().len() > 0;

        // Resolve node's preferred/min/max sizes (width/heights) against the available space (percentages resolve to pixel values)
        // For ContentSize mode, we pretend that the node has no size styles as these should be ignored.
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

        // Short circuit if inline context contains no text or inline boxes
        if !has_styles_preventing_being_collapsed_through
            && !inline_layout.has_text()
            && inline_layout.layout.inline_boxes().len() == 0
        {
            // Put layout back
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

        // Compute available space
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

        // Compute size of inline boxes
        let child_inputs = taffy::tree::LayoutInput {
            known_dimensions: Size::NONE,
            available_space,
            sizing_mode: SizingMode::InherentSize,
            parent_size: Size {
                width: available_space.width.into_option(),
                // Anonymous blocks do not establish the containing block for percentages.
                height: if self.nodes[node_id].is_anonymous() {
                    parent_size.height
                } else {
                    available_space.height.into_option()
                },
            },
            // Atomic inlines (e.g. inline-block) establish independent formatting
            // contexts: their margins never collapse with their children's margins.
            vertical_margins_are_collapsible: taffy::Line::FALSE,
            ..inputs
        };
        #[cfg(feature = "floats")]
        let float_child_inputs = taffy::tree::LayoutInput {
            available_space: Size::MAX_CONTENT,
            ..child_inputs
        };

        #[cfg(feature = "floats")]
        let mut has_floated_box = false;

        // Update inline boxes
        for ibox in inline_layout.layout.inline_boxes_mut() {
            if ibox.kind == parley::InlineBoxKind::EmptySpan {
                continue;
            }
            let style = self.child_layout_style(&self.nodes[NodeId::from_u64(ibox.id)]);
            let margin = style
                .margin()
                .resolve_or_zero(inputs.parent_size, resolve_calc_value);

            #[cfg(feature = "floats")]
            let is_floated = style.float().is_floated();
            #[cfg(not(feature = "floats"))]
            let is_floated = false;
            #[cfg(feature = "floats")]
            {
                has_floated_box |= is_floated;
            }

            let is_out_of_flow = style.position().is_out_of_flow();
            // `baseline-source: auto` selects the last baseline for inline-blocks and the first
            // for other atomic inlines. A block-axis scroll-container inline-block (except buttons)
            // instead uses its bottom margin edge, but only with `auto` (css-align-3 §9.1).
            // `overflow: clip` is not a scroll container. Exported scroll-container baselines
            // are clamped to the border box.
            // A layout-contained box is treated as having no baseline (css-contain-1 §3.3).
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
                // Vertical margins adjust the space the box reserves in the line. A box with a
                // baseline splits that space into ascent (`margin.top + baseline`) and descent
                // (`margin.bottom + height - baseline`), either of which may be negative. A box
                // without a baseline sits on the baseline and cannot reserve negative space.
                // Kept finite: huge author lengths can sum to infinity, which is taller than
                // the line breaker's `f32::MAX` height limit, and it then yields
                // `MaxHeightExceeded` for this box forever without advancing.
                let margin_box_height = margin.top + margin.bottom + output.size.height;
                ibox.height = if ibox.baseline.is_some() {
                    (margin_box_height * scale).min(f32::MAX)
                } else {
                    (margin_box_height.max(0.0) * scale).min(f32::MAX)
                };
            }
        }

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
        // Percentage indents do not contribute to intrinsic widths.
        inline_layout.layout.set_text_indent(
            text_indent.length.resolve(CSSPixelLength::new(0.0)).px() * scale,
            indent_options,
        );

        let pbw = container_pb.horizontal_components().sum() * scale;
        let width = known_dimensions
            .width
            .map(|w| (w * scale) - pbw)
            .unwrap_or_else(|| {
                // TODO: Cache content widths.
                //
                // This is a little tricky as the size of the inline boxes may depend on whether we are sizing under
                // and a min-content or max-content constraint. So if we want to compute both widths in one pass then
                // we need to store both a min-content and max-content size on each box.
                let content_sizes = inline_layout.layout.calculate_content_widths();
                let min_content_width = content_sizes.min;
                let max_content_width = content_sizes.max;

                #[cfg(feature = "floats")]
                let float_width = match available_space.width {
                    AvailableSpace::MinContent => {
                        let mut width: f32 = 0.0;
                        for ibox in inline_layout.layout.inline_boxes_mut() {
                            let (is_floated, margin) = {
                                let style =
                                    self.child_layout_style(&self.nodes[NodeId::from_u64(ibox.id)]);
                                (
                                    style.float().is_floated(),
                                    style
                                        .margin()
                                        .resolve_or_zero(inputs.parent_size, resolve_calc_value),
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
                    // A definite available width only occurs here when shrink-to-fit sizing,
                    // which needs the max-content contribution of the floats.
                    AvailableSpace::MaxContent | AvailableSpace::Definite(_) => {
                        // Max-content width is unbounded, and floats only drop to a new band when
                        // `clear` pushes them there. Widths add up within a band; bands stack
                        // vertically and the widest one wins.
                        let mut left_band: f32 = 0.0;
                        let mut right_band: f32 = 0.0;
                        let mut width: f32 = 0.0;
                        for ibox in inline_layout.layout.inline_boxes_mut() {
                            let (float, clear, margin) = {
                                let style =
                                    self.child_layout_style(&self.nodes[NodeId::from_u64(ibox.id)]);
                                (
                                    style.float(),
                                    style.clear(),
                                    style
                                        .margin()
                                        .resolve_or_zero(inputs.parent_size, resolve_calc_value),
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
                };

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
        let _ = block_ctx; // Suppress unused variable warning

        // Set block context width if this is a block context root
        #[cfg(feature = "floats")]
        let is_bfc_root = block_ctx.is_bfc_root();
        #[cfg(feature = "floats")]
        if is_bfc_root {
            block_ctx.set_width((width + pbw) / scale);
        }

        // Create sub-context to account for the inline layout's padding/border
        #[cfg(feature = "floats")]
        let outer_block_ctx = block_ctx;
        #[cfg(feature = "floats")]
        let mut block_ctx = outer_block_ctx.sub_context(
            container_pb.top,
            FloatInsets::new(container_pb.left, container_pb.right),
        );
        // block_ctx.apply_content_box_inset([container_pb.left, container_pb.right]);

        if inputs.run_mode == taffy::RunMode::ComputeSize
            && inputs.axis == RequestedAxis::Horizontal
        {
            // Put layout back
            self.nodes[node_id]
                .data
                .downcast_element_mut()
                .unwrap()
                .inline_layout_data = Some(inline_layout);

            let measured_size = inputs.known_dimensions.unwrap_or(taffy::Size {
                width: width / scale,
                // Height is ignored if RequestedAxis if Horizontal
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

        // Content sized to fit exactly (e.g. shrink-to-fit) can end up a few ulps wider than
        // `width`: the width goes through round trips in CSS px, and the line breaker sums
        // advances in a different order to `calculate_content_widths`. Give the line breaker a
        // few ulps of slack in that case so that the content doesn't wrap.
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

        // Out-of-flow candidates bubbled up from this container and its in-flow subtree.
        // These are laid out by the out-of-flow positioning pass (`compute_oof_layout`).
        let mut oof_candidates = OofCandidates::new();

        // The bottoms of the last two line boxes (indexed by line index % 2), including any
        // space skipped to move them down past floats. Only the last counted line is read, which
        // is at most one line before the last.
        #[cfg(feature = "floats")]
        let mut line_bottoms = [0.0f64; 2];
        #[cfg(feature = "floats")]
        let mut line_count_so_far = 0usize;

        let (alignment, last_line_alignment) = self.nodes[node_id]
            .primary_styles()
            .map(|s| {
                (
                    stylo_to_parley::text_align(s.clone_text_align()),
                    stylo_to_parley::text_align_last(s.clone_text_align_last()),
                )
            })
            .unwrap_or((parley::layout::Alignment::Start, None));

        // Perform inline layout
        #[cfg(feature = "floats")]
        {
            let wraps = self.nodes[node_id].primary_styles().is_none_or(|s| {
                s.clone_text_wrap_mode() == stylo_to_parley::stylo::TextWrapMode::Wrap
            });
            let balances = wraps
                && !has_floated_box
                && !inline_layout.has_clone_spans
                && self.nodes[node_id]
                    .primary_styles()
                    .is_some_and(|s| s.slow_clone_text_wrap_style() == TextWrapStyle::Balance)
                && block_ctx
                    .find_content_slot(0.0, 0.0, Clear::None, None)
                    .segment_id
                    .is_none();
            let balanced_advances = balances
                .then(|| balanced_line_advances(&mut inline_layout.layout, max_advance))
                .flatten();
            let clone_edges =
                clone_line_edges(&self.nodes, &mut inline_layout, node_id, max_advance, scale);
            let is_rtl = inline_layout.layout.is_rtl();
            let end_share = match alignment {
                parley::layout::Alignment::Center => 0.5,
                parley::layout::Alignment::Right => 1.0,
                parley::layout::Alignment::Left => 0.0,
                parley::layout::Alignment::End if is_rtl => 0.0,
                parley::layout::Alignment::End => 1.0,
                parley::layout::Alignment::Start | parley::layout::Alignment::Justify if is_rtl => {
                    1.0
                }
                parley::layout::Alignment::Start | parley::layout::Alignment::Justify => 0.0,
            };
            let mut breaker = inline_layout.layout.break_lines();
            let mut line_slot = block_ctx.find_content_slot(0.0, 0.0, Clear::None, None);
            let mut has_active_floats = line_slot.segment_id.is_some();
            let mut pending_floats: Vec<PendingFloat> = Vec::new();
            let state = breaker.state_mut();
            state.set_layout_max_advance(max_advance);
            set_line_slot(state, &line_slot, scale, slack);
            if let Some((x, advance)) =
                balanced_line_box(balanced_advances.as_deref(), 0, max_advance, end_share)
            {
                state.set_line_x(x);
                state.set_line_max_advance(advance);
            }
            reserve_clone_edges(state, &clone_edges, 0, is_rtl);

            // The state at the start of the current line, which is reverted to when the line
            // needs to be retried lower down (or narrower) to avoid floats.
            // `None` if the line can't be retried (no floats beside it, or a float was placed
            // part way through it).
            let mut line_start_state = has_active_floats.then(|| breaker.state().clone());

            while let Some(yield_data) = breaker.break_next() {
                match yield_data {
                    YieldData::LineBreak(line_break_data) => {
                        if let Some(saved_state) = line_start_state.take()
                            && let Some(retry_slot) = retry_line_slot(
                                &block_ctx,
                                &line_slot,
                                &line_break_data,
                                wraps,
                                scale,
                            )
                        {
                            // Floats queued during this line attempt will be encountered again
                            pending_floats.clear();
                            line_slot = retry_slot;
                            has_active_floats = line_slot.segment_id.is_some();
                            breaker.revert_to(saved_state);
                            set_line_slot(breaker.state_mut(), &line_slot, scale, slack);
                            reserve_clone_edges(
                                breaker.state_mut(),
                                &clone_edges,
                                line_count_so_far,
                                is_rtl,
                            );
                            line_start_state = has_active_floats.then(|| breaker.state().clone());
                            continue;
                        }
                        line_bottoms[line_count_so_far % 2] = line_break_data.line_y_end;
                        line_count_so_far += 1;

                        let state = breaker.state_mut();

                        if !pending_floats.is_empty() {
                            let min_y = state.line_y() as f32 / scale;
                            for float in pending_floats.drain(..) {
                                self.place_inline_float(
                                    &mut block_ctx,
                                    float,
                                    min_y,
                                    container_pb,
                                    &mut oof_candidates,
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
                                balanced_advances.as_deref(),
                                line_count_so_far,
                                max_advance,
                                end_share,
                            )
                            .unwrap_or((0.0, max_advance));
                            state.set_line_x(x);
                            state.set_line_max_advance(advance);
                        }
                        reserve_clone_edges(state, &clone_edges, line_count_so_far, is_rtl);

                        line_start_state = has_active_floats.then(|| breaker.state().clone());
                        continue;
                    }
                    YieldData::MaxHeightExceeded(_data) => {
                        // TODO
                        continue;
                    }
                    YieldData::InlineBoxBreak(box_break_data) => {
                        let state = breaker.state_mut();
                        let node_id = NodeId::from_u64(box_break_data.inline_box_id);

                        let (direction, clear, margin) = {
                            let style = self.child_layout_style(&self.nodes[node_id]);
                            // We can assume that the box is a float because we only set `break_on_box: true` for floats
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
                                    .resolve_or_zero(inputs.parent_size, resolve_calc_value),
                            )
                        };

                        let output = self.compute_child_layout(
                            crate::taffy_node_id(node_id),
                            float_child_inputs,
                        );
                        let float_width = (output.size.width + margin.left + margin.right) * scale;
                        let float = PendingFloat {
                            node_id,
                            output,
                            margin,
                            direction,
                            clear,
                        };

                        // A float that doesn't fit beside the content already on the line is
                        // placed below the line (CSS 2.1 §9.5.1), as are any floats after it
                        // (a float may not be higher than an earlier float).
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
                                &mut block_ctx,
                                float,
                                min_y,
                                container_pb,
                                &mut oof_candidates,
                            );

                            let min_y = state.line_y() / scale as f64;
                            line_slot =
                                block_ctx.find_content_slot(min_y as f32, 0.0, Clear::None, None);
                            has_active_floats = line_slot.segment_id.is_some();
                            set_line_slot(state, &line_slot, scale, slack);

                            if !opens_line {
                                // The float is already placed and the line can't be retried.
                                line_start_state = None;
                            }
                        }

                        // Floats are out-of-flow and must not contribute to the line's height.
                        state.append_inline_box_to_line(
                            box_break_data.advance,
                            f32::NEG_INFINITY,
                            f32::NEG_INFINITY,
                        );
                        if opens_line {
                            line_start_state = has_active_floats.then(|| breaker.state().clone());
                        }

                        // if float.is_floated() {
                        //     println!("INLINE FLOATED BOX ({}) {:?}", ibox.id, float);
                        //     println!(
                        //         "w:{} h:{} x:{}, y:{}",
                        //         layout.size.width, layout.size.height, 0, 0
                        //     );
                        // }
                    }
                }
            }
            breaker.finish();
        }

        // Propagate the height consumed by floats placed within this container to the
        // enclosing block context (so that the BFC root can contain them)
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

        // Parley lays out empty text as a single strut-height line (text-editor semantics),
        // but a line box containing no text, inline boxes or other in-flow content is a
        // zero-height line box in CSS (CSS2 §9.4.2).
        let has_inline_content =
            inline_layout.has_text() || inline_layout.layout.inline_boxes().len() > 0;

        let mut height = if has_inline_content {
            inline_layout.layout.height()
        } else {
            0.0
        };

        // A forced line break (e.g. `<br>` or a preserved newline) at the end of the inline
        // content ends the final line box without starting a new one. Parley still emits an
        // empty line after it (so that editors have a line to place the cursor on), so that
        // line is excluded from the measured height (and from the last baseline).
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

        // Parley's height is the sum of the line heights, which misses the space skipped when a
        // line is moved down past floats, so extend it to the bottom of the last counted line
        // (Parley doesn't count an empty last line either).
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

        // `line-clamp` cuts the block off after its last visible line.
        if let Some(max_lines) = self.nodes[node_id]
            .primary_styles()
            .and_then(|style| crate::util::clamped_line_count(&style, &inline_layout.layout))
            && let Some(last_line) = inline_layout.layout.get(max_lines - 1)
        {
            height = height.min(last_line.metrics().block_max_coord);
        }

        // Note: `width` and `height` are content-box measurements of the inline content.
        // `known_dimensions` must not be substituted in here: those are border-box sizes, and
        // using them for the scrollable overflow rect (which adds padding below) would
        // double-count padding, incorrectly making the container's content overflow it.
        let measured_size = taffy::Size {
            width: width / scale,
            height: height / scale,
        };

        // Unbreakable content (e.g. `white-space: pre` text) may overflow the max advance the
        // lines were broken at. `width` is the advance the lines were broken at, so the actual
        // extent of the line boxes must be taken from the laid-out lines for `content_size`.
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

        // `align-content` aligns the line boxes (as a single unit) within the content box in
        // the block axis. Floats are positioned relative to the formatting context and are not
        // moved. The offset is stored on the text layout for painting and hit-testing.
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

        // Store sizes and positions of inline boxes. Walking a line's items builds every
        // glyph run on it, so lines are only walked when there are boxes to find.
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
                    // Inline formatting contexts have no `justify-items`/`align-items` for an
                    // `auto` self-alignment to defer to, so it behaves as `normal`.
                    let justify_self =
                        OofItemStyle::justify_self(&style).unwrap_or(taffy::AlignItems::NORMAL);
                    let align_self =
                        OofItemStyle::align_self(&style).unwrap_or(taffy::AlignItems::NORMAL);

                    // The static position of an absolutely positioned box depends on the
                    // display its hypothetical box would have had (the display specified
                    // before position:absolute blockified it): inline-level boxes sit at
                    // their position within the line, while block-level boxes start at the
                    // content-box left edge of their containing block.
                    let is_inline_level =
                        style.style.get_box().original_display.outside() == DisplayOutside::Inline;

                    // Resolve relative inset offsets against the containing block
                    // (the content box of the inline container).
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
                    let box_inputs = inline_box_inputs(style.size().width, margin, child_inputs);
                    drop(style);

                    if is_absolute {
                        // The static-position rectangle
                        // (https://www.w3.org/TR/css-position-3/#staticpos-rect):
                        // - An inline-level box's rectangle is zero-width at its position
                        //   within the line (`ibox.y` is the baseline as out-of-flow boxes are
                        //   zero-sized) and spans the line box in the block axis.
                        // - A block-level box's rectangle spans the containing block's content
                        //   box in the inline axis and is zero-height at the top of the line
                        //   box, or below it if in-flow content precedes it on the line (its
                        //   hypothetical box would start a new line after that content).
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
                        layout.padding = padding; //.map(|p| p / scale);
                        layout.border = border; //.map(|p| p / scale);
                        layout.margin = margin;
                    } else {
                        line_has_in_flow_content = true;
                        // Re-measure the box to get its border-box size (this hits the layout
                        // cache). The size cannot be recovered from `ibox` dimensions as the
                        // space reserved in the line is clamped to be non-negative.
                        let mut output =
                            self.compute_child_layout(taffy::NodeId::from(ibox.id), box_inputs);
                        let size = output.size;
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

                        let layout = node.unrounded_layout_mut();
                        layout.size = size;
                        layout.scrollable_overflow_rect = output.scrollable_overflow_rect;
                        layout.location.x = if is_sideways_lr {
                            inline_position(ibox.x / scale, margin.right + size.width)
                        } else {
                            inline_position(ibox.x / scale, 0.0) + margin.left
                        } + inset_offset.x
                            + span_x;
                        // Parley positions the margin box; offset to the border box even
                        // when a negative top margin makes it extend above the margin box.
                        let margin_box_top = ibox.y / scale;
                        layout.location.y = if mirrors_lines {
                            container_pb.top + content_block_size
                                - block_offset
                                - margin_box_top
                                - margin.top
                                - size.height
                                + inset_offset.y
                        } else {
                            margin_box_top + margin.top + line_box_top + inset_offset.y
                        } + span_y;
                        layout.padding = padding; //.map(|p| p / scale);
                        layout.border = border; //.map(|p| p / scale);
                        layout.margin = margin;

                        // Translate anchors from item-relative to container-relative
                        // coordinates and collect candidates bubbled from the box's subtree
                        if !output.oof_candidates.is_empty() {
                            let location = layout.location;
                            output.oof_candidates.translate(location);
                            oof_candidates.append(&mut output.oof_candidates);
                        }
                    }
                }
            }
        }

        // println!("INLINE LAYOUT FOR {:?}. max_advance: {:?}", node_id, max_advance);
        // dbg!(&inline_layout.text);
        // println!("Computed: w: {} h: {}", inline_layout.layout.width(), inline_layout.layout.height());
        // println!("known_dimensions: w: {:?} h: {:?}", inputs.known_dimensions.width, inputs.known_dimensions.height);
        // println!("\n");

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

        // Put layout back
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
