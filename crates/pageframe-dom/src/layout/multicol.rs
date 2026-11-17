use std::ops::Range;

use style::values::computed::{CSSPixelLength, ColumnFill};
use style::values::generics::column::GenericColumnCount;
use style::values::generics::length::{
    GenericLengthPercentageOrAuto, GenericLengthPercentageOrNormal,
};
use taffy::{
    AvailableSpace, Baselines, BoxSizing, CollapsibleMarginSet, CoreStyle as _, LayoutInput,
    LayoutOutput, Line, MaybeMath as _, MaybeResolve as _, Point, Rect, RequestedAxis,
    ResolveOrZero as _, RunMode, ScrollableOverflow, Size, SizingMode,
};

use pageframe_traits::node_id::NodeId;

use crate::BaseDocument;
use crate::layout::LayoutPassState;
use crate::layout::replaced::is_replaced_element;
use crate::layout::resolve_calc_value;
use crate::node::{Node, SpecialElementData};

const MAX_COLUMNS: usize = 4096;
const BALANCE_STEPS: usize = 24;
const EDGE: f32 = 0.01;
const LAYOUT_UNIT: f32 = 64.0;
const FAR: f32 = 1.0e7;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColumnFragment {
    pub flow_start: f32,
    pub flow_end: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnLayout {
    pub fragments: Vec<ColumnFragment>,
    pub column_width: f32,
    pub stride: f32,
    pub gap: f32,
    pub content_left: f32,
    pub content_top: f32,
    pub height: f32,
}

impl ColumnLayout {
    fn column_x(&self, index: usize) -> f32 {
        let count = f32::from(u16::try_from(index).unwrap_or(u16::MAX));
        (count * self.stride * LAYOUT_UNIT).round() / LAYOUT_UNIT
    }

    pub fn index_at(&self, flow_y: f32) -> usize {
        self.fragments
            .partition_point(|fragment| fragment.flow_start <= flow_y + EDGE)
            .saturating_sub(1)
    }

    pub fn offset(&self, index: usize) -> Point<f32> {
        let flow_start = self
            .fragments
            .get(index)
            .map_or(self.content_top, |fragment| fragment.flow_start);
        Point {
            x: self.column_x(index),
            y: self.content_top - flow_start,
        }
    }

    pub fn offset_at(&self, flow_y: f32) -> Point<f32> {
        self.offset(self.index_at(flow_y))
    }

    pub fn flow_range(&self, index: usize) -> (f32, f32) {
        let Some(fragment) = self.fragments.get(index) else {
            return (-FAR, FAR);
        };
        let start = if index == 0 {
            -FAR
        } else {
            fragment.flow_start
        };
        let end = if index + 1 == self.fragments.len() {
            FAR
        } else {
            fragment.flow_end
        };
        (start, end)
    }

    pub fn flow_point(&self, x: f32, y: f32) -> Option<Point<f32>> {
        let last = self.fragments.len().checked_sub(1)?;
        let half_gap = self.gap / 2.0;
        (0..=last).find_map(|index| {
            let left = self.content_left + self.column_x(index);
            let right = left + self.column_width;
            let is_inside =
                (index == 0 || x >= left - half_gap) && (index == last || x < right + half_gap);
            if !is_inside {
                return None;
            }
            let offset = self.offset(index);
            let flow_y = y - offset.y;
            let (start, end) = self.flow_range(index);
            (flow_y >= start && flow_y < end).then_some(Point {
                x: x - offset.x,
                y: flow_y,
            })
        })
    }

    pub fn visual_box(&self, location: Point<f32>, size: Size<f32>) -> (Point<f32>, Size<f32>) {
        let first = self.index_at(location.y);
        let bottom = location.y + size.height;
        let last = if size.height > 2.0 * EDGE {
            self.index_at(bottom - 2.0 * EDGE).max(first)
        } else {
            first
        };
        let start = self.offset(first);
        if first == last {
            let moved = Point {
                x: location.x + start.x,
                y: location.y + start.y,
            };
            return (moved, size);
        }
        let end = self.offset(last);
        let height = self
            .fragments
            .get(first..last)
            .unwrap_or_default()
            .iter()
            .map(|fragment| fragment.flow_end - fragment.flow_start)
            .fold(bottom + end.y - self.content_top, f32::max);
        (
            Point {
                x: location.x + start.x,
                y: self.content_top,
            },
            Size {
                width: size.width + end.x - start.x,
                height,
            },
        )
    }

    pub fn extent(&self) -> f32 {
        let count = self.fragments.len().max(1);
        self.column_x(count) - self.gap
    }
}

#[derive(Debug, Clone, Copy)]
struct Solid {
    top: f32,
    bottom: f32,
    is_breakable: bool,
    line: Option<LineSlot>,
}

#[derive(Debug, Clone, Copy)]
struct LineSlot {
    group: usize,
    first: usize,
    index: usize,
    count: usize,
    orphans: usize,
    widows: usize,
    box_top: f32,
}

struct ColumnStyle {
    count: Option<usize>,
    width: Option<f32>,
    gap: Option<f32>,
    font_size: f32,
    fill: ColumnFill,
}

fn used_columns(style: &ColumnStyle, content_width: f32, gap: f32) -> (usize, f32) {
    let fitting = |column_width: f32| -> usize {
        let stride = (column_width + gap).max(EDGE);
        let mut count = 1;
        while count < MAX_COLUMNS && f32_from(count + 1) * stride <= content_width + gap + EDGE {
            count += 1;
        }
        count
    };
    let count = match (style.count, style.width) {
        (Some(count), None) => count,
        (None, Some(width)) => fitting(width),
        (Some(count), Some(width)) => count.min(fitting(width)),
        (None, None) => 1,
    }
    .clamp(1, MAX_COLUMNS);
    let width = ((content_width + gap) / f32_from(count) - gap).max(0.0);
    (count, width)
}

fn relink_lines(solids: &mut [Solid]) {
    for (position, solid) in solids.iter_mut().enumerate() {
        if let Some(line) = solid.line.as_mut() {
            line.first = position.saturating_sub(line.index);
        }
    }
}

fn line_minimum(value: i32) -> usize {
    usize::try_from(value).unwrap_or(1).max(1)
}

fn f32_from(count: usize) -> f32 {
    f32::from(u16::try_from(count).unwrap_or(u16::MAX))
}

fn break_before(solids: &[Solid], start: f32, limit: f32) -> f32 {
    let mut end = limit;
    while let Some(solid) = solids
        .iter()
        .find(|solid| !solid.is_breakable && solid.top < end - EDGE && solid.bottom > end + EDGE)
    {
        end = solid.top;
    }
    if end > start + EDGE {
        return honour_orphans_and_widows(solids, start, end);
    }
    solids
        .iter()
        .filter(|solid| {
            !solid.is_breakable && solid.top <= start + EDGE && solid.bottom > start + EDGE
        })
        .map(|solid| solid.bottom)
        .fold(limit.max(start + 1.0), f32::max)
}

fn honour_orphans_and_widows(solids: &[Solid], start: f32, end: f32) -> f32 {
    let Some(slot) = solids
        .iter()
        .filter(|solid| solid.top >= end - EDGE)
        .min_by(|left, right| {
            left.top
                .total_cmp(&right.top)
                .then_with(|| right.line.is_some().cmp(&left.line.is_some()))
        })
        .and_then(|solid| solid.line)
    else {
        return end;
    };
    let line_top = |index: usize| {
        solids
            .get(slot.first + index)
            .filter(|solid| {
                solid
                    .line
                    .is_some_and(|line| line.group == slot.group && line.index == index)
            })
            .map_or(end, |solid| solid.top)
    };
    let first_here = (0..slot.index)
        .find(|&index| line_top(index) >= start - EDGE)
        .unwrap_or(slot.index);
    let fitted = slot.index - first_here;
    let minimum = if first_here == 0 {
        slot.orphans
    } else {
        slot.orphans.max(slot.widows)
    };
    if fitted < minimum {
        let before = if slot.box_top > start + EDGE {
            slot.box_top
        } else {
            line_top(0)
        };
        return if first_here == 0 && before > start + EDGE {
            before
        } else {
            end
        };
    }
    let remaining = slot.count - first_here;
    if remaining - fitted >= slot.widows {
        return end;
    }
    let kept = remaining
        .saturating_sub(slot.widows)
        .max(remaining.min(slot.orphans));
    let top = line_top(first_here + kept);
    if top > start + EDGE { top } else { end }
}

fn next_start(solids: &[Solid], end: f32, flow_end: f32) -> f32 {
    let is_covered = solids
        .iter()
        .any(|solid| solid.top <= end + EDGE && solid.bottom > end + EDGE);
    if is_covered {
        return end;
    }
    solids
        .iter()
        .filter(|solid| solid.top > end)
        .map(|solid| solid.top)
        .fold(flow_end, f32::min)
}

fn fill_columns(
    solids: &[Solid],
    flow: Range<f32>,
    height: f32,
    most: usize,
) -> Vec<ColumnFragment> {
    let Range {
        start: flow_start,
        end: flow_end,
    } = flow;
    let height = height.max(1.0);
    let mut fragments = Vec::new();
    let mut start = flow_start;
    loop {
        let limit = start + height;
        if limit >= flow_end - EDGE || fragments.len() + 1 >= most {
            fragments.push(ColumnFragment {
                flow_start: start,
                flow_end: flow_end.max(start),
            });
            return fragments;
        }
        let end = break_before(solids, start, limit);
        fragments.push(ColumnFragment {
            flow_start: start,
            flow_end: end,
        });
        start = next_start(solids, end, flow_end);
        if start >= flow_end - EDGE {
            return fragments;
        }
    }
}

fn balanced_height(solids: &[Solid], flow_start: f32, flow_end: f32, count: usize) -> f32 {
    let total = flow_end - flow_start;
    if total <= 0.0 {
        return 0.0;
    }
    let fits = |height: f32| {
        let most = (count + 2).min(MAX_COLUMNS);
        fill_columns(solids, flow_start..flow_end, height, most).len() <= count
    };
    let snap = |height: f32| (height * LAYOUT_UNIT - EDGE).ceil() / LAYOUT_UNIT;
    let mut low = total / f32_from(count);
    if fits(low) {
        return snap(low);
    }
    let mut high = total;
    for _ in 0..BALANCE_STEPS {
        let middle = (low + high) / 2.0;
        if fits(middle) {
            high = middle;
        } else {
            low = middle;
        }
    }
    snap(high)
}

impl BaseDocument {
    fn column_style(&self, node_id: NodeId, content_width: f32) -> Option<ColumnStyle> {
        let style = self.nodes[node_id].primary_styles()?;
        let column = style.get_column();
        let count = match column.column_count {
            GenericColumnCount::Integer(count) => Some(usize::try_from(count.0).unwrap_or(1)),
            GenericColumnCount::Auto => None,
        };
        let width = match &column.column_width {
            GenericLengthPercentageOrAuto::LengthPercentage(width) => Some(width.0.px()),
            GenericLengthPercentageOrAuto::Auto => None,
        };
        let gap = match &style.get_position().column_gap {
            GenericLengthPercentageOrNormal::Normal => None,
            GenericLengthPercentageOrNormal::LengthPercentage(gap) => {
                Some(gap.0.resolve(CSSPixelLength::new(content_width)).px())
            }
        };
        Some(ColumnStyle {
            count,
            width,
            gap,
            font_size: style.get_font().font_size.computed_size().px(),
            fill: column.column_fill,
        })
    }

    fn is_breakable_container(&self, node: &Node) -> bool {
        if node.flags.is_inline_root() {
            return true;
        }
        let Some(element) = node.data.downcast_element() else {
            return false;
        };
        if !matches!(element.special_data, SpecialElementData::None)
            || is_replaced_element(&element.name.local)
            || node.flags.is_table_root()
        {
            return false;
        }
        if !matches!(
            node.taffy_display(),
            taffy::Display::Block | taffy::Display::FlowRoot
        ) {
            return false;
        }
        node.primary_styles().is_some_and(|style| {
            let overflow = style.get_box();
            !style.is_multicol()
                && !overflow.overflow_x.is_scrollable()
                && !overflow.overflow_y.is_scrollable()
        })
    }

    fn collect_lines(&self, node: &Node, box_top: f32, content_top: f32, solids: &mut Vec<Solid>) {
        let Some(text_layout) = node
            .data
            .downcast_element()
            .and_then(|element| element.inline_layout_data.as_ref())
        else {
            return;
        };
        let (orphans, widows) = node.primary_styles().map_or((2, 2), |style| {
            let text = style.get_inherited_text();
            (line_minimum(text.orphans), line_minimum(text.widows))
        });
        let scale = self.viewport.scale();
        let lines_top = content_top + text_layout.block_offset;
        let group = solids.len();
        let count = text_layout.layout.lines().count();
        for (index, line) in text_layout.layout.lines().enumerate() {
            let metrics = line.metrics();
            solids.push(Solid {
                top: lines_top + metrics.block_min_coord / scale,
                bottom: lines_top + metrics.block_max_coord / scale,
                is_breakable: false,
                line: Some(LineSlot {
                    group,
                    first: group,
                    index,
                    count,
                    orphans,
                    widows,
                    box_top,
                }),
            });
        }
    }

    fn collect_solids(&self, node_id: NodeId, top: f32, solids: &mut Vec<Solid>) {
        let node = &self.nodes[node_id];
        if node.flags.is_inline_root() {
            let layout = node.unrounded_layout();
            self.collect_lines(
                node,
                top,
                top + layout.border.top + layout.padding.top,
                solids,
            );
            return;
        }
        let children = node.layout_children.borrow();
        for &child_id in children.iter().flatten() {
            let child = &self.nodes[child_id];
            if child.is_out_of_flow() {
                continue;
            }
            let layout = child.unrounded_layout();
            let child_top = top + layout.location.y;
            let child_bottom = child_top + layout.size.height;
            if child_bottom <= child_top {
                continue;
            }
            if !self.is_breakable_container(child) {
                solids.push(Solid {
                    top: child_top,
                    bottom: child_bottom,
                    is_breakable: false,
                    line: None,
                });
                continue;
            }
            let inner_top = child_top + layout.border.top + layout.padding.top;
            let inner_bottom = child_bottom - layout.border.bottom - layout.padding.bottom;
            if inner_top > child_top {
                solids.push(Solid {
                    top: child_top,
                    bottom: inner_top,
                    is_breakable: true,
                    line: None,
                });
            }
            let before = solids.len();
            self.collect_solids(child_id, child_top, solids);
            if solids.len() == before && inner_bottom > inner_top {
                solids.push(Solid {
                    top: inner_top,
                    bottom: inner_bottom,
                    is_breakable: true,
                    line: None,
                });
            }
            if child_bottom > inner_bottom {
                solids.push(Solid {
                    top: inner_bottom,
                    bottom: child_bottom,
                    is_breakable: true,
                    line: None,
                });
            }
        }
    }
}

impl LayoutPassState<'_> {
    pub(crate) fn compute_multicol_layout(
        &mut self,
        node_id: NodeId,
        inputs: LayoutInput,
        mut flow_layout: impl FnMut(&mut Self, LayoutInput) -> LayoutOutput,
    ) -> LayoutOutput {
        let (padding_border, padding_size, style_size, min_size, max_size) = {
            let style = self.nodes[node_id].layout_style();
            let padding = style
                .padding()
                .resolve_or_zero(inputs.parent_size.width, resolve_calc_value);
            let border = style
                .border()
                .resolve_or_zero(inputs.parent_size.width, resolve_calc_value);
            let padding_border = padding + border;
            if inputs.sizing_mode == SizingMode::ContentSize {
                (
                    padding_border,
                    padding.sum_axes(),
                    Size::NONE,
                    Size::NONE,
                    Size::NONE,
                )
            } else {
                let adjustment = if style.box_sizing() == BoxSizing::ContentBox {
                    padding_border.sum_axes()
                } else {
                    Size::ZERO
                };
                (
                    padding_border,
                    padding.sum_axes(),
                    style
                        .size()
                        .maybe_resolve(inputs.parent_size, resolve_calc_value)
                        .maybe_add(adjustment),
                    style
                        .min_size()
                        .maybe_resolve(inputs.parent_size, resolve_calc_value)
                        .maybe_add(adjustment),
                    style
                        .max_size()
                        .maybe_resolve(inputs.parent_size, resolve_calc_value)
                        .maybe_add(adjustment),
                )
            }
        };
        let edges = padding_border.sum_axes();
        let padding_width = padding_size.width;
        let padding_height = padding_size.height;
        let border_width = edges.width - padding_width;
        let known_width = inputs
            .known_dimensions
            .width
            .or(style_size.width.maybe_clamp(min_size.width, max_size.width));
        let Some(column_style) = self.column_style(
            node_id,
            known_width.map_or(0.0, |width| width - edges.width),
        ) else {
            return flow_layout(self, inputs);
        };
        let gap = column_style.gap.unwrap_or(column_style.font_size).max(0.0);
        let width = known_width.unwrap_or_else(|| {
            let natural = flow_layout(self, inputs).size.width - edges.width;
            let count = column_style.count.unwrap_or(1).clamp(1, MAX_COLUMNS);
            let column = natural.max(column_style.width.unwrap_or(0.0));
            let wanted = f32_from(count) * (column + gap) - gap + edges.width;
            let fitted = match inputs.available_space.width {
                AvailableSpace::Definite(available) => wanted.min(available.max(edges.width)),
                _ => wanted,
            };
            fitted.maybe_clamp(min_size.width, max_size.width)
        });
        let content_width = (width - edges.width).max(0.0);
        let (count, exact_width) = used_columns(&column_style, content_width, gap);
        let column_width = (exact_width * LAYOUT_UNIT + EDGE).floor() / LAYOUT_UNIT;

        let mut flow_inputs = inputs;
        flow_inputs.run_mode = RunMode::PerformLayout;
        flow_inputs.sizing_mode = SizingMode::ContentSize;
        flow_inputs.axis = RequestedAxis::Both;
        flow_inputs.known_dimensions = Size {
            width: Some(column_width + edges.width),
            height: None,
        };
        flow_inputs.available_space = Size {
            width: AvailableSpace::Definite(column_width + edges.width),
            height: AvailableSpace::MaxContent,
        };
        flow_inputs.vertical_margins_are_collapsible = Line::FALSE;
        let flow = flow_layout(self, flow_inputs);

        let flow_start = padding_border.top;
        let clamped_end = (flow.size.height - padding_border.bottom).max(flow_start);
        let mut solids = Vec::new();
        let node = &self.nodes[node_id];
        if node.flags.is_inline_root() {
            self.collect_lines(node, flow_start, flow_start, &mut solids);
        } else {
            self.collect_solids(node_id, 0.0, &mut solids);
        }
        solids.sort_by(|a, b| a.top.total_cmp(&b.top));
        relink_lines(&mut solids);
        let flow_end = solids
            .iter()
            .map(|solid| solid.bottom)
            .fold(clamped_end, f32::max);

        let fixed = inputs
            .known_dimensions
            .height
            .or(style_size
                .height
                .maybe_clamp(min_size.height, max_size.height))
            .map(|height| (height - edges.height).max(0.0));
        let limit = fixed.or(max_size
            .height
            .map(|height| (height - edges.height).max(0.0)));
        let (column_height, fragments) = if column_style.fill == ColumnFill::Auto {
            let available = limit.unwrap_or(flow_end - flow_start);
            let fragments = fill_columns(&solids, flow_start..flow_end, available, MAX_COLUMNS);
            let used = fixed.unwrap_or_else(|| {
                fragments
                    .iter()
                    .map(|fragment| fragment.flow_end - fragment.flow_start)
                    .fold(0.0, f32::max)
                    .min(available)
            });
            (used, fragments)
        } else {
            let balanced = balanced_height(&solids, flow_start, flow_end, count);
            let column_height = limit.map_or(balanced, |limit| balanced.min(limit));
            let fragments = fill_columns(&solids, flow_start..flow_end, column_height, MAX_COLUMNS);
            (column_height, fragments)
        };
        let content_height = fixed.unwrap_or_else(|| {
            column_height.maybe_max(min_size.height.map(|height| height - edges.height))
        });
        let columns = ColumnLayout {
            fragments,
            column_width,
            stride: exact_width + gap,
            gap,
            content_left: padding_border.left,
            content_top: padding_border.top,
            height: column_height,
        };

        let mut output = flow;
        output.size = Size {
            width,
            height: content_height + edges.height,
        };
        output.scrollable_overflow_rect = ScrollableOverflow::new(
            Rect {
                left: 0.0,
                top: 0.0,
                right: (columns.extent() + padding_width).max(width - border_width),
                bottom: content_height + padding_height,
            },
            Point { x: false, y: false },
        );
        output.baselines = Baselines::NONE;
        output.top_margin = CollapsibleMarginSet::ZERO;
        output.bottom_margin = CollapsibleMarginSet::ZERO;
        output.margins_can_collapse_through = false;
        if inputs.run_mode == RunMode::PerformLayout {
            self.nodes[node_id].layout_data_mut().columns = Some(Box::new(columns));
        }
        output
    }
}
