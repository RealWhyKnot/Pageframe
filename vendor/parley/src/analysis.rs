// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{Brush, InlineBoxKind, LayoutContext, WhiteSpaceCollapse};

use parley_engine::break_overrides::LineBreakOverrideFn;

use parley_engine::{AnalysisOptions, LineBreakConfig};

use parlance::BaseDirection;

pub(crate) fn analyze_text<B: Brush>(
    lcx: &mut LayoutContext<B>,
    text: &str,
    base_direction: BaseDirection,
    line_break_override: Option<&LineBreakOverrideFn>,
) {
    let text = if text.is_empty() { " " } else { text };

    // Collect the style runs' line break configurations. Gaps use the default configuration, so
    // only non-default configurations need an entry, and adjacent equal configurations are merged.
    //
    // Separately, collect the `break-spaces` runs, which allow wrapping after each preserved space
    // or tab. Adjacent runs are merged, so that an opportunity is not created before the first
    // space of a sequence spanning a style boundary.
    lcx.line_break.clear();
    lcx.break_spaces.clear();
    for style_run in lcx.style_runs.iter() {
        let style = &lcx.style_table[style_run.style_index as usize];
        let line_break = LineBreakConfig {
            word_break: style.word_break,
            line_break: style.line_break,
            language: style.locale,
        };
        if line_break != LineBreakConfig::default() {
            match lcx.line_break.last_mut() {
                Some((range, last))
                    if range.end == style_run.range.start && *last == line_break =>
                {
                    range.end = style_run.range.end;
                }
                _ => lcx.line_break.push((style_run.range.clone(), line_break)),
            }
        }
        if style.white_space_collapse == WhiteSpaceCollapse::BreakSpaces {
            match lcx.break_spaces.last_mut() {
                Some(last) if last.end == style_run.range.start => last.end = style_run.range.end,
                _ => lcx.break_spaces.push(style_run.range.clone()),
            }
        }
    }

    lcx.objects.clear();
    lcx.objects.extend(
        lcx.inline_boxes
            .iter()
            .filter(|layout_box| layout_box.inline_box.kind == InlineBoxKind::InFlow)
            .map(|layout_box| layout_box.inline_box.index),
    );
    let options = AnalysisOptions {
        base_direction,
        line_break: &lcx.line_break,
        break_spaces: &lcx.break_spaces,
        objects: &lcx.objects,
        line_break_override,
    };
    lcx.analyzer.analyze(text, &options, &mut lcx.analysis);
}

pub(crate) fn collect_zwsp_positions(
    positions: &mut alloc::vec::Vec<(usize, crate::layout::data::ZwspPosition)>,
    analysis: &parley_engine::Analysis,
    text: &str,
    inline_ends: &[usize],
    bidi_controls: &[usize],
) {
    use crate::layout::data::ZwspPosition;

    if !text.contains('\u{200b}') {
        return;
    }

    let mut ends = inline_ends.iter().copied().peekable();
    let mut controls = bidi_controls.iter().copied().peekable();
    let mut after_zwsp = None;
    let mut has_inline_end = false;
    let chars = text.char_indices().chain([(text.len(), '\n')]);
    for (index, (offset, character)) in chars.enumerate() {
        let inline_end = ends.next_if_eq(&offset).is_some();
        let bidi_control = controls.next_if_eq(&offset).is_some();
        if inline_end || bidi_control {
            has_inline_end |= inline_end;
            continue;
        }
        if let Some((first_index, first_byte)) = after_zwsp.take()
            && has_inline_end
            && analysis.char_info()[first_index..index]
                .iter()
                .any(|character| character.is_soft_wrap_opportunity())
        {
            positions.extend(
                text[first_byte..offset]
                    .char_indices()
                    .map(|(delta, _)| (first_byte + delta, ZwspPosition::SuppressBreak)),
            );
            if !matches!(
                character,
                ' ' | '\u{200b}'
                    | '\n'
                    | '\r'
                    | '\u{b}'
                    | '\u{c}'
                    | '\u{85}'
                    | '\u{2028}'
                    | '\u{2029}'
            ) {
                positions.push((offset, ZwspPosition::AllowBreak));
            }
        }
        has_inline_end = false;
        if character == '\u{200b}' {
            positions.push((offset, ZwspPosition::Author));
            after_zwsp = Some((index + 1, offset + character.len_utf8()));
        }
    }
}
