use super::inline_hit::{SpanChain, SpanGeometry};
use super::node::{LineAxes, Node};
use pageframe_traits::node_id::NodeId;
use parley::{BreakReason, Brush, Line, PositionedLayoutItem};
use std::ops::Range;

pub(super) enum CaretTarget {
    Shift((f32, f32)),
    Offset(usize),
}

struct CaretRun {
    chain: SpanChain,
    start: f32,
    end: f32,
    text: Range<usize>,
    shift: (f32, f32),
    is_rtl: bool,
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

fn pick(runs: &[&CaretRun], depth: usize, point: f32) -> Option<CaretTarget> {
    let groups = group_at(runs, depth);
    if let Some(group) = groups
        .iter()
        .find(|group| point >= group.start && point < group.end)
    {
        if group.is_leaf {
            return group.runs.first().map(|run| CaretTarget::Shift(run.shift));
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
    Some(CaretTarget::Offset(if is_before == is_rtl {
        text_end
    } else {
        text_start
    }))
}

pub(super) fn before_hung_space<B: Brush>(line: &Line<'_, B>, text: &str, offset: usize) -> usize {
    let range = line.text_range();
    let is_soft_break = matches!(
        line.break_reason(),
        BreakReason::Regular | BreakReason::Emergency
    );
    if !is_soft_break || offset != range.end {
        return offset;
    }
    let ends_in_space = text
        .get(range)
        .is_some_and(|line_text| line_text.ends_with(' '));
    if ends_in_space {
        offset.saturating_sub(1)
    } else {
        offset
    }
}

impl Node {
    pub(super) fn caret_target(&self, x: f32, y: f32) -> CaretTarget {
        let Some(geometry) = self.span_geometry() else {
            return CaretTarget::Shift((0.0, 0.0));
        };
        let (x, y) = (x + geometry.inset.left, y + geometry.inset.top);
        if let Some(node) = self
            .inline_hit(x, y)
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
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
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
            CaretTarget::Offset(offset) => {
                Some(CaretTarget::Offset(before_hung_space(&line, text, offset)))
            }
            shift @ CaretTarget::Shift(_) => Some(shift),
        }
    }
}
