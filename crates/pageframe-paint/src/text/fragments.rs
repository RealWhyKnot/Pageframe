use super::*;

pub(crate) struct InlineFragment {
    pub(crate) node_id: NodeId,
    pub(crate) color: Color,
    pub(crate) rect: Rect,
    pub(crate) is_boxed: bool,
    pub(crate) is_start: bool,
    pub(crate) is_end: bool,
    pub(crate) group: Option<NodeId>,
}

struct SpanBox {
    span: LineSpan<f64>,
    depth: usize,
    color: Color,
    is_boxed: bool,
    font_size: f64,
    baseline: f64,
    extent: Option<(f64, f64)>,
    is_own_font: bool,
    shift: Vec2,
    group: Option<NodeId>,
}

struct SpanItem {
    left: f64,
    right: f64,
    baseline: f64,
    font: Option<(NodeId, f64, f64, f64)>,
    opens: bool,
    closes: bool,
    is_reordered_space: bool,
}

struct InlineAncestor {
    node_id: NodeId,
    color: Color,
    is_boxed: bool,
    font_size: f64,
    shift: Vec2,
    is_faded: bool,
    is_positioned: bool,
}

fn is_block_level(doc: &BaseDocument, box_id: u64) -> bool {
    doc.get_node(NodeId::from_u64(box_id))
        .and_then(|node| node.primary_styles())
        .is_some_and(|styles| {
            styles.get_box().display.outside()
                == style::values::specified::box_::DisplayOutside::Block
        })
}

fn inline_ancestors(
    doc: &BaseDocument,
    start: Option<NodeId>,
    inline_root_id: NodeId,
) -> Vec<InlineAncestor> {
    let mut ancestors = Vec::new();
    let mut next_id = start;
    while let Some(node) = next_id
        .filter(|node_id| *node_id != inline_root_id)
        .and_then(|node_id| doc.get_node(node_id))
    {
        next_id = node.parent;
        let Some(styles) = node.primary_styles() else {
            break;
        };
        let display = styles.get_box().display;
        if display.is_contents() {
            continue;
        }
        if display != Display::Inline {
            break;
        }
        let current_color = styles.clone_color();
        let color = styles
            .get_background()
            .background_color
            .resolve_to_absolute(&current_color)
            .as_srgb_color();
        ancestors.push(InlineAncestor {
            node_id: node.id,
            color,
            is_boxed: pageframe_dom::util::inline_box_is_decorated(&styles),
            font_size: f64::from(styles.get_font().font_size.used_size.0.px()),
            shift: relative_shift(node),
            is_faded: styles.get_effects().opacity < 1.0,
            is_positioned: is_positioned(&styles),
        });
    }
    ancestors
}

pub(super) fn relative_shift(node: &pageframe_dom::Node) -> Vec2 {
    let (x, y) = node.inline_relative_offset();
    Vec2::new(f64::from(x), f64::from(y))
}

pub(super) fn covers_its_start(doc: &BaseDocument, owner: NodeId, is_rtl: bool) -> bool {
    let Some(styles) = doc.get_node(owner).and_then(|node| node.primary_styles()) else {
        return false;
    };
    let border = styles.get_border();
    let (line, width, color) = if is_rtl {
        (
            border.border_right_style,
            &border.border_right_width,
            &border.border_right_color,
        )
    } else {
        (
            border.border_left_style,
            &border.border_left_width,
            &border.border_left_color,
        )
    };
    let has_border = !line.none_or_hidden()
        && width.0.to_f32_px() > 0.0
        && crate::color::border_side_color(&styles, color, line).components[3] >= 1.0;
    let background = styles
        .get_background()
        .background_color
        .resolve_to_absolute(&styles.clone_color())
        .as_srgb_color();
    has_border || background.components[3] >= 1.0
}

pub(super) fn push_spacer_holes(
    glyph_run: &parley::GlyphRun<'_, TextBrush>,
    run_text: &str,
    snap_origin: Option<f64>,
    holes: &mut SmallVec<[(f64, f64); 2]>,
) {
    let is_spacer = |ch: &char| pageframe_dom::util::is_inline_spacer(*ch);
    let leading = run_text.chars().take_while(is_spacer).count();
    let trailing = run_text.chars().rev().take_while(is_spacer).count();
    if leading + trailing == 0 {
        return;
    }
    let is_rtl = glyph_run.run().is_rtl();
    let (first, last) = if is_rtl {
        (trailing, leading)
    } else {
        (leading, trailing)
    };
    let opens_right = glyph_run
        .run()
        .inline_edge()
        .map_or(is_rtl, |edge| edge.is_rtl);
    let count = glyph_run.positioned_glyphs().count();
    for (index, glyph) in glyph_run.positioned_glyphs().enumerate() {
        let is_mark = index < first || index + last >= count;
        if !is_mark || glyph.advance <= 0.0 {
            continue;
        }
        let mut left = f64::from(glyph.x);
        let mut right = left + f64::from(glyph.advance);
        let from_start = if is_rtl { count - 1 - index } else { index };
        let mark = if from_start < leading {
            run_text.chars().nth(from_start)
        } else {
            run_text.chars().rev().nth(count - 1 - from_start)
        };
        if let (Some(origin), Some(mark)) = (snap_origin, mark)
            && pageframe_dom::util::INLINE_EDGE_START.contains(mark)
        {
            if opens_right {
                right = right.max((right + origin).round() - origin);
            } else {
                left = left.min((left + origin).round() - origin);
            }
        }
        holes.push((left, right));
    }
}

pub(crate) fn inline_fragments<'a>(
    lines: impl Iterator<Item = Line<'a, TextBrush>>,
    doc: &BaseDocument,
    inline_root_id: NodeId,
    text: &str,
    scale: f64,
) -> Vec<InlineFragment> {
    let mut fragments = Vec::new();
    let mut primary_extents = HashMap::new();
    let mut font_ratios: Option<(f64, f64)> = None;
    let Some(root) = doc.get_node(inline_root_id) else {
        return fragments;
    };
    let (clone_basis, scale_f32) = root
        .element_data()
        .and_then(|element| element.inline_layout_data.as_ref())
        .map_or((0.0, 1.0), |layout| {
            (layout.percent_basis, layout.layout.scale())
        });
    for line in lines {
        let line_baseline = f64::from(line.metrics().baseline);
        let mut boxes: Vec<SpanBox> = Vec::new();
        let trim = LineTrim::new(root, &line, text);
        for item in line.items() {
            let (start, span_item) = match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => {
                    let run = glyph_run.run();
                    let metrics = run.font_metrics();
                    let left = f64::from(glyph_run.offset());
                    let font_size = f64::from(run.font_size());
                    let ascent = f64::from(metrics.ascent);
                    let descent = f64::from(metrics.descent);
                    if font_size > 0.0 {
                        font_ratios = Some((ascent / font_size, descent / font_size));
                    }
                    let owner = glyph_run.style().brush.id;
                    let run_text = text.get(run.text_range()).unwrap_or_default();
                    (
                        Some(owner),
                        SpanItem {
                            left,
                            right: left + f64::from(glyph_run.advance()),
                            baseline: f64::from(glyph_run.baseline()),
                            font: Some((owner, ascent, descent, font_size)),
                            opens: run_text.starts_with(pageframe_dom::util::INLINE_EDGE_START),
                            closes: run_text.ends_with(pageframe_dom::util::INLINE_EDGE_END),
                            is_reordered_space: is_reordered_preserved_space(
                                root, &glyph_run, text,
                            ),
                        },
                    )
                }
                PositionedLayoutItem::InlineBox(inline_box)
                    if inline_box.kind == parley::InlineBoxKind::InFlow
                        && !is_block_level(doc, inline_box.id) =>
                {
                    let left = f64::from(inline_box.x);
                    (
                        doc.get_node(NodeId::from_u64(inline_box.id))
                            .and_then(|node| node.parent),
                        SpanItem {
                            left,
                            right: left + f64::from(inline_box.width),
                            baseline: line_baseline,
                            font: None,
                            opens: false,
                            closes: false,
                            is_reordered_space: false,
                        },
                    )
                }
                PositionedLayoutItem::InlineBox(_) => continue,
            };
            let mut span_item = span_item;
            if let (Some(left), Some(right)) = (span_item.left.to_f32(), span_item.right.to_f32())
                && trim.is_hung(left, right)
            {
                span_item.left = f64::INFINITY;
                span_item.right = f64::NEG_INFINITY;
            }

            let ancestors = inline_ancestors(doc, start, inline_root_id);
            let depth_of_first = ancestors.len();
            let mut outer_shift = ancestors
                .iter()
                .fold(Vec2::ZERO, |sum, ancestor| sum + ancestor.shift);
            let groups: Vec<Option<NodeId>> = (0..ancestors.len())
                .map(|index| {
                    ancestors
                        .get(index..)
                        .unwrap_or_default()
                        .iter()
                        .find(|ancestor| ancestor.is_faded || ancestor.is_positioned)
                        .map(|ancestor| ancestor.node_id)
                })
                .collect();
            for ((index, ancestor), group) in ancestors.into_iter().enumerate().zip(groups) {
                let shift = outer_shift * scale;
                outer_shift -= ancestor.shift;
                if ancestor.color == Color::TRANSPARENT
                    && !ancestor.is_boxed
                    && !ancestor.is_faded
                    && !ancestor.is_positioned
                {
                    continue;
                }
                let node_id = ancestor.node_id;
                let is_own = span_item.font.is_some_and(|(owner, ..)| owner == node_id);
                let position = boxes
                    .iter()
                    .rposition(|span_box| span_box.span.node_id == node_id)
                    .filter(|&index| {
                        boxes[index]
                            .span
                            .continues(span_item.left, span_item.is_reordered_space)
                    })
                    .unwrap_or_else(|| {
                        boxes.push(SpanBox {
                            span: LineSpan {
                                node_id,
                                left: span_item.left,
                                right: span_item.right,
                                is_start: false,
                                is_end: false,
                                has_reordered_space: false,
                            },
                            depth: depth_of_first - index,
                            color: ancestor.color,
                            is_boxed: ancestor.is_boxed,
                            font_size: ancestor.font_size * scale,
                            baseline: line_baseline,
                            extent: None,
                            is_own_font: false,
                            shift,
                            group,
                        });
                        boxes.len() - 1
                    });
                let span_box = &mut boxes[position];
                let span = &mut span_box.span;
                span.left = span.left.min(span_item.left);
                span.right = span.right.max(span_item.right);
                span.is_start |= is_own && span_item.opens;
                span.is_end |= is_own && span_item.closes;
                span.has_reordered_space |= span_item.is_reordered_space;
                let Some((_, ascent, descent, run_font_size)) = span_item.font else {
                    continue;
                };
                if span_box.is_own_font {
                    continue;
                }
                if is_own {
                    span_box.is_own_font = true;
                    span_box.baseline = span_item.baseline;
                    span_box.extent = Some((ascent, descent));
                } else if span_box.extent.is_none() && run_font_size > 0.0 {
                    let ratio = span_box.font_size / run_font_size;
                    span_box.extent = Some((ascent * ratio, descent * ratio));
                }
            }
        }

        boxes.sort_by_key(|span_box| span_box.depth);
        let mut spans: Vec<_> = boxes.iter().map(|span_box| span_box.span).collect();
        settle_line_spans(root, &mut spans, &trim, clone_basis, scale_f32);
        for (span_box, span) in boxes.into_iter().zip(spans) {
            let primary = primary_extents.entry(span.node_id).or_insert_with(|| {
                doc.inline_font_ascent_descent(span.node_id)
                    .map(|(ascent, descent)| (f64::from(ascent), f64::from(descent)))
            });
            let extent = primary.or(span_box.extent).or_else(|| {
                font_ratios.map(|(ascent, descent)| {
                    (ascent * span_box.font_size, descent * span_box.font_size)
                })
            });
            let Some((ascent, descent)) = extent else {
                continue;
            };
            fragments.push(InlineFragment {
                node_id: span.node_id,
                color: span_box.color,
                rect: Rect::new(
                    span.left,
                    span_box.baseline - ascent.round(),
                    span.right,
                    span_box.baseline + descent.round(),
                ) + span_box.shift,
                is_boxed: span_box.is_boxed,
                is_start: span.is_start,
                is_end: span.is_end,
                group: span_box.group,
            });
        }
    }
    fragments
}
