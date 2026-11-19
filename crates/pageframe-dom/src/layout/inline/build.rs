use markup5ever::local_name;
use pageframe_traits::node_id::NodeId;
use parley::{
    FontContext, InlineBox, InlineBoxKind, LayoutContext, TreeBuilder, WhiteSpaceCollapse,
};
use style::{
    computed_values::position::T as PositionProperty,
    properties::ComputedValues,
    values::{
        computed::{BoxDecorationBreak, Display, Float},
        specified::box_::{DisplayInside, DisplayOutside},
    },
};
use stylo_taffy::CopyStyleGetters;

use crate::{
    NodeData,
    layout::list::BULLET_FONT_FAMILY,
    layout::replaced::is_inline_box_element,
    layout::text_transform::{CaseTransform, TextTransformer},
    node::{ListItemLayout, ListItemLayoutPosition, Marker, TextBrush, TextLayout},
    stylo_to_parley,
};

const DISCLOSURE_MARKER_GAP_EM: f32 = 0.4;

struct InlineEdges {
    owner: u64,
    is_rtl: bool,
    has_edge_marks: bool,
    margin_start: f32,
    inset_start: f32,
    inset_end: f32,
    margin_end: f32,
    has_percent_margin: bool,
}

impl InlineEdges {
    fn has_margin_marks(&self) -> bool {
        self.has_percent_margin
            || self.margin_start.abs() >= f32::EPSILON
            || self.margin_end.abs() >= f32::EPSILON
    }

    fn has_end_mark(&self) -> bool {
        self.has_edge_marks || self.has_margin_marks()
    }
}

fn trailing_space(nodes: &crate::NodeTree, node_id: NodeId) -> Option<bool> {
    let node = &nodes[node_id];
    match &node.data {
        NodeData::Text(data) => {
            let last = data.content.chars().next_back()?;
            let wraps = node
                .parent
                .and_then(|parent| nodes[parent].primary_styles())
                .is_none_or(|style| {
                    style.clone_text_wrap_mode() == stylo_to_parley::stylo::TextWrapMode::Wrap
                });
            Some(wraps && matches!(last, ' ' | '\t' | '\n' | '\r' | '\u{c}'))
        }
        NodeData::Element(element_data) | NodeData::AnonymousBlock(element_data) => {
            let display = node.display_style()?;
            if display.is_none() {
                return None;
            }
            let is_flow = display.is_contents()
                || (display.outside() == DisplayOutside::Inline
                    && display.inside() == DisplayInside::Flow);
            let is_span = is_flow
                && !is_inline_box_element(&element_data.name.local)
                && !node.is_ruby_box()
                && element_data.name.local != local_name!("br");
            if !is_span {
                return Some(false);
            }
            node.after()
                .into_iter()
                .chain(node.children.iter().rev().copied())
                .find_map(|child| trailing_space(nodes, child))
        }
        _ => None,
    }
}

struct EdgeBasis {
    width: f32,
    has_percent: bool,
    has_clone: bool,
}

fn inline_edges(
    style: &ComputedValues,
    owner: u64,
    basis: &mut EdgeBasis,
    scale: f32,
) -> InlineEdges {
    basis.has_clone |= style.get_border().box_decoration_break == BoxDecorationBreak::Clone;
    let writing_mode = style.writing_mode;
    let start = crate::util::inline_side(
        style,
        writing_mode.inline_start_physical_side(),
        basis.width,
        scale,
    );
    let end = crate::util::inline_side(
        style,
        writing_mode.inline_end_physical_side(),
        basis.width,
        scale,
    );
    let has_percent_margin = start.has_percent_margin || end.has_percent_margin;
    basis.has_percent |= has_percent_margin || crate::util::has_percent_padding(style);
    InlineEdges {
        owner,
        is_rtl: !writing_mode.is_bidi_ltr(),
        has_edge_marks: crate::util::inline_box_paints(style),
        margin_start: start.margin,
        inset_start: start.inset,
        inset_end: end.inset,
        margin_end: end.margin,
        has_percent_margin,
    }
}

fn push_spacer(
    builder: &mut TreeBuilder<TextBrush>,
    edges: &InlineEdges,
    spacer: &str,
    width: f32,
) {
    let is_start =
        spacer == crate::util::INLINE_EDGE_START || spacer == crate::util::INLINE_MARGIN_START;
    let run_mark = if is_start { 1.0 } else { 2.0 };
    builder.push_style_modification_span(&[
        parley::StyleProperty::LetterSpacing(width),
        parley::StyleProperty::WordSpacing(run_mark),
        parley::StyleProperty::InlineEdge(Some(parley::InlineEdge {
            owner: edges.owner,
            is_start,
            is_rtl: edges.is_rtl,
        })),
    ]);
    builder.push_text(spacer);
    builder.pop_style_span();
}

fn push_margin_spacer(
    builder: &mut TreeBuilder<TextBrush>,
    edges: &InlineEdges,
    spacer: &str,
    width: f32,
) {
    if edges.has_margin_marks() {
        push_spacer(builder, edges, spacer, width);
    }
}

fn push_edge_spacer(
    builder: &mut TreeBuilder<TextBrush>,
    edges: &InlineEdges,
    spacer: &str,
    width: f32,
) {
    if edges.has_edge_marks {
        push_spacer(builder, edges, spacer, width);
    }
}

fn shows_dropdown_value(nodes: &crate::NodeTree, root: NodeId) -> bool {
    let host = match nodes[root].parent {
        Some(parent) if nodes[parent].before() == Some(root) => parent,
        _ => root,
    };
    let select = if nodes[host]
        .data
        .is_element_with_tag_name(&local_name!("option"))
    {
        nodes[host].parent.and_then(|parent| {
            if nodes[parent]
                .data
                .is_element_with_tag_name(&local_name!("optgroup"))
            {
                nodes[parent].parent
            } else {
                Some(parent)
            }
        })
    } else {
        Some(host)
    };
    select
        .and_then(|select| nodes[select].element_data())
        .is_some_and(|element| {
            element.name.local == local_name!("select")
                && crate::select_list::select_list_rows(element).is_none()
        })
}

fn dropdown_line_height(
    font_ctx: &mut FontContext,
    computed: &ComputedValues,
    scale: f32,
) -> Option<f32> {
    let font_size = computed.slow_clone_font_size().used_size().px();
    let metrics =
        crate::font_metrics::form_control_metrics(font_ctx, computed.get_font(), font_size, scale)?;
    let text_height = (metrics.ascent.round() + metrics.descent.round()) / scale;
    Some(text_height + 2.0 * scale.floor() / scale)
}

pub(crate) fn build_inline_layout_into(
    nodes: &crate::NodeTree,
    layout_ctx: &mut LayoutContext<TextBrush>,
    font_ctx: &mut FontContext,
    text_layout: &mut TextLayout,
    scale: f32,
    inline_context_root_node_id: NodeId,
) {
    // Get the inline context's root node's text styles
    let root_node = &nodes[inline_context_root_node_id];
    let root_node_style = root_node.primary_styles().or_else(|| {
        root_node
            .parent
            .and_then(|parent_id| nodes[parent_id].primary_styles())
    });

    let parley_style = root_node_style
        .as_ref()
        .map(|s| {
            let mut root_style = parley::TextStyle {
                vertical_align: parley::VerticalAlign::BASELINE,
                ..stylo_to_parley::style(inline_context_root_node_id, s)
            };
            if shows_dropdown_value(nodes, inline_context_root_node_id) {
                root_style.line_height = dropdown_line_height(font_ctx, s, scale).map_or(
                    parley::LineHeight::MetricsRelative(1.0),
                    parley::LineHeight::Absolute,
                );
            }
            root_style
        })
        .unwrap_or_else(|| parley::TextStyle {
            white_space_collapse: WhiteSpaceCollapse::Collapse,
            ..Default::default()
        });

    // Create a parley tree builder
    let mut builder = layout_ctx.tree_builder(font_ctx, scale, true, &parley_style);
    builder.set_line_break_override(Some(crate::util::LINE_BREAK_OVERRIDE));
    if let Some(style) = root_node_style.as_deref() {
        builder.set_base_direction(stylo_to_parley::base_direction(
            style.clone_direction(),
            style.clone_unicode_bidi(),
        ));
    }

    let text_transform = root_node_style
        .as_deref()
        .map(|s| CaseTransform::from_style(s))
        .unwrap_or(CaseTransform::NONE);

    // Render position-inside list items
    if let Some(ListItemLayout {
        marker,
        position: ListItemLayoutPosition::Inside,
        ..
    }) = root_node
        .element_data()
        .and_then(|el| el.list_item_data.as_deref())
    {
        match marker {
            Marker::Symbol(symbol) => {
                let mut marker_style = parley_style.clone();
                let advance = symbol.size() + marker_style.font_size * scale;
                marker_style.letter_spacing = (advance - symbol.space_advance) / scale;
                builder.push_style_span(marker_style);
                builder.push_text(&marker.inside_text());
                builder.pop_style_span();
            }
            // Bullet glyphs live in the bundled bullet font. The position-outside
            // path already asks for it; without the same span here a marker like
            // disclosure-closed (U+25B8) falls back to the element's own font and
            // renders as a missing glyph.
            Marker::Char(_) => {
                let mut marker_style = parley_style.clone();
                marker_style.font_family = BULLET_FONT_FAMILY.into();
                if marker.is_disclosure() {
                    marker_style.letter_spacing =
                        DISCLOSURE_MARKER_GAP_EM * marker_style.font_size / scale;
                }
                builder.push_style_span(marker_style);
                builder.push_text(&marker.inside_text());
                builder.pop_style_span();
            }
            Marker::String(str) => builder.push_text(str),
        }
    };
    // The marker is a separate box, so words in the content don't continue from it.
    let mut text_transformer = TextTransformer::default();
    text_transformer.word_break(&builder);
    let mut edge_basis = EdgeBasis {
        width: text_layout.percent_basis,
        has_percent: false,
        has_clone: false,
    };

    if let Some(before_id) = root_node.before() {
        build_inline_layout_recursive(
            &mut builder,
            &mut text_transformer,
            nodes,
            before_id,
            &text_transform,
            &mut edge_basis,
        );
    }
    for child_id in root_node.children.iter().copied() {
        build_inline_layout_recursive(
            &mut builder,
            &mut text_transformer,
            nodes,
            child_id,
            &text_transform,
            &mut edge_basis,
        );
    }
    if let Some(after_id) = root_node.after() {
        build_inline_layout_recursive(
            &mut builder,
            &mut text_transformer,
            nodes,
            after_id,
            &text_transform,
            &mut edge_basis,
        );
    }

    text_layout.text = builder.build_into(&mut text_layout.layout);
    for inline_box in text_layout.layout.inline_boxes_mut() {
        if let parley::BaselineShift::Length(length) = inline_box.vertical_align.shift {
            inline_box.vertical_align.shift = parley::BaselineShift::Length(length * scale);
        }
    }
    text_layout.has_percent_edges = edge_basis.has_percent;
    text_layout.has_clone_spans = edge_basis.has_clone;
    return;

    fn build_inline_layout_recursive(
        builder: &mut TreeBuilder<TextBrush>,
        text_transformer: &mut TextTransformer,
        nodes: &crate::NodeTree,
        node_id: NodeId,
        parent_text_transform: &CaseTransform,
        edge_basis: &mut EdgeBasis,
    ) {
        let node = &nodes[node_id];

        let style = node.primary_styles();
        let style = style.as_ref();

        let text_transform = style
            .map(|s| CaseTransform::from_style(s))
            .unwrap_or(CaseTransform::NONE);

        match &node.data {
            NodeData::Element(element_data) | NodeData::AnonymousBlock(element_data) => {
                // if the input type is hidden, hide it
                if *element_data.name.local == *"input" {
                    if element_data.input_type().as_deref() == Some("hidden") {
                        return;
                    }
                }

                let display = node.display_style().unwrap_or(Display::inline());
                let position = style
                    .map(|s| s.clone_position())
                    .unwrap_or(PositionProperty::Static);
                let float = style.map(|s| s.clone_float()).unwrap_or(Float::None);
                let box_kind = if position.is_absolutely_positioned() {
                    InlineBoxKind::OutOfFlow
                } else if float.is_floating() {
                    InlineBoxKind::CustomOutOfFlow
                } else {
                    InlineBoxKind::InFlow
                };

                match (display.outside(), display.inside()) {
                    (DisplayOutside::None, DisplayInside::None) => {
                        // node.remove_damage(CONSTRUCT_DESCENDENT | CONSTRUCT_FC | CONSTRUCT_BOX);
                    }
                    (DisplayOutside::None, DisplayInside::Contents) => {
                        let whitespace = style.map(|s| {
                            [
                                parley::StyleProperty::WhiteSpaceCollapse(
                                    stylo_to_parley::white_space_collapse(
                                        s.clone_white_space_collapse(),
                                    ),
                                ),
                                parley::StyleProperty::TextWrapMode(
                                    stylo_to_parley::text_wrap_mode(s.clone_text_wrap_mode()),
                                ),
                                parley::StyleProperty::TabSize(stylo_to_parley::tab_size(
                                    &s.get_inherited_text().tab_size,
                                )),
                            ]
                        });
                        builder.push_style_modification_span(
                            whitespace
                                .as_ref()
                                .map_or(&[][..], |styles| styles.as_slice()),
                        );
                        for child_id in node.children.iter().copied() {
                            // node.remove_damage(CONSTRUCT_DESCENDENT | CONSTRUCT_FC | CONSTRUCT_BOX);
                            build_inline_layout_recursive(
                                builder,
                                text_transformer,
                                nodes,
                                child_id,
                                &text_transform,
                                edge_basis,
                            );
                        }
                        builder.pop_style_span();
                    }
                    (DisplayOutside::Inline, DisplayInside::Flow) => {
                        let tag_name = &element_data.name.local;

                        if is_inline_box_element(tag_name) || node.is_ruby_box() {
                            if box_kind == InlineBoxKind::InFlow {
                                text_transformer.word_break(builder);
                            }
                            builder.push_inline_box(InlineBox {
                                id: node_id.as_u64(),
                                kind: box_kind,
                                // Overridden by push_inline_box method
                                index: 0,
                                // Width and height are set during layout
                                width: 0.0,
                                height: 0.0,
                                baseline: None,
                                vertical_align: node
                                    .primary_styles()
                                    .map(|s| stylo_to_parley::vertical_align(&s))
                                    .unwrap_or_default(),
                            });
                        } else if *tag_name == local_name!("br") {
                            // node.remove_damage(CONSTRUCT_DESCENDENT | CONSTRUCT_FC | CONSTRUCT_BOX);
                            // TODO: update span id for br spans
                            builder.push_style_modification_span(&[
                                parley::StyleProperty::WhiteSpaceCollapse(
                                    WhiteSpaceCollapse::Preserve,
                                ),
                            ]);
                            builder.push_text("\n");
                            builder.pop_style_span();
                            text_transformer.word_break(builder);
                        } else {
                            let edges = node.primary_styles().map(|s| {
                                inline_edges(
                                    &s,
                                    node_id.as_u64(),
                                    edge_basis,
                                    node.tree().device_scale(),
                                )
                            });
                            if let Some(edges) = &edges {
                                push_margin_spacer(
                                    builder,
                                    edges,
                                    crate::util::INLINE_MARGIN_START,
                                    edges.margin_start,
                                );
                            }
                            let style = node
                                .primary_styles()
                                .map(|s| stylo_to_parley::style(node.id, &s))
                                .unwrap_or_else(|| parley::TextStyle {
                                    white_space_collapse: WhiteSpaceCollapse::Collapse,
                                    ..Default::default()
                                });

                            builder.push_style_span(style);
                            if let Some(edges) = &edges {
                                push_edge_spacer(
                                    builder,
                                    edges,
                                    crate::util::INLINE_EDGE_START,
                                    edges.inset_start,
                                );
                            }

                            if let Some(before_id) = node.before() {
                                build_inline_layout_recursive(
                                    builder,
                                    text_transformer,
                                    nodes,
                                    before_id,
                                    &text_transform,
                                    edge_basis,
                                );
                            }

                            for child_id in node.children.iter().copied() {
                                build_inline_layout_recursive(
                                    builder,
                                    text_transformer,
                                    nodes,
                                    child_id,
                                    &text_transform,
                                    edge_basis,
                                );
                            }
                            if let Some(after_id) = node.after() {
                                build_inline_layout_recursive(
                                    builder,
                                    text_transformer,
                                    nodes,
                                    after_id,
                                    &text_transform,
                                    edge_basis,
                                );
                            }

                            if let Some(edges) = &edges {
                                push_edge_spacer(
                                    builder,
                                    edges,
                                    crate::util::INLINE_EDGE_END,
                                    edges.inset_end,
                                );
                            }
                            builder.push_empty_span(node_id.as_u64());
                            builder.pop_style_span();
                            if let Some(edges) = &edges {
                                push_margin_spacer(
                                    builder,
                                    edges,
                                    crate::util::INLINE_MARGIN_END,
                                    edges.margin_end,
                                );
                                if edges.has_end_mark()
                                    && trailing_space(nodes, node_id) == Some(true)
                                {
                                    builder.push_style_modification_span(&[
                                        parley::StyleProperty::TextWrapMode(
                                            parley::TextWrapMode::Wrap,
                                        ),
                                        parley::StyleProperty::LetterSpacing(0.0),
                                    ]);
                                    builder.push_text(crate::util::INLINE_BREAK);
                                    builder.pop_style_span();
                                }
                            }
                        }
                    }
                    // Inline box
                    (_, _) => {
                        if box_kind == InlineBoxKind::InFlow {
                            text_transformer.word_break(builder);
                        }
                        builder.push_inline_box(InlineBox {
                            id: node_id.as_u64(),
                            kind: box_kind,
                            // Overridden by push_inline_box method
                            index: 0,
                            // Width and height are set during layout
                            width: 0.0,
                            height: 0.0,
                            baseline: None,
                            vertical_align: node
                                .primary_styles()
                                .map(|s| stylo_to_parley::vertical_align(&s))
                                .unwrap_or_default(),
                        });
                    }
                };
            }
            NodeData::Text(data) => {
                // node.remove_damage(CONSTRUCT_DESCENDENT | CONSTRUCT_FC | CONSTRUCT_BOX);
                // dbg!(&data.content);

                let in_closed_details = node.parent.is_some_and(|parent_id| {
                    let parent = &nodes[parent_id];
                    parent
                        .data
                        .is_element_with_tag_name(&local_name!("details"))
                        && parent.attr(local_name!("open")).is_none()
                });
                if !in_closed_details {
                    let text =
                        text_transformer.transform(&data.content, parent_text_transform, builder);
                    builder.push_text(text);
                }
            }
            NodeData::Comment { .. } => {
                // node.remove_damage(CONSTRUCT_DESCENDENT | CONSTRUCT_FC | CONSTRUCT_BOX);
            }
            NodeData::Document(_) => unreachable!(),
        }
    }
}
