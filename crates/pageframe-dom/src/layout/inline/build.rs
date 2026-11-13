use markup5ever::local_name;
use pageframe_traits::node_id::NodeId;
use parley::{
    FontContext, InlineBox, InlineBoxKind, LayoutContext, TreeBuilder, WhiteSpaceCollapse,
};
use style::{
    computed_values::{direction::T as Direction, position::T as PositionProperty},
    properties::ComputedValues,
    values::{
        computed::{BorderSideWidth, BoxDecorationBreak, Display, Float, Length, LengthPercentage},
        generics::length::GenericMargin,
        specified::{
            border::BorderStyle,
            box_::{DisplayInside, DisplayOutside},
        },
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
    has_edge_marks: bool,
    margin_start: f32,
    inset_start: f32,
    inset_end: f32,
    margin_end: f32,
    has_percent_margin: bool,
}

impl InlineEdges {
    fn has_end_mark(&self) -> bool {
        self.has_edge_marks || self.has_percent_margin || self.margin_end.abs() >= f32::EPSILON
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

fn inline_edges(style: &ComputedValues, basis: &mut EdgeBasis) -> InlineEdges {
    let basis_width = Length::new(basis.width);
    let margin = style.get_margin();
    let padding = style.get_padding();
    let border = style.get_border();
    basis.has_clone |= border.box_decoration_break == BoxDecorationBreak::Clone;
    let margin_px = |value: &GenericMargin<LengthPercentage>| match value {
        GenericMargin::LengthPercentage(value) => value.resolve(basis_width).px(),
        _ => 0.0,
    };
    let is_percent = |value: &GenericMargin<LengthPercentage>| match value {
        GenericMargin::LengthPercentage(value) => value.has_percentage(),
        _ => false,
    };
    let has_percent_margin = is_percent(&margin.margin_left) || is_percent(&margin.margin_right);
    basis.has_percent |= has_percent_margin
        || padding.padding_left.0.has_percentage()
        || padding.padding_right.0.has_percentage();
    let border_px = |width: &BorderSideWidth, line: BorderStyle| {
        if line.none_or_hidden() {
            0.0
        } else {
            width.0.to_f32_px()
        }
    };
    let left = (
        margin_px(&margin.margin_left),
        border_px(&border.border_left_width, border.border_left_style)
            + padding.padding_left.0.resolve(basis_width).px(),
    );
    let right = (
        margin_px(&margin.margin_right),
        border_px(&border.border_right_width, border.border_right_style)
            + padding.padding_right.0.resolve(basis_width).px(),
    );
    let (start, end) = if style.clone_direction() == Direction::Rtl {
        (right, left)
    } else {
        (left, right)
    };
    InlineEdges {
        has_edge_marks: crate::util::inline_box_is_decorated(style)
            || style
                .get_background()
                .background_color
                .resolve_to_absolute(&style.clone_color())
                .alpha
                > 0.0,
        margin_start: start.0,
        inset_start: start.1,
        inset_end: end.1,
        margin_end: end.0,
        has_percent_margin,
    }
}

fn push_spacer(builder: &mut TreeBuilder<TextBrush>, spacer: &str, width: f32) {
    let is_start =
        spacer == crate::util::INLINE_EDGE_START || spacer == crate::util::INLINE_MARGIN_START;
    let run_mark = if is_start { 1.0 } else { 2.0 };
    builder.push_style_modification_span(&[
        parley::StyleProperty::LetterSpacing(width),
        parley::StyleProperty::WordSpacing(run_mark),
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
    if edges.has_percent_margin || width.abs() >= f32::EPSILON {
        push_spacer(builder, spacer, width);
    }
}

fn push_edge_spacer(
    builder: &mut TreeBuilder<TextBrush>,
    edges: &InlineEdges,
    spacer: &str,
    width: f32,
) {
    if edges.has_edge_marks {
        push_spacer(builder, spacer, width);
    }
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
        .map(|s| parley::TextStyle {
            vertical_align: parley::VerticalAlign::BASELINE,
            ..stylo_to_parley::style(inline_context_root_node_id, s)
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

                        if is_inline_box_element(tag_name) {
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
                            let edges = node.primary_styles().map(|s| inline_edges(&s, edge_basis));
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
