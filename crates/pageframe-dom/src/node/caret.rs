use super::TextBrush;
use super::inline_hit::{SpanChain, SpanGeometry};
use super::node::{LineAxes, Node};
use crate::inline_trim::collapses_spaces;
use pageframe_traits::node_id::NodeId;
use parley::{BreakReason, InlineBoxKind, Line, PositionedLayoutItem};
use std::ops::Range;

pub(super) enum CaretTarget {
    Shift((f32, f32)),
    Offset(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum LineEdge {
    Left,
    Right,
}

enum Picked {
    Leaf((f32, f32)),
    Offset(usize),
    Edge(LineEdge),
}

struct CaretRun {
    chain: SpanChain,
    start: f32,
    end: f32,
    text: Range<usize>,
    shift: (f32, f32),
    is_rtl: bool,
    edge: Option<LineEdge>,
    is_lone: bool,
}

struct CaretGroup<'runs> {
    is_leaf: bool,
    start: f32,
    end: f32,
    runs: Vec<&'runs CaretRun>,
}

fn group_at<'runs>(runs: &[&'runs CaretRun], depth: usize) -> Vec<CaretGroup<'runs>> {
    let mut groups: Vec<(Option<NodeId>, CaretGroup<'runs>)> = Vec::new();
    for run in runs {
        let key = run
            .chain
            .len()
            .checked_sub(depth + 1)
            .and_then(|index| run.chain.get(index).copied());
        let known = key.and_then(|id| groups.iter_mut().find(|(owner, _)| *owner == Some(id)));
        if let Some((_, group)) = known {
            group.start = group.start.min(run.start);
            group.end = group.end.max(run.end);
            group.runs.push(run);
            continue;
        }
        groups.push((
            key,
            CaretGroup {
                is_leaf: key.is_none(),
                start: run.start,
                end: run.end,
                runs: vec![run],
            },
        ));
    }
    groups.into_iter().map(|(_, group)| group).collect()
}

fn pick(runs: &[&CaretRun], depth: usize, point: f32) -> Option<Picked> {
    let groups = group_at(runs, depth);
    if let Some(group) = groups
        .iter()
        .find(|group| point >= group.start && point < group.end)
    {
        if group.is_leaf {
            return group.runs.first().map(|run| Picked::Leaf(run.shift));
        }
        return pick(&group.runs, depth + 1, point);
    }
    let distance = |group: &CaretGroup<'_>| {
        if point < group.start {
            group.start - point
        } else {
            point - group.end
        }
    };
    let nearest = groups
        .iter()
        .min_by(|first, second| distance(first).total_cmp(&distance(second)))?;
    let text_start = nearest.runs.iter().map(|run| run.text.start).min()?;
    let text_end = nearest.runs.iter().map(|run| run.text.end).max()?;
    let is_rtl = nearest.runs.first().is_some_and(|run| run.is_rtl);
    let is_before = point < nearest.start;
    let wanted = if is_before {
        LineEdge::Left
    } else {
        LineEdge::Right
    };
    let at_edge = nearest
        .runs
        .iter()
        .any(|run| run.is_lone || run.edge == Some(wanted));
    if at_edge {
        return Some(Picked::Edge(wanted));
    }
    Some(Picked::Offset(if is_before == is_rtl {
        text_end
    } else {
        text_start
    }))
}

#[derive(Clone, Copy)]
struct EdgeItem {
    left: f32,
    right: f32,
    is_glyphs: bool,
}

fn edge_items(line: &Line<'_, TextBrush>) -> Option<(EdgeItem, EdgeItem)> {
    let mut items = line.items().filter_map(|item| match item {
        PositionedLayoutItem::GlyphRun(glyph_run) => Some(EdgeItem {
            left: glyph_run.offset(),
            right: glyph_run.offset() + glyph_run.advance(),
            is_glyphs: true,
        }),
        PositionedLayoutItem::InlineBox(inline_box) if inline_box.kind == InlineBoxKind::InFlow => {
            Some(EdgeItem {
                left: inline_box.x,
                right: inline_box.x + inline_box.width,
                is_glyphs: false,
            })
        }
        PositionedLayoutItem::InlineBox(_) => None,
    });
    let first = items.next()?;
    let last = items.last().unwrap_or(first);
    Some((first, last))
}

pub(super) fn edge_beyond_line(line: &Line<'_, TextBrush>, inline: f32) -> Option<LineEdge> {
    let (first, last) = edge_items(line)?;
    let left = if first.is_glyphs {
        first.left
    } else {
        first.right
    };
    let right = if last.is_glyphs {
        last.right
    } else {
        last.left
    };
    if inline < left {
        Some(LineEdge::Left)
    } else if inline >= right {
        Some(LineEdge::Right)
    } else {
        None
    }
}

struct RunSpan {
    left: f32,
    right: f32,
    text: std::ops::Range<usize>,
    is_rtl: bool,
}

impl RunSpan {
    fn offset_at(&self, side: LineEdge) -> usize {
        if (side == LineEdge::Left) == self.is_rtl {
            self.text.end
        } else {
            self.text.start
        }
    }
}

pub(super) fn base_run_offset(
    line: &Line<'_, TextBrush>,
    base_is_rtl: bool,
    inline: f32,
    offset: usize,
) -> usize {
    let spans: Vec<RunSpan> = line
        .items()
        .filter_map(|item| match item {
            PositionedLayoutItem::GlyphRun(glyph_run) => Some(RunSpan {
                left: glyph_run.offset(),
                right: glyph_run.offset() + glyph_run.advance(),
                text: glyph_run.text_range(),
                is_rtl: glyph_run.run().is_line_rtl(),
            }),
            PositionedLayoutItem::InlineBox(inline_box)
                if inline_box.kind == InlineBoxKind::InFlow =>
            {
                Some(RunSpan {
                    left: inline_box.x,
                    right: inline_box.x + inline_box.width,
                    text: inline_box.index..inline_box.index,
                    is_rtl: inline_box.is_rtl,
                })
            }
            PositionedLayoutItem::InlineBox(_) => None,
        })
        .collect();
    let Some(index) = spans
        .iter()
        .position(|span| span.left <= inline && inline < span.right)
    else {
        return offset;
    };
    let hit = &spans[index];
    if hit.is_rtl == base_is_rtl {
        return offset;
    }
    let neighbour = if hit.offset_at(LineEdge::Left) == offset {
        index.checked_sub(1).map(|before| (before, LineEdge::Right))
    } else if hit.offset_at(LineEdge::Right) == offset {
        Some((index + 1, LineEdge::Left))
    } else {
        None
    };
    neighbour
        .and_then(|(index, side)| {
            let span = spans.get(index)?;
            (span.is_rtl == base_is_rtl).then(|| span.offset_at(side))
        })
        .unwrap_or(offset)
}

pub(super) fn inline_box_offset_at(line: &Line<'_, TextBrush>, inline: f32) -> Option<usize> {
    line.items().find_map(|item| match item {
        PositionedLayoutItem::InlineBox(inline_box)
            if inline_box.kind == InlineBoxKind::InFlow
                && inline_box.x <= inline
                && inline < inline_box.x + inline_box.width =>
        {
            Some(inline_box.index)
        }
        _ => None,
    })
}

pub(super) fn glyphs_reach_edge(line: &Line<'_, TextBrush>, edge: LineEdge) -> bool {
    edge_items(line).is_some_and(|(first, last)| match edge {
        LineEdge::Left => first.is_glyphs,
        LineEdge::Right => last.is_glyphs,
    })
}

impl Node {
    pub(super) fn before_hung_space(
        &self,
        line: &Line<'_, TextBrush>,
        text: &str,
        offset: usize,
    ) -> usize {
        let range = line.text_range();
        let is_soft_break = matches!(
            line.break_reason(),
            BreakReason::Regular | BreakReason::Emergency
        );
        if !is_soft_break || offset != range.end {
            return offset;
        }
        let ends_in_space = text
            .get(range.clone())
            .is_some_and(|line_text| line_text.ends_with(' '));
        let tree = self.tree();
        let collapses = line
            .items()
            .filter_map(|item| match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => Some(glyph_run),
                PositionedLayoutItem::InlineBox(_) => None,
            })
            .find(|glyph_run| glyph_run.text_range().end == range.end)
            .and_then(|glyph_run| tree.get(glyph_run.style().brush.id))
            .is_some_and(collapses_spaces);
        if ends_in_space && collapses {
            offset.saturating_sub(1)
        } else {
            offset
        }
    }

    pub(super) fn line_edge_offset(
        &self,
        line: &Line<'_, TextBrush>,
        edge: LineEdge,
    ) -> Option<usize> {
        let inline_layout = self.element_data()?.inline_layout_data.as_ref()?;
        let range = line.text_range();
        let is_start = matches!(edge, LineEdge::Left) != line.is_rtl();
        if is_start {
            return Some(range.start);
        }
        let text = &inline_layout.text;
        let ends_in_break = text
            .get(range.clone())
            .is_some_and(|line_text| line_text.ends_with('\n'));
        if ends_in_break {
            return Some(range.end.saturating_sub(1));
        }
        Some(self.before_hung_space(line, text, range.end))
    }

    pub(super) fn caret_target(&self, x: f32, y: f32) -> CaretTarget {
        let Some(geometry) = self.span_geometry() else {
            return CaretTarget::Shift((0.0, 0.0));
        };
        let (x, y) = (x + geometry.inset.left, y + geometry.inset.top);
        if let Some(node) = self
            .inline_hit(
                x,
                y,
                &super::hit_probe::HitProbe::device_pixel(f64::from(geometry.scale)),
                super::inline_hit::InlineScope::All,
            )
            .filter(|hit| hit.is_text)
            .and_then(|hit| self.tree().get(hit.node_id))
        {
            return CaretTarget::Shift(node.inline_relative_shift());
        }
        self.line_caret_target(&geometry, x, y)
            .unwrap_or(CaretTarget::Shift((0.0, 0.0)))
    }

    fn line_caret_target(&self, geometry: &SpanGeometry, x: f32, y: f32) -> Option<CaretTarget> {
        let layout = &self.element_data()?.inline_layout_data.as_ref()?.layout;
        let (_, block) = self.inline_layout_point(
            geometry.block_offset,
            x - geometry.inset.left,
            y - geometry.inset.top,
        );
        let block = block * geometry.scale;
        let (_, line) = layout.line_for_offset(block)?;
        let axes = self.line_axes();
        let tree = self.tree();
        let mut runs = Vec::new();
        let last_index = line.items().count().saturating_sub(1);
        for (index, item) in line.items().enumerate() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };
            let edge = if index == 0 {
                Some(LineEdge::Left)
            } else if index == last_index {
                Some(LineEdge::Right)
            } else {
                None
            };
            let owner = glyph_run.style().brush.id;
            let shift = tree.get(owner)?.inline_relative_shift();
            let rect = self.glyph_run_rect(geometry, &glyph_run, shift);
            let (start, end, inline_shift) = match axes {
                LineAxes::Level => (rect.left, rect.right, (shift.0, 0.0)),
                LineAxes::Clockwise(_) => (rect.top, rect.bottom, (0.0, shift.1)),
                LineAxes::CounterClockwise(_) => (-rect.bottom, -rect.top, (0.0, shift.1)),
            };
            runs.push(CaretRun {
                chain: self.span_chain(owner)?,
                start,
                end,
                text: glyph_run.text_range(),
                shift: inline_shift,
                is_rtl: glyph_run.run().is_rtl(),
                edge,
                is_lone: last_index == 0,
            });
        }
        if runs.iter().all(|run| run.shift == (0.0, 0.0)) {
            return None;
        }
        let point = match axes {
            LineAxes::Level => x,
            LineAxes::Clockwise(_) => y,
            LineAxes::CounterClockwise(_) => -y,
        };
        let refs: Vec<&CaretRun> = runs.iter().collect();
        let text = &self.element_data()?.inline_layout_data.as_ref()?.text;
        match pick(&refs, 0, point)? {
            Picked::Offset(offset) => Some(CaretTarget::Offset(
                self.before_hung_space(&line, text, offset),
            )),
            Picked::Edge(edge) => Some(CaretTarget::Offset(self.line_edge_offset(&line, edge)?)),
            Picked::Leaf(shift) => Some(CaretTarget::Shift(shift)),
        }
    }
}
