use super::TextBrush;
use super::inline_hit::{LineSpans, SpanGeometry};
use super::node::{FontExtent, Node};
use crate::inline_trim::LineTrim;
use crate::node::NodeData;
use pageframe_traits::node_id::NodeId;
use parley::{
    AlignmentBaseline, BaselineShift, BreakReason, InlineBoxKind, Layout, Line,
    PositionedLayoutItem,
};
use smallvec::SmallVec;

#[derive(Clone, Copy)]
pub(super) struct EmptySpan {
    line: Option<usize>,
    index: usize,
    order: usize,
    follows_space: bool,
}

pub(super) struct EmptyLine<'a, 'l> {
    pub(super) index: usize,
    pub(super) line: &'a Line<'l, TextBrush>,
    pub(super) text: &'a str,
    pub(super) trim: &'a LineTrim,
    pub(super) found: &'a LineSpans,
}

pub(super) fn empty_spans(
    root: &Node,
    layout: &Layout<TextBrush>,
    keep: impl Fn(NodeId) -> bool,
) -> SmallVec<[EmptySpan; 2]> {
    let mut found = SmallVec::new();
    for (order, inline_box) in layout.inline_boxes().enumerate() {
        if inline_box.kind != InlineBoxKind::EmptySpan {
            continue;
        }
        let id = NodeId::from_u64(inline_box.id);
        if !keep(id) {
            continue;
        }
        let index = inline_box.index;
        found.push(EmptySpan {
            line: layout.lines().position(|line| holds(&line, index)),
            index,
            order,
            follows_space: follows_space(root, id),
        });
    }
    found
}

fn holds(line: &Line<'_, TextBrush>, index: usize) -> bool {
    let range = line.text_range();
    range.start <= index
        && (index < range.end || index == range.end && line.break_reason() != BreakReason::Explicit)
}

fn follows_space(root: &Node, id: NodeId) -> bool {
    let tree = root.tree();
    let mut current = id;
    loop {
        let Some(parent) = tree.get(current).and_then(|node| node.parent) else {
            return false;
        };
        let Some(parent_node) = tree.get(parent) else {
            return false;
        };
        let siblings = &parent_node.children;
        let before = siblings
            .iter()
            .position(|&child| child == current)
            .unwrap_or_default();
        for &sibling in siblings[..before].iter().rev() {
            if let Some(is_space) = trailing_content(root, sibling) {
                return is_space;
            }
        }
        if parent == root.id || !parent_node.is_non_atomic_inline() {
            return false;
        }
        current = parent;
    }
}

fn trailing_content(root: &Node, id: NodeId) -> Option<bool> {
    let node = root.tree().get(id)?;
    match &node.data {
        NodeData::Text(text) => text
            .content
            .chars()
            .next_back()
            .map(|last| matches!(last, ' ' | '\t' | '\n' | '\r' | '\x0c')),
        NodeData::Element(_) if !node.has_boxes() => None,
        NodeData::Element(_) if node.is_non_atomic_inline() => node
            .children
            .iter()
            .rev()
            .find_map(|&child| trailing_content(root, child)),
        NodeData::Element(_) => Some(false),
        _ => None,
    }
}

fn caret_x(layout: &Layout<TextBrush>, at: &EmptyLine<'_, '_>, span: &EmptySpan) -> f32 {
    let mut upstream = None;
    let mut downstream = None;
    let mut space_after = None;
    for positioned in at.line.items() {
        match positioned {
            PositionedLayoutItem::GlyphRun(glyph_run) => {
                let mut x = glyph_run.offset();
                for cluster in glyph_run.run().visual_clusters() {
                    let advance = cluster.advance();
                    let range = cluster.text_range();
                    let (lead, trail) = if cluster.is_rtl() {
                        (x + advance, x)
                    } else {
                        (x, x + advance)
                    };
                    if range.end == span.index {
                        upstream.get_or_insert(trail);
                    }
                    if range.start == span.index {
                        downstream.get_or_insert(lead);
                        if matches!(at.text.get(range), Some(" " | "\t")) {
                            let step = (advance * 64.0).ceil() / 64.0;
                            space_after.get_or_insert(if cluster.is_rtl() {
                                lead - step
                            } else {
                                lead + step
                            });
                        }
                    }
                    x += advance;
                }
            }
            PositionedLayoutItem::InlineBox(inline_box) => {
                if inline_box.index != span.index || inline_box.kind != InlineBoxKind::InFlow {
                    continue;
                }
                let Some(order) = layout
                    .inline_boxes()
                    .position(|peer| peer.id == inline_box.id)
                else {
                    continue;
                };
                let (lead, trail) = if inline_box.is_rtl {
                    (inline_box.x + inline_box.width, inline_box.x)
                } else {
                    (inline_box.x, inline_box.x + inline_box.width)
                };
                if order < span.order {
                    upstream = Some(trail);
                } else {
                    downstream.get_or_insert(lead);
                }
            }
        }
    }
    let metrics = at.line.metrics();
    let x = space_after
        .filter(|_| span.follows_space)
        .or(upstream)
        .or(downstream)
        .unwrap_or(metrics.inline_min_coord + metrics.offset);
    if at.trim.is_hung(x, x) {
        at.trim.hung_edge()
    } else {
        x
    }
}

fn baseline_shift(node: &Node, scale: f32) -> f32 {
    let Some(style) = node.primary_styles() else {
        return 0.0;
    };
    let align = crate::stylo_to_parley::vertical_align(&style);
    if align.alignment != AlignmentBaseline::Baseline {
        return 0.0;
    }
    let parent_size = node
        .parent
        .and_then(|parent| node.tree().get(parent))
        .and_then(Node::primary_styles)
        .map_or(0.0, |style| {
            style.get_font().font_size.used_size.0.px() * scale
        });
    let floor = |value: f32| (value * 64.0).floor() / 64.0;
    match align.shift {
        BaselineShift::Length(shift) => shift * scale,
        BaselineShift::Super => floor(parent_size / 3.0) + 1.0,
        BaselineShift::Sub => -(floor(parent_size / 5.0) + 1.0),
        _ => 0.0,
    }
}

impl Node {
    pub(super) fn empty_fragment(
        &self,
        root: &Node,
        at: EmptyLine<'_, '_>,
        empties: &[EmptySpan],
        font: Option<FontExtent>,
        geometry: &SpanGeometry,
    ) -> Option<taffy::Rect<f32>> {
        let layout = &root.element_data()?.inline_layout_data.as_ref()?.layout;
        let mut xs = empties
            .iter()
            .filter(|empty| empty.line == Some(at.index))
            .map(|empty| caret_x(layout, &at, empty));
        let first = xs.next()?;
        let edges = xs.fold((first, first), |(left, right), x| {
            (left.min(x), right.max(x))
        });
        let metrics = at.line.metrics();
        let mut baseline = metrics.baseline;
        let mut shift = 0.0;
        let mut cursor = Some(self);
        while let Some(node) = cursor {
            if let Some((own, _)) = at.found.own(node.id) {
                baseline = own;
                break;
            }
            if node.id == root.id {
                break;
            }
            shift += baseline_shift(node, geometry.scale);
            cursor = node.parent.and_then(|parent| self.tree().get(parent));
        }
        let extent = font.map(|font| (baseline - shift, font));
        Some(self.line_span_rect(root, metrics, extent, edges, geometry))
    }

    pub(super) fn blank_fragment(
        &self,
        root: &Node,
        empties: &[EmptySpan],
        geometry: &SpanGeometry,
    ) -> Option<taffy::Rect<f32>> {
        if !empties.iter().any(|empty| empty.line.is_none()) {
            return None;
        }
        let style = root.primary_styles()?;
        let layout = root.unrounded_layout();
        let is_flipped = !style.writing_mode.is_vertical() && !style.writing_mode.is_bidi_ltr();
        let width = layout.size.width
            - layout.padding.left
            - layout.padding.right
            - layout.border.left
            - layout.border.right;
        let x = if is_flipped {
            width * geometry.scale
        } else {
            0.0
        };
        Some(root.inline_item_rect(geometry, (x, x), (0.0, 0.0), self.inline_relative_shift()))
    }
}
