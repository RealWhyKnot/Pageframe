use pageframe_traits::node_id::NodeId;
use parley::PositionedLayoutItem;
use style::computed_values::position::T as Position;
use style::values::specified::align::AlignFlags;
use style::values::specified::box_::{DisplayInside, DisplayOutside};
use stylo_taffy::CopyStyleGetters as _;
use taffy::{CoreStyle as _, Rect, ResolveOrZero, Size};

use super::em_ascent;
use crate::layout::{LayoutPassState, resolve_calc_value};
use crate::node::TextLayout;
use crate::taffy_node_id;

#[derive(Debug, Clone, Copy, Default)]
pub struct AnnotationSpace {
    pub rise: f32,
    pub under: f32,
    pub borrow: f32,
    pub block_end: f32,
}

impl AnnotationSpace {
    pub const ZERO: Self = Self {
        rise: 0.0,
        under: 0.0,
        borrow: 0.0,
        block_end: 0.0,
    };
}

#[derive(Default)]
struct MarginStrut {
    positive: f32,
    negative: f32,
}

impl MarginStrut {
    fn append(&mut self, margin: f32) {
        self.positive = self.positive.max(margin);
        self.negative = self.negative.min(margin);
    }

    fn sum(&self) -> f32 {
        self.positive + self.negative
    }
}

struct Edges {
    padding: Rect<f32>,
    border: Rect<f32>,
    margin: Rect<f32>,
}

impl LayoutPassState<'_> {
    fn is_line_ruby(&self, node_id: NodeId) -> bool {
        let node = &self.nodes[node_id];
        node.is_ruby_box()
            && node
                .display_style()
                .is_some_and(|display| display.outside() == DisplayOutside::Inline)
            && node
                .primary_styles()
                .is_some_and(|style| !style.writing_mode.is_vertical())
    }

    pub(crate) fn ruby_line_margin(
        &self,
        node_id: NodeId,
        margin: Rect<f32>,
        parent_size: Size<Option<f32>>,
    ) -> Rect<f32> {
        if !self.is_line_ruby(node_id) {
            return margin;
        }
        let style = self.nodes[node_id].layout_style();
        let frame = style
            .padding()
            .resolve_or_zero(parent_size, resolve_calc_value)
            + style
                .border()
                .resolve_or_zero(parent_size, resolve_calc_value);
        Rect {
            top: -frame.top,
            bottom: -frame.bottom,
            ..margin
        }
    }

    pub(crate) fn placed_ruby_margin(
        &self,
        node_id: NodeId,
        margin: Rect<f32>,
        parent_size: Size<Option<f32>>,
    ) -> Rect<f32> {
        let mut margin = self.ruby_line_margin(node_id, margin, parent_size);
        if self.is_line_ruby(node_id) {
            margin.top -= self.nodes[node_id].layout_data().annotation.borrow;
        }
        margin
    }

    pub(super) fn record_annotation_extent(&mut self, node_id: NodeId, rise: f32, under: f32) {
        let space = &mut self
            .node_from_id_mut(taffy_node_id(node_id))
            .layout_data_mut()
            .annotation;
        space.rise = rise;
        space.under = under;
    }

    pub(crate) fn borrow_annotation_space(
        &mut self,
        node_id: NodeId,
        inline_layout: &mut TextLayout,
        max_advance: f32,
        parent_width: Option<f32>,
    ) {
        let scale = self.viewport.scale();
        let mut rubies = Vec::new();
        for (index, ibox) in inline_layout.layout.inline_boxes().enumerate() {
            let id = NodeId::from_u64(ibox.id);
            if self.is_line_ruby(id) {
                rubies.push((index, id, 0.0_f32));
                self.node_from_id_mut(taffy_node_id(id))
                    .layout_data_mut()
                    .annotation
                    .borrow = 0.0;
            }
        }
        let rises = |state: &Self, id: NodeId| state.nodes[id].layout_data().annotation.rise;
        if rubies.iter().all(|&(_, id, _)| rises(self, id) <= 0.0) {
            return;
        }
        let mut probe = inline_layout.layout.clone();
        probe.break_all_lines(Some(max_advance));
        let mut space = self.block_start_annotation_space(node_id, parent_width) * scale;
        for line in probe.lines() {
            for item in line.items() {
                if let PositionedLayoutItem::InlineBox(placed) = item
                    && let Some(entry) = rubies
                        .iter_mut()
                        .find(|entry| entry.1.as_u64() == placed.id)
                {
                    entry.2 = space.min(rises(self, entry.1) * scale).max(0.0);
                }
            }
            space = self.line_space_under(node_id, &line);
        }
        for (index, ibox) in inline_layout.layout.inline_boxes_mut().enumerate() {
            if let Some(&(_, _, borrow)) = rubies.iter().find(|entry| entry.0 == index) {
                ibox.height -= borrow;
                ibox.baseline = ibox.baseline.map(|baseline| baseline - borrow);
            }
        }
        for &(_, id, borrow) in &rubies {
            self.node_from_id_mut(taffy_node_id(id))
                .layout_data_mut()
                .annotation
                .borrow = borrow / scale;
        }
    }

    pub(crate) fn record_block_end_annotation_space(
        &mut self,
        node_id: NodeId,
        inline_layout: &TextLayout,
        last_line: Option<usize>,
        content_end: f32,
        height: f32,
    ) {
        let scale = self.viewport.scale();
        let edges = self.edges(node_id, None);
        let under = last_line
            .and_then(|index| inline_layout.layout.get(index))
            .map_or(0.0, |line| self.line_space_under(node_id, &line) / scale);
        let block_end =
            if under > 0.0 && edges.border.bottom == 0.0 && !self.establishes_bfc(node_id) {
                (height - edges.border.bottom - (content_end - under)).max(0.0)
            } else {
                0.0
            };
        self.node_from_id_mut(taffy_node_id(node_id))
            .layout_data_mut()
            .annotation
            .block_end = block_end;
    }

    fn line_space_under(
        &self,
        node_id: NodeId,
        line: &parley::Line<'_, crate::node::TextBrush>,
    ) -> f32 {
        let scale = self.viewport.scale();
        let metrics = line.metrics();
        let (top, bottom) = (metrics.block_min_coord, metrics.block_max_coord);
        let mut over = f32::INFINITY;
        let mut under = f32::NEG_INFINITY;
        for item in line.items() {
            match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => {
                    let run = glyph_run.run();
                    let font = run.font_metrics();
                    let above = em_ascent(font.ascent, font.descent, run.font_size(), false);
                    over = over.min(glyph_run.baseline() - above);
                    under = under.max(glyph_run.baseline() - above + run.font_size());
                }
                PositionedLayoutItem::InlineBox(placed) => {
                    if placed.width == 0.0 && placed.height == 0.0 {
                        continue;
                    }
                    let id = NodeId::from_u64(placed.id);
                    over = over.min(placed.y);
                    under = under.max(if self.is_line_ruby(id) {
                        let space = self.nodes[id].layout_data().annotation;
                        placed.y + (space.under - space.borrow) * scale
                    } else {
                        placed.y + placed.height
                    });
                }
            }
        }
        let font_size = self.nodes[node_id]
            .primary_styles()
            .map_or(0.0, |style| style.slow_clone_font_size().used_size().px())
            * scale;
        if over.is_infinite() || under - over < font_size {
            under = bottom - ((bottom - top - font_size) / 2.0).max(0.0);
        }
        (bottom - under.min(bottom)).max(0.0)
    }

    fn edges(&self, node_id: NodeId, width: Option<f32>) -> Edges {
        let style = self.nodes[node_id].layout_style();
        Edges {
            padding: style.padding().resolve_or_zero(width, resolve_calc_value),
            border: style.border().resolve_or_zero(width, resolve_calc_value),
            margin: style.margin().resolve_or_zero(width, resolve_calc_value),
        }
    }

    fn establishes_bfc(&self, node_id: NodeId) -> bool {
        let node = &self.nodes[node_id];
        let Some(style) = node.primary_styles() else {
            return true;
        };
        let Some(parent) = node
            .layout_parent
            .get()
            .filter(|&id| self.nodes[id].is_element())
        else {
            return true;
        };
        let display = style.get_box().display;
        let parent_is_flow = self.nodes[parent]
            .display_style()
            .is_some_and(|display| display.inside() == DisplayInside::Flow);
        !parent_is_flow
            || display.inside() != DisplayInside::Flow
            || display.outside() != DisplayOutside::Block
            || style.clone_float().is_floating()
            || matches!(style.clone_position(), Position::Absolute | Position::Fixed)
            || style.get_box().overflow_x.is_scrollable()
            || style.get_box().overflow_y.is_scrollable()
            || style.get_column().is_multicol()
            || style.get_position().align_content.primary() != AlignFlags::NORMAL
    }

    fn previous_in_flow_sibling(&self, node_id: NodeId) -> Option<NodeId> {
        let parent = self.nodes[node_id].layout_parent.get()?;
        let children = self.nodes[parent].layout_children.borrow();
        let children = children.as_ref()?;
        let index = children.iter().position(|&child| child == node_id)?;
        children[..index].iter().rev().copied().find(|&child| {
            let sibling = &self.nodes[child];
            !sibling.is_out_of_flow()
                && sibling
                    .primary_styles()
                    .is_some_and(|style| !style.clone_float().is_floating())
        })
    }

    fn block_start_annotation_space(&self, node_id: NodeId, width: Option<f32>) -> f32 {
        let edges = self.edges(node_id, width);
        let padding_top = edges.padding.top;
        if edges.border.top != 0.0 || self.establishes_bfc(node_id) {
            return padding_top;
        }
        let mut strut = MarginStrut::default();
        strut.append(edges.margin.top);
        let mut previous_space = 0.0;
        let mut current = node_id;
        let mut direct = true;
        loop {
            if let Some(previous) = self.previous_in_flow_sibling(current) {
                strut.append(self.edges(previous, width).margin.bottom);
                if direct {
                    previous_space = self.block_end_annotation_space(previous);
                }
                break;
            }
            let Some(parent) = self.nodes[current].layout_parent.get() else {
                break;
            };
            let parent_edges = self.edges(parent, width);
            if self.establishes_bfc(parent)
                || parent_edges.border.top != 0.0
                || parent_edges.padding.top != 0.0
            {
                break;
            }
            strut.append(parent_edges.margin.top);
            current = parent;
            direct = false;
        }
        padding_top.max(strut.sum() + padding_top + previous_space)
    }

    fn block_end_annotation_space(&self, node_id: NodeId) -> f32 {
        let node = &self.nodes[node_id];
        if node.flags.is_inline_root() {
            return node.layout_data().annotation.block_end;
        }
        let edges = self.edges(node_id, None);
        if edges.border.bottom != 0.0 || self.establishes_bfc(node_id) {
            return 0.0;
        }
        let last = node.layout_children.borrow().as_ref().and_then(|children| {
            children.iter().rev().copied().find(|&child| {
                let child = &self.nodes[child];
                !child.is_out_of_flow()
                    && child
                        .primary_styles()
                        .is_some_and(|style| !style.clone_float().is_floating())
            })
        });
        let Some(last) = last else {
            return 0.0;
        };
        let space = self.block_end_annotation_space(last);
        if space <= 0.0 {
            return 0.0;
        }
        let layout = node.unrounded_layout();
        let child = self.nodes[last].unrounded_layout();
        (layout.size.height - layout.border.bottom - (child.location.y + child.size.height - space))
            .max(0.0)
    }
}
