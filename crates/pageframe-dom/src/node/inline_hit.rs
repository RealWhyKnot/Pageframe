use super::TextBrush;
use super::node::{FontExtent, LineAxes, Node};
use crate::inline_trim::{LineSpan, LineTrim, settle_line_spans};
use pageframe_traits::node_id::NodeId;
use parley::{LineMetrics, PositionedLayoutItem};
use smallvec::SmallVec;
use style::computed_values::pointer_events::T as PointerEvents;
use style::computed_values::visibility::T as Visibility;
use style::properties::generated::longhands::position::computed_value::T as Position;
use stylo_taffy::CopyStyleGetters;

pub(super) type SpanChain = SmallVec<[NodeId; 4]>;

pub(super) struct SpanGeometry {
    pub(super) scale: f32,
    pub(super) block_offset: f32,
    pub(super) inset: taffy::Rect<f32>,
    pub(super) basis: f32,
}

pub(super) struct LineSpans {
    pub(super) spans: SmallVec<[LineSpan<f32>; 4]>,
    first: SmallVec<[usize; 4]>,
    own: SmallVec<[(NodeId, f32, FontExtent); 4]>,
}

impl LineSpans {
    pub(super) fn own(&self, id: NodeId) -> Option<(f32, FontExtent)> {
        self.own
            .iter()
            .find(|(owner, ..)| *owner == id)
            .map(|(_, baseline, extent)| (*baseline, *extent))
    }
}

#[derive(Clone, Copy)]
pub(crate) struct InlineHit {
    pub(crate) node_id: NodeId,
    pub(crate) is_text: bool,
}

fn takes_pointer(node: &Node) -> bool {
    node.primary_styles().is_none_or(|style| {
        style.clone_pointer_events() != PointerEvents::None
            && style.clone_visibility() == Visibility::Visible
    })
}

#[derive(Clone, Copy)]
pub(crate) enum InlineScope {
    All,
    Flow,
    Layer(NodeId),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct HitOrder {
    is_layered: bool,
    line: usize,
    item: usize,
    depth: usize,
}

fn is_positioned(node: &Node) -> bool {
    node.primary_styles()
        .is_some_and(|style| style.clone_position() != Position::Static)
}

fn union(first: taffy::Rect<f32>, second: taffy::Rect<f32>) -> taffy::Rect<f32> {
    taffy::Rect {
        left: first.left.min(second.left),
        top: first.top.min(second.top),
        right: first.right.max(second.right),
        bottom: first.bottom.max(second.bottom),
    }
}

impl Node {
    pub(crate) fn inline_hit_reach(&self) -> f32 {
        self.span_geometry()
            .map_or(0.0, |geometry| self.hit_reach(geometry.basis))
    }

    fn hit_reach(&self, span_basis: f32) -> f32 {
        let basis = self.inset_basis();
        let tree = self.tree();
        let kids = |node: &Node| {
            node.before()
                .into_iter()
                .chain(node.children.iter().copied())
                .chain(node.after())
                .collect::<SmallVec<[NodeId; 8]>>()
        };
        let font_size = |node: &Node| {
            node.primary_styles()
                .map_or(0.0, |style| style.get_font().font_size.computed_size().px())
        };
        let mut reach = font_size(self) * 2.0;
        let mut pending: Vec<(NodeId, (f32, f32))> =
            kids(self).into_iter().map(|id| (id, (0.0, 0.0))).collect();
        while let Some((id, shift)) = pending.pop() {
            let Some(node) = tree.get(id) else {
                continue;
            };
            let Some(style) = node.primary_styles() else {
                continue;
            };
            if !node.is_non_atomic_inline() && !style.get_box().display.is_contents() {
                continue;
            }
            let (across, down) = crate::util::inline_relative_offset(&style, basis);
            let shift = (shift.0 + across, shift.1 + down);
            let edges = node.block_edges(span_basis);
            let edge = edges.left.max(edges.right).max(edges.top).max(edges.bottom);
            let size = style.get_font().font_size.computed_size().px();
            reach = reach.max(shift.0.abs() + shift.1.abs() + edge + size * 2.0);
            pending.extend(kids(node).into_iter().map(|kid| (kid, shift)));
        }
        reach
    }

    pub(super) fn span_geometry(&self) -> Option<SpanGeometry> {
        let inline_layout = self.element_data()?.inline_layout_data.as_ref()?;
        let root_layout = self.unrounded_layout();
        Some(SpanGeometry {
            scale: inline_layout.layout.scale(),
            block_offset: inline_layout.block_offset,
            inset: root_layout.padding + root_layout.border,
            basis: inline_layout.percent_basis,
        })
    }

    pub(super) fn collect_line_spans(
        &self,
        line: &parley::Line<'_, TextBrush>,
        trim: &LineTrim,
        geometry: &SpanGeometry,
        chain_of: impl Fn(NodeId) -> Option<SpanChain>,
    ) -> LineSpans {
        let text = self
            .element_data()
            .and_then(|element| element.inline_layout_data.as_ref())
            .map_or("", |inline_layout| inline_layout.text.as_str());
        let mut found = LineSpans {
            spans: SmallVec::new(),
            first: SmallVec::new(),
            own: SmallVec::new(),
        };
        for (index, item) in line.items().enumerate() {
            let (owner, start, end, run_text) = match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => {
                    let owner = glyph_run.style().brush.id;
                    let run_text = text.get(glyph_run.run().text_range()).unwrap_or_default();
                    if !found.own.iter().any(|(known, ..)| *known == owner) {
                        let metrics = glyph_run.run().font_metrics();
                        let extent = FontExtent {
                            ascent: metrics.ascent,
                            descent: metrics.descent,
                        };
                        found.own.push((owner, glyph_run.baseline(), extent));
                    }
                    let start = glyph_run.offset();
                    (owner, start, start + glyph_run.advance(), run_text)
                }
                PositionedLayoutItem::InlineBox(inline_box) => (
                    NodeId::from_u64(inline_box.id),
                    inline_box.x,
                    inline_box.x + inline_box.width,
                    "",
                ),
            };
            let Some(chain) = chain_of(owner) else {
                continue;
            };
            let (start, end) = if trim.is_hung(start, end) {
                (f32::INFINITY, f32::NEG_INFINITY)
            } else {
                (start, end)
            };
            for (position, node_id) in chain.into_iter().enumerate() {
                let slot = match found.spans.iter().position(|span| span.node_id == node_id) {
                    Some(slot) => slot,
                    None => {
                        found.spans.push(LineSpan {
                            node_id,
                            left: start,
                            right: end,
                            is_start: false,
                            is_end: false,
                        });
                        found.first.push(index);
                        found.spans.len() - 1
                    }
                };
                let span = &mut found.spans[slot];
                span.left = span.left.min(start);
                span.right = span.right.max(end);
                if position == 0 {
                    span.is_start |= run_text.starts_with(crate::util::INLINE_EDGE_START);
                    span.is_end |= run_text.ends_with(crate::util::INLINE_EDGE_END);
                }
            }
        }
        settle_line_spans(self, &mut found.spans, trim, geometry.basis, geometry.scale);
        found
    }

    pub(super) fn inline_item_rect(
        &self,
        geometry: &SpanGeometry,
        (left, right): (f32, f32),
        (top, bottom): (f32, f32),
        shift: (f32, f32),
    ) -> taffy::Rect<f32> {
        let scale = geometry.scale;
        let unscaled = taffy::Rect {
            left: left / scale,
            top: top / scale,
            right: right / scale,
            bottom: bottom / scale,
        };
        let physical = self.inline_layout_rect(geometry.block_offset, unscaled);
        taffy::Rect {
            left: geometry.inset.left + physical.left + shift.0,
            top: geometry.inset.top + physical.top + shift.1,
            right: geometry.inset.left + physical.right + shift.0,
            bottom: geometry.inset.top + physical.bottom + shift.1,
        }
    }

    pub(super) fn line_span_rect(
        &self,
        root: &Node,
        metrics: &LineMetrics,
        extent: Option<(f32, FontExtent)>,
        (left, right): (f32, f32),
        geometry: &SpanGeometry,
    ) -> taffy::Rect<f32> {
        let edges = self.block_edges(geometry.basis);
        let (over, under) = match root.line_axes() {
            LineAxes::Clockwise(_) => (edges.right, edges.left),
            LineAxes::CounterClockwise(_) => (edges.left, edges.right),
            LineAxes::Level => (edges.top, edges.bottom),
        };
        let (top, bottom) = extent.map_or(
            (metrics.block_min_coord, metrics.block_max_coord),
            |(baseline, extent)| {
                (
                    baseline - extent.ascent.round(),
                    baseline + extent.descent.round(),
                )
            },
        );
        let scale = geometry.scale;
        root.inline_item_rect(
            geometry,
            (left, right),
            (top - over * scale, bottom + under * scale),
            self.inline_relative_shift(),
        )
    }

    pub(super) fn glyph_run_rect(
        &self,
        geometry: &SpanGeometry,
        glyph_run: &parley::GlyphRun<'_, TextBrush>,
        shift: (f32, f32),
    ) -> taffy::Rect<f32> {
        let start = glyph_run.offset();
        self.glyph_span_rect(
            geometry,
            glyph_run,
            (start, start + glyph_run.advance()),
            shift,
        )
    }

    fn glyph_span_rect(
        &self,
        geometry: &SpanGeometry,
        glyph_run: &parley::GlyphRun<'_, TextBrush>,
        extent: (f32, f32),
        shift: (f32, f32),
    ) -> taffy::Rect<f32> {
        let metrics = glyph_run.run().font_metrics();
        let baseline = glyph_run.baseline();
        self.inline_item_rect(
            geometry,
            extent,
            (
                baseline - metrics.ascent.round(),
                baseline + metrics.descent.round(),
            ),
            shift,
        )
    }

    pub(crate) fn inline_overflow_bounds(&self) -> Option<taffy::Rect<f32>> {
        let inline_layout = self.element_data()?.inline_layout_data.as_ref()?;
        let layout = &inline_layout.layout;
        let geometry = self.span_geometry()?;
        let tree = self.tree();
        let size = self.final_layout().size;
        let is_moved = |node: &Node| node.inline_relative_shift() != (0.0, 0.0);
        let overhangs = |rect: &taffy::Rect<f32>| {
            rect.left < 0.0
                || rect.top < 0.0
                || rect.right > size.width
                || rect.bottom > size.height
        };
        let mut bounds: Option<taffy::Rect<f32>> = None;
        let mut has_moved = false;
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let shift = tree
                    .get(glyph_run.style().brush.id)
                    .map_or((0.0, 0.0), Node::inline_relative_shift);
                let is_shifted = shift != (0.0, 0.0);
                has_moved |= is_shifted;
                let rect = self.glyph_run_rect(&geometry, &glyph_run, shift);
                if is_shifted || overhangs(&rect) {
                    bounds = Some(bounds.map_or(rect, |known| union(known, rect)));
                }
            }
        }
        if !has_moved {
            return bounds;
        }
        let text = inline_layout.text.as_str();
        let is_rtl = layout.is_rtl();
        for line in layout.lines() {
            let trim = LineTrim::new(self, &line, text, is_rtl);
            let found = self.collect_line_spans(&line, &trim, &geometry, |id| self.span_chain(id));
            for span in &found.spans {
                let Some(node) = tree
                    .get(span.node_id)
                    .filter(|node| node.is_non_atomic_inline() && is_moved(node))
                else {
                    continue;
                };
                let rect = node.line_span_rect(
                    self,
                    line.metrics(),
                    found.own(span.node_id),
                    (span.left, span.right),
                    &geometry,
                );
                bounds = Some(bounds.map_or(rect, |known| union(known, rect)));
            }
        }
        bounds
    }

    pub(super) fn span_chain(&self, mut id: NodeId) -> Option<SpanChain> {
        let stop = self.is_anonymous().then_some(self.parent).flatten();
        let mut chain = SpanChain::new();
        while id != self.id && Some(id) != stop {
            chain.push(id);
            id = self.tree().get(id)?.parent?;
        }
        Some(chain)
    }

    pub(crate) fn inline_hit(
        &self,
        x: f32,
        y: f32,
        probe: &super::hit_probe::HitProbe,
        scope: InlineScope,
    ) -> Option<InlineHit> {
        let inline_layout = self.element_data()?.inline_layout_data.as_ref()?;
        let layout = &inline_layout.layout;
        let geometry = self.span_geometry()?;
        let text = inline_layout.text.as_str();
        let is_rtl = layout.is_rtl();
        let tree = self.tree();
        let order = |id: NodeId, line: usize, item: usize| {
            let chain = self.span_chain(id).unwrap_or_default();
            HitOrder {
                is_layered: chain
                    .iter()
                    .any(|id| tree.get(*id).is_some_and(is_positioned)),
                line,
                item,
                depth: chain.len(),
            }
        };
        let in_scope = |id: NodeId| match scope {
            InlineScope::All => true,
            InlineScope::Flow => !self
                .span_chain(id)
                .unwrap_or_default()
                .iter()
                .any(|id| tree.get(*id).is_some_and(Node::starts_inline_layer)),
            InlineScope::Layer(layer) => self.span_chain(id).unwrap_or_default().contains(&layer),
        };
        let mut best: Option<(HitOrder, InlineHit)> = None;
        let mut offer = |key: HitOrder, hit: InlineHit| {
            if !in_scope(hit.node_id) {
                return;
            }
            if best.as_ref().is_none_or(|(known, _)| key >= *known) {
                best = Some((key, hit));
            }
        };
        let reach = self.layout_data().inline_hit_reach * geometry.scale;
        let (_, block) = self.inline_layout_point(
            geometry.block_offset,
            x - geometry.inset.left,
            y - geometry.inset.top,
        );
        let block = block * geometry.scale;
        let count = layout.len();
        let is_mirrored = layout
            .get(0)
            .zip(count.checked_sub(1).and_then(|last| layout.get(last)))
            .is_some_and(|(first, last)| {
                first.metrics().block_min_coord > last.metrics().block_min_coord
            });
        let line_index_at = |position: usize| {
            if is_mirrored {
                count.saturating_sub(position + 1)
            } else {
                position
            }
        };
        let mut low = 0;
        let mut high = count;
        while low < high {
            let middle = low + (high - low) / 2;
            let is_above = layout
                .get(line_index_at(middle))
                .is_some_and(|line| line.metrics().block_max_coord + reach < block);
            if is_above {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        for position in low..count {
            let line_index = line_index_at(position);
            let Some(line) = layout.get(line_index) else {
                break;
            };
            if line.metrics().block_min_coord - reach > block {
                break;
            }
            let trim = LineTrim::new(self, &line, text, is_rtl);
            for (index, item) in line.items().enumerate() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let owner = glyph_run.style().brush.id;
                let Some(node) = tree.get(owner).filter(|node| takes_pointer(node)) else {
                    continue;
                };
                let start = glyph_run.offset();
                let end = start + glyph_run.advance();
                if trim.is_hung(start, end) {
                    continue;
                }
                let extent = trim.clamp(start, end, false);
                let shift = node.inline_relative_shift();
                let rect = self.glyph_span_rect(&geometry, &glyph_run, extent, shift);
                if probe.holds(&rect, x, y) {
                    let mut key = order(owner, line_index, index);
                    key.depth += 1;
                    offer(
                        key,
                        InlineHit {
                            node_id: owner,
                            is_text: true,
                        },
                    );
                }
            }
            let found = self.collect_line_spans(&line, &trim, &geometry, |id| self.span_chain(id));
            for (span, first) in found.spans.iter().zip(&found.first) {
                let Some(node) = tree
                    .get(span.node_id)
                    .filter(|node| node.is_non_atomic_inline() && takes_pointer(node))
                else {
                    continue;
                };
                let rect = node.line_span_rect(
                    self,
                    line.metrics(),
                    found.own(span.node_id),
                    (span.left, span.right),
                    &geometry,
                );
                if probe.holds(&rect, x, y) {
                    let key = order(span.node_id, line_index, *first);
                    offer(
                        key,
                        InlineHit {
                            node_id: span.node_id,
                            is_text: false,
                        },
                    );
                }
            }
        }
        best.map(|(_, hit)| hit)
    }
}
