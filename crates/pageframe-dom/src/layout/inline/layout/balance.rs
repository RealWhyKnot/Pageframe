use crate::node::TextBrush;
use parley::{BreakReason, Layout, YieldData};

const BALANCE_MAX_LINES: usize = 6;

const BALANCE_PRECISION: f32 = 1.0 / 64.0;

const BALANCE_MAX_SEGMENTS: usize = 1024;

struct BalanceSegment {
    pos_if_break: f32,
    pos_no_break: f32,
    text_end: usize,
}

fn break_balance_paragraph(layout: &mut Layout<TextBrush>, max_advance: f32, advances: &[f32]) {
    let mut breaker = layout.break_lines();
    breaker.state_mut().set_layout_max_advance(max_advance);
    for advance in advances
        .iter()
        .copied()
        .chain(std::iter::repeat(max_advance))
    {
        breaker.state_mut().set_line_max_advance(advance);
        let Some(YieldData::LineBreak(line)) = breaker.break_next() else {
            break;
        };
        if line.reason == BreakReason::Explicit {
            break;
        }
    }
    breaker.finish();
}

pub(super) fn balanced_line_advances(
    layout: &mut Layout<TextBrush>,
    max_advance: f32,
) -> Option<(Vec<f32>, bool)> {
    break_balance_paragraph(layout, max_advance, &[]);
    let line_count = layout.len();
    if !(2..=BALANCE_MAX_LINES).contains(&line_count) {
        return None;
    }
    let is_rtl = layout.get(0)?.is_rtl();
    let has_forced_break = layout.get(line_count - 1)?.break_reason() == BreakReason::Explicit;

    break_balance_paragraph(layout, 0.0, &[]);
    let min_advance = has_forced_break.then(|| {
        layout
            .lines()
            .map(|line| {
                let metrics = line.metrics();
                metrics.indent + metrics.advance - metrics.hanging_advance
            })
            .fold(0.0, f32::max)
    });
    let advances = evenest_line_advances(layout, max_advance, line_count).unwrap_or_else(|| {
        let min_advance = min_advance.unwrap_or_else(|| layout.calculate_content_widths().min);
        vec![narrowest_max_advance(layout, max_advance, min_advance, line_count); line_count]
    });
    Some((advances, is_rtl))
}

fn narrowest_max_advance(
    layout: &mut Layout<TextBrush>,
    max_advance: f32,
    min_advance: f32,
    line_count: usize,
) -> f32 {
    let mut narrow = min_advance.min(max_advance);
    let mut wide = max_advance;
    while wide - narrow > BALANCE_PRECISION {
        let middle = (narrow + wide) / 2.0;
        break_balance_paragraph(layout, middle, &[]);
        if layout.len() <= line_count {
            wide = middle;
        } else {
            narrow = middle;
        }
    }
    wide
}

fn evenest_line_advances(
    layout: &mut Layout<TextBrush>,
    max_advance: f32,
    line_count: usize,
) -> Option<Vec<f32>> {
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
        .break_candidate_positions()
        .map(|(line, pos_if_break, pos_no_break)| BalanceSegment {
            pos_if_break,
            pos_no_break,
            text_end: line.text_range().end,
        })
        .collect();

    let line_ends = evenest_line_ends(&segments, indent, max_advance, line_count)?;
    let mut line_start = 0;
    let mut advances = Vec::with_capacity(line_count);
    for line_end in &line_ends {
        let width = balance_line_width(&segments, line_start, *line_end, indent);
        advances.push((width + BALANCE_PRECISION).min(max_advance));
        line_start = *line_end;
    }

    break_balance_paragraph(layout, max_advance, &advances);

    let breaks_as_planned = layout.len() == line_count
        && layout
            .lines()
            .zip(&line_ends)
            .all(|(line, line_end)| line.text_range().end == segments[*line_end - 1].text_end);
    breaks_as_planned.then_some(advances)
}

fn balance_line_width(segments: &[BalanceSegment], start: usize, end: usize, indent: f32) -> f32 {
    let start_position = start
        .checked_sub(1)
        .map_or(-indent, |i| segments[i].pos_no_break);
    segments[end - 1].pos_if_break - start_position
}

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
                let width = balance_line_width(segments, line_start, line_end, indent);
                if width > max_advance + BALANCE_PRECISION && line_end - line_start > 1 {
                    break;
                }
                let slack = max_advance + BALANCE_PRECISION - width;
                let cost = costs[line - 1][line_start] + slack.powi(2);
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

pub(super) fn balanced_line_box(
    balanced_advances: Option<&[f32]>,
    line_index: usize,
    max_advance: f32,
    end_share: f32,
) -> Option<(f32, f32)> {
    let advance = *balanced_advances?.get(line_index)?;
    Some(((max_advance - advance).max(0.0) * end_share, advance))
}
