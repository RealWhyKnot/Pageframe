use crate::layout::resolve_calc_value;
use crate::node::{Node, SpecialElementData};
use markup5ever::local_name;
use parley::AlignmentOptions;
use stylo_taffy::CopyStyleGetters;
use taffy::prelude::TaffyAuto;
use taffy::{
    CoreStyle, LayoutInput, LayoutOutput, MaybeResolve, ResolveOrZero, Size, compute_leaf_layout,
};

pub(crate) fn layout(node: &mut Node, mut inputs: LayoutInput) -> LayoutOutput {
    let Some(element) = node.element_data_mut() else {
        return LayoutOutput::HIDDEN;
    };
    if !matches!(element.special_data, SpecialElementData::ImageFallback(_)) {
        return LayoutOutput::HIDDEN;
    }
    let SpecialElementData::ImageFallback(mut fallback) = element.special_data.take() else {
        return LayoutOutput::HIDDEN;
    };
    let scale = fallback.layout.layout.scale();
    let source = element.attr(local_name!("src"));
    let has_source = source.is_some();
    let has_alt = element
        .attr(local_name!("alt"))
        .is_some_and(|alt| !alt.is_empty());
    let missing_source = source.is_none();
    let source_style = node.layout_style();
    let mut style: taffy::Style = taffy::Style {
        size: source_style.size(),
        min_size: source_style.min_size(),
        max_size: source_style.max_size(),
        padding: source_style.padding(),
        border: source_style.border(),
        margin: source_style.margin(),
        box_sizing: source_style.box_sizing(),
        aspect_ratio: source_style.aspect_ratio(),
        overflow: source_style.overflow(),
        ..Default::default()
    };
    let inline = node
        .display_style()
        .is_some_and(|display| display.is_inline_flow());
    let both = !style.size.width.is_auto() && !style.size.height.is_auto();
    if !fallback.quirks && !fallback.loading && inline && (has_alt || (has_source && !both)) {
        style.size = Size::AUTO;
        style.aspect_ratio = None;
        inputs.known_dimensions = Size::NONE;
    }
    let specified = style
        .size()
        .maybe_resolve(inputs.parent_size, resolve_calc_value);
    let sized = specified.width.is_some() || specified.height.is_some();
    let square = fallback.quirks && sized && (has_source || has_alt);
    let block = node.display_style().is_some_and(|display| {
        display.outside() == style::values::specified::box_::DisplayOutside::Block
    });
    let alignment = node
        .primary_styles()
        .map(|s| crate::stylo_to_parley::text_align(s.clone_text_align()))
        .unwrap_or(parley::Alignment::Start);
    fallback.inset = if sized && !fallback.loading { 2.0 } else { 0.0 };
    let inset = fallback.inset * 2.0;
    let natural = if fallback.loading {
        0.0
    } else {
        fallback.layout.content_widths().max / scale
    };
    let pb = style
        .padding()
        .resolve_or_zero(inputs.parent_size.width, resolve_calc_value)
        + style
            .border()
            .resolve_or_zero(inputs.parent_size.width, resolve_calc_value);
    let mut output = compute_leaf_layout(inputs, &style, resolve_calc_value, |known, available| {
        let known = Size {
            width: known.width.or(specified.width),
            height: known.height.or(specified.height),
        };
        let width = known.width.unwrap_or_else(|| {
            if square {
                known.height.unwrap_or(natural + inset)
            } else if block && (!missing_source || has_alt) {
                available.width.into_option().unwrap_or(natural)
            } else {
                natural.min(available.width.into_option().unwrap_or(natural))
            }
        });
        fallback
            .layout
            .layout
            .break_all_lines(Some(((width - inset).max(0.0)) * scale));
        let height = known.height.unwrap_or_else(|| {
            if square {
                width
            } else if fallback.loading || natural == 0.0 {
                0.0
            } else if !has_alt {
                16.0
            } else {
                fallback.layout.layout.height() / scale + inset
            }
        });
        Size { width, height }
    });
    let content_width = (output.size.width - pb.horizontal_axis_sum() - inset).max(0.0);
    fallback
        .layout
        .layout
        .break_all_lines(Some(content_width * scale));
    fallback
        .layout
        .layout
        .align(alignment, AlignmentOptions::default());
    if has_alt && !sized {
        output.baselines.first = fallback
            .layout
            .layout
            .lines()
            .next()
            .map(|line| line.metrics().baseline / scale);
        output.baselines.last = output.baselines.first;
    }
    drop(source_style);
    if let Some(element) = node.element_data_mut() {
        element.special_data = SpecialElementData::ImageFallback(fallback);
    }
    output
}

pub(crate) fn prepare(doc: &mut crate::BaseDocument, node_id: crate::NodeId) {
    let node = &doc.nodes[node_id];
    let Some(element) = node.element_data() else {
        return;
    };
    let SpecialElementData::ImageFallback(fallback) = &element.special_data else {
        return;
    };
    let mut text_style = node
        .primary_styles()
        .map(|s| crate::stylo_to_parley::style(node_id, &s))
        .unwrap_or_default();
    text_style.vertical_align = parley::VerticalAlign::BASELINE;
    let alt = element.attr(local_name!("alt"));
    let icon = !fallback.loading
        && match alt {
            Some(alt) => !alt.is_empty(),
            None => element
                .attr(local_name!("src"))
                .is_some_and(|src| !src.is_empty()),
        };
    let text = if fallback.loading {
        ""
    } else {
        alt.unwrap_or("")
    }
    .to_owned();
    let scale = doc.viewport.scale();
    let mut layout = crate::node::TextLayout::new();
    let Ok(mut fonts) = doc.font_ctx.lock() else {
        return;
    };
    let mut root_style = text_style.clone();
    root_style.font_size = 0.0;
    root_style.line_height = parley::LineHeight::Absolute(0.0);
    let mut builder = doc
        .layout_ctx
        .tree_builder(&mut fonts, scale, true, &root_style);
    builder.set_line_break_override(Some(crate::util::LINE_BREAK_OVERRIDE));
    if icon {
        builder.push_inline_box(parley::InlineBox {
            id: node_id.as_u64(),
            kind: parley::InlineBoxKind::InFlow,
            index: 0,
            width: 16.0 * scale,
            height: 16.0 * scale,
            baseline: Some(16.0 * scale),
            vertical_align: parley::VerticalAlign::TOP,
        });
    }
    builder.push_style_span(text_style);
    builder.push_text(&text);
    builder.pop_style_span();
    layout.text = builder.build_into(&mut layout.layout);
    drop(fonts);
    if let Some(element) = doc.nodes[node_id].element_data_mut()
        && let SpecialElementData::ImageFallback(fallback) = &mut element.special_data
    {
        fallback.layout = layout;
        fallback.quirks = doc.in_quirks_mode;
    }
}
