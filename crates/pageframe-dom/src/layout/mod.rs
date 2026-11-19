//! Enable the dom to lay itself out using taffy
//!
//! In servo, style and layout happen together during traversal
//! However, in Pageframe, we do a style pass then a layout pass.
//! This is slower, yes, but happens fast enough that it's not a huge issue.

use crate::node::{
    ComputedStyleRef, ImageData, ListItemLayoutPosition, NodeData, SpecialElementData,
};
use crate::{document::BaseDocument, dom_node_id, node::Node, taffy_node_id};
use markup5ever::{LocalName, local_name};
use std::cell::Ref;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use style::Atom;
use style::values::computed::CSSPixelLength;
use style::values::computed::length_percentage::CalcLengthPercentage;
use style::values::computed::{Appearance, Contain, ContainerType};
use style::values::generics::length::{GenericMaxSize, GenericSize};
use stylo_taffy::CopyStyleGetters;
use stylo_taffy::TaffyStyloStyle;
use taffy::{
    AxisStaticEdge, AxisStaticPosition, BlockContext, CoreStyle as _, DetailedLayoutInfo,
    FlexDirection, FloatInsets, LayoutContainingBlock, LayoutPartialTree, MaybeMath as _, NodeId,
    OofCandidate, ResolveOrZero, RoundTree, RunMode, TraversePartialTree, TraverseTree,
    compute_block_layout, compute_cached_layout, compute_flexbox_layout, compute_grid_layout,
    compute_leaf_layout, compute_oof_layout, prelude::*,
};

pub(crate) mod construct;
pub(crate) mod counter_style;
pub(crate) mod damage;
pub(crate) mod date_field;
pub(crate) mod fieldset;
pub(crate) mod generated_content;
pub(crate) mod image_fallback;
pub(crate) mod inline;
pub(crate) mod line_clamp;
pub(crate) mod list;
pub(crate) mod multicol;
pub(crate) mod paint_tree;
pub(crate) mod replaced;
pub(crate) mod round;
pub(crate) mod ruby;
pub(crate) mod table;
pub(crate) mod text_transform;
#[cfg(feature = "writing-mode")]
pub(crate) mod writing_mode;

use self::replaced::{
    IntrinsicSizes, ReplacedContext, compute_replaced_layout, is_replaced_element,
};
use self::table::TableTreeWrapper;

/// The default object size for replaced elements
/// (https://drafts.csswg.org/css-images/#default-object-size).
const DEFAULT_OBJECT_SIZE: taffy::Size<f32> = taffy::Size {
    width: 300.0,
    height: 150.0,
};

/// The intrinsic dimensions and default object size for a replaced element
/// whose intrinsic dimensions are determined by its tag: an image with no
/// loaded resource has no intrinsic dimensions and a zero default object size;
/// a canvas has an intrinsic size and aspect ratio given by its width/height
/// attributes (defaulting to 300x150); other replaced elements (video, iframe,
/// embed) have no intrinsic dimensions and the 300x150 default object size.
fn tag_intrinsic_sizes(
    tag_name: &LocalName,
    attr_size: taffy::Size<Option<f32>>,
) -> (IntrinsicSizes, taffy::Size<f32>) {
    if *tag_name == local_name!("img") || *tag_name == local_name!("svg") {
        return (IntrinsicSizes::default(), taffy::Size::ZERO);
    }
    if *tag_name == local_name!("canvas") {
        let width = attr_size.width.unwrap_or(300.0);
        let height = attr_size.height.unwrap_or(150.0);
        return (
            IntrinsicSizes {
                width: Some(width),
                height: Some(height),
                ratio: Some(width / height),
            },
            DEFAULT_OBJECT_SIZE,
        );
    }
    (IntrinsicSizes::default(), DEFAULT_OBJECT_SIZE)
}

pub(crate) fn resolve_calc_value(calc_ptr: *const (), parent_size: f32) -> f32 {
    let calc = unsafe { &*(calc_ptr as *const CalcLengthPercentage) };
    let result = calc.resolve(CSSPixelLength::new(parent_size));
    result.px()
}

/// Per-pass layout state: wraps the document for the duration of a layout pass and carries
/// the Taffy tree trait implementations. Derefs to [`BaseDocument`].
pub(crate) struct LayoutPassState<'doc> {
    doc: &'doc mut BaseDocument,
    /// The writing mode of the box whose layout algorithm is currently running (see
    /// `layout::writing_mode`). Child styles read during that algorithm are expressed in its axes.
    #[cfg(feature = "writing-mode")]
    pub(crate) layout_wm: stylo_taffy::WritingMode,
    /// The root element and the writing mode its algorithm runs in (see `BaseDocument::root_layout_wm`).
    #[cfg(feature = "writing-mode")]
    root_id: Option<crate::NodeId>,
    #[cfg(feature = "writing-mode")]
    root_wm: stylo_taffy::WritingMode,
    /// Whether the node whose layout algorithm is running aligns its children's `align-self` in its
    /// inline axis (a column flex container).
    #[cfg(feature = "writing-mode")]
    pub(crate) current_align_axis_is_inline: bool,
    /// The box currently being laid out in an orthogonal flow and its containing block's inline
    /// size, against which its percentage padding resolves (see `layout::writing_mode`).
    #[cfg(feature = "writing-mode")]
    pub(crate) orthogonal_percent_basis: Option<(crate::NodeId, f32)>,
}

const SCROLLBAR_WIDTH: f32 = 15.0;
const NATIVE_TOGGLE_SIDE: f32 = 13.0;
const COLOR_SWATCH_INSET: f32 = 4.0;
const SLIDER_SIZE: taffy::Size<f32> = taffy::Size {
    width: 129.0,
    height: 16.0,
};

struct FormControlSizing {
    avg_char_width: f32,
    max_char_width: f32,
    scale: f32,
    line_height: f32,
    baseline: f32,
}

impl FormControlSizing {
    fn text_width(&self, chars: f32, extra: f32) -> f32 {
        (self.avg_char_width * chars + extra).ceil() / self.scale
    }
}

fn positive_integer_attr(element: &crate::node::ElementData, name: LocalName, default: u32) -> f32 {
    element
        .attr(name)
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default) as f32
}

impl<'doc> LayoutPassState<'doc> {
    fn form_control_metrics(&self, dom_id: crate::NodeId) -> Option<FormControlSizing> {
        use style::values::computed::font::LineHeight;

        let node = &self.doc.nodes[dom_id];
        let element = node.element_data()?;
        if !matches!(element.name.local.as_ref(), "input" | "textarea") {
            return None;
        }
        let style = node.primary_styles()?;
        let font_size = style.slow_clone_font_size().used_size().px();
        let scale = self.doc.viewport.scale();
        let metrics = crate::font_metrics::form_control_metrics(
            &mut self.doc.font_ctx.lock().unwrap(),
            style.get_font(),
            font_size,
            scale,
        )?;
        let ascent = metrics.ascent / scale;
        let descent = metrics.descent / scale;
        let line_height = match style.clone_line_height() {
            LineHeight::Normal => metrics.normal_line_height / scale,
            LineHeight::Number(num) => font_size * num.0,
            LineHeight::Length(value) => value.0.px(),
        };
        let baseline = match style.clone_line_height() {
            LineHeight::Normal => metrics.normal_baseline() / scale,
            _ => (line_height - ascent - descent) / 2.0 + ascent,
        };
        Some(FormControlSizing {
            avg_char_width: metrics.avg_char_width,
            max_char_width: metrics.max_char_width,
            scale,
            line_height,
            baseline,
        })
    }

    /// The containing block's inline size when `dom_id` is the box currently laid out in an orthogonal
    /// flow: its percentage padding resolves against it (see `layout::writing_mode`).
    #[inline]
    fn orthogonal_percent_basis_of(&self, dom_id: crate::NodeId) -> Option<f32> {
        #[cfg(feature = "writing-mode")]
        {
            self.orthogonal_percent_basis
                .filter(|(node, _)| *node == dom_id)
                .map(|(_, basis)| basis)
        }
        #[cfg(not(feature = "writing-mode"))]
        {
            let _ = dom_id;
            None
        }
    }

    pub(crate) fn new(doc: &'doc mut BaseDocument) -> Self {
        #[cfg(feature = "writing-mode")]
        let root_id = doc.try_root_element().map(|root| root.id);
        Self {
            #[cfg(feature = "writing-mode")]
            root_wm: root_id.map_or(stylo_taffy::WritingMode::empty(), |id| {
                doc.root_layout_wm(id)
            }),
            doc,
            #[cfg(feature = "writing-mode")]
            root_id,
            #[cfg(feature = "writing-mode")]
            layout_wm: stylo_taffy::WritingMode::empty(),
            #[cfg(feature = "writing-mode")]
            current_align_axis_is_inline: false,
            #[cfg(feature = "writing-mode")]
            orthogonal_percent_basis: None,
        }
    }
}

impl Deref for LayoutPassState<'_> {
    type Target = BaseDocument;
    fn deref(&self) -> &BaseDocument {
        self.doc
    }
}

impl DerefMut for LayoutPassState<'_> {
    fn deref_mut(&mut self) -> &mut BaseDocument {
        self.doc
    }
}

impl LayoutPassState<'_> {
    fn node_from_id(&self, node_id: taffy::prelude::NodeId) -> &Node {
        &self.nodes[dom_node_id(node_id)]
    }
    fn node_from_id_mut(&mut self, node_id: taffy::prelude::NodeId) -> &mut Node {
        &mut self.nodes[dom_node_id(node_id)]
    }
    fn set_content_box_subpixel_offset(
        &mut self,
        node_id: taffy::prelude::NodeId,
        offset: taffy::Point<f32>,
    ) {
        let Some(element) = self.node_from_id_mut(node_id).element_data_mut() else {
            return;
        };
        if let Some(inline_layout) = element.inline_layout_data.as_mut() {
            inline_layout.content_box_subpixel_offset = offset;
        }
        if let Some(list_item) = element.list_item_data.as_mut() {
            list_item.content_box_subpixel_offset = offset;
        }
        if let Some(text_input) = element.text_input_data_mut() {
            text_input.content_box_subpixel_offset = offset;
        }
    }
}

impl LayoutPassState<'_> {
    /// Run the node's layout algorithm, then lay out the out-of-flow (absolute/fixed)
    /// boxes for which it is the containing block. Must be called inside the layout
    /// cache wrapper so that cache hits do not re-run the out-of-flow pass.
    fn compute_child_layout_internal(
        &mut self,
        node_id: NodeId,
        inputs: taffy::tree::LayoutInput,
        block_ctx: Option<&mut BlockContext<'_>>,
    ) -> taffy::tree::LayoutOutput {
        #[cfg(feature = "writing-mode")]
        {
            self.compute_child_layout_in_own_wm(node_id, inputs, block_ctx)
        }
        #[cfg(not(feature = "writing-mode"))]
        {
            self.compute_child_layout_in_current_wm(node_id, inputs, block_ctx)
        }
    }

    /// Lay out `node_id` in the writing mode of the currently running algorithm (its parent's)
    pub(crate) fn compute_child_layout_in_current_wm(
        &mut self,
        node_id: NodeId,
        inputs: taffy::tree::LayoutInput,
        block_ctx: Option<&mut BlockContext<'_>>,
    ) -> taffy::tree::LayoutOutput {
        let inputs = self.apply_size_containment(node_id, inputs);
        let mut output = match self.keyword_min_width(node_id, &inputs) {
            Some(floor) => self.dispatch_with_floor(node_id, inputs, block_ctx, floor),
            None => self.dispatch_child_layout(node_id, inputs, block_ctx),
        };
        if self.is_rendered_legend(dom_node_id(node_id)) {
            output.baselines = taffy::Baselines::NONE;
        }
        if inputs.known_dimensions.width.is_none()
            && self.nodes[dom_node_id(node_id)]
                .data
                .is_element_with_tag_name(&local_name!("option"))
            && !self
                .option_select(dom_node_id(node_id))
                .is_some_and(|select| self.select_is_list(select))
        {
            let scale = self.doc.viewport.scale();
            output.size.width = (output.size.width * scale).ceil() / scale;
        }
        if let Some(line_height) = self.empty_list_item_line_height(node_id, &inputs) {
            output.size.height += line_height;
            output.margins_can_collapse_through = false;
        }
        if inputs.run_mode == RunMode::PerformLayout {
            compute_oof_layout(self, node_id, &mut output);
            let span_hoisted = self.nodes[dom_node_id(node_id)]
                .element_data()
                .and_then(|element| element.inline_layout_data.as_deref())
                .map(|layout| layout.span_hoisted.clone())
                .unwrap_or_default();
            if !span_hoisted.is_empty() {
                self.add_hoisted_children(node_id, &span_hoisted);
            }
        }
        output
    }

    fn empty_list_item_line_height(
        &self,
        node_id: NodeId,
        inputs: &taffy::tree::LayoutInput,
    ) -> Option<f32> {
        if inputs.known_dimensions.height.is_some() {
            return None;
        }
        let node = &self.nodes[dom_node_id(node_id)];
        let has_children = node
            .layout_children
            .borrow()
            .as_ref()
            .is_some_and(|children| !children.is_empty());
        if has_children || node.flags.is_inline_root() {
            return None;
        }
        let styles = node.primary_styles()?;
        let position = styles.get_position();
        let is_unconstrained = position.height.is_auto()
            && position.min_height.is_auto()
            && matches!(position.max_height, GenericMaxSize::None);
        if !is_unconstrained {
            return None;
        }
        let list_item = node.element_data()?.list_item_data.as_deref()?;
        let ListItemLayoutPosition::Outside(layout) = &list_item.position else {
            return None;
        };
        let line = layout.lines().next()?;
        Some(line.metrics().line_height / layout.scale())
    }

    fn keyword_min_width(
        &mut self,
        node_id: NodeId,
        inputs: &taffy::tree::LayoutInput,
    ) -> Option<f32> {
        if inputs.sizing_mode == taffy::SizingMode::ContentSize {
            return None;
        }
        let node = &self.nodes[dom_node_id(node_id)];
        let style = node.primary_styles()?;
        #[cfg(feature = "writing-mode")]
        let inline_is_width = !style.writing_mode.is_vertical() && !self.layout_wm.is_vertical();
        #[cfg(not(feature = "writing-mode"))]
        let inline_is_width = !style.writing_mode.is_vertical();
        if !inline_is_width {
            return None;
        }
        let available = match style.get_position().min_width {
            GenericSize::MinContent => AvailableSpace::MinContent,
            GenericSize::MaxContent => AvailableSpace::MaxContent,
            _ => return None,
        };
        drop(style);
        let measure = taffy::tree::LayoutInput {
            run_mode: RunMode::ComputeSize,
            sizing_mode: taffy::SizingMode::ContentSize,
            axis: taffy::RequestedAxis::Horizontal,
            known_dimensions: taffy::Size::NONE,
            available_space: taffy::Size {
                width: available,
                height: AvailableSpace::MaxContent,
            },
            vertical_margins_are_collapsible: taffy::Line::FALSE,
            ..*inputs
        };
        let measured = self.compute_child_layout_in_current_wm(node_id, measure, None);
        Some(measured.size.width)
    }

    fn apply_size_containment(
        &self,
        node_id: NodeId,
        mut inputs: taffy::tree::LayoutInput,
    ) -> taffy::tree::LayoutInput {
        let node = &self.nodes[dom_node_id(node_id)];
        let Some(style) = node.primary_styles() else {
            return inputs;
        };
        let display = style.get_box().display;
        if display.is_none() || display.is_contents() || display.is_inline_flow() {
            return inputs;
        }
        let contain = style.get_box().contain;
        let container_type = style.get_box().container_type;
        let inline = contain.contains(Contain::INLINE_SIZE)
            || container_type.intersects(ContainerType::INLINE_SIZE | ContainerType::SIZE);
        let block =
            contain.contains(Contain::BLOCK_SIZE) || container_type.contains(ContainerType::SIZE);
        if !inline && !block {
            return inputs;
        }
        #[cfg(feature = "writing-mode")]
        let inline_is_width = style.writing_mode.is_vertical() == self.layout_wm.is_vertical();
        #[cfg(not(feature = "writing-mode"))]
        let inline_is_width = !style.writing_mode.is_vertical();
        let (contain_width, contain_height) = if inline_is_width {
            (inline, block)
        } else {
            (block, inline)
        };
        let width_open = contain_width && inputs.known_dimensions.width.is_none();
        let height_open = contain_height && inputs.known_dimensions.height.is_none();
        if !width_open && !height_open {
            return inputs;
        }
        let empty = compute_leaf_layout(
            inputs,
            &self.child_layout_style(node),
            resolve_calc_value,
            |_known_size, _available_space| taffy::Size::ZERO,
        );
        if width_open {
            inputs.known_dimensions.width = Some(empty.size.width);
        }
        if height_open {
            inputs.known_dimensions.height = Some(empty.size.height);
        }
        inputs
    }

    /// The style of a node being laid out *by* the node whose algorithm is currently running,
    /// expressed in that node's writing mode (see `layout::writing_mode`).
    #[inline]
    pub(crate) fn child_layout_style<'a>(
        &self,
        node: &'a Node,
    ) -> stylo_taffy::TaffyStyloStyle<ComputedStyleRef<'a>> {
        #[cfg(feature = "writing-mode")]
        {
            let mut style = node.layout_style_in(self.layout_wm);
            style.align_axis_is_inline = self.current_align_axis_is_inline;
            style
        }
        #[cfg(not(feature = "writing-mode"))]
        {
            node.layout_style()
        }
    }

    fn dispatch_with_floor(
        &mut self,
        node_id: NodeId,
        inputs: taffy::tree::LayoutInput,
        block_ctx: Option<&mut BlockContext<'_>>,
        floor: f32,
    ) -> taffy::tree::LayoutOutput {
        let assumed_width = match inputs.known_dimensions.width {
            Some(width) => width,
            None => {
                let measure = taffy::tree::LayoutInput {
                    run_mode: RunMode::ComputeSize,
                    axis: taffy::RequestedAxis::Horizontal,
                    ..inputs
                };
                self.dispatch_child_layout(node_id, measure, None)
                    .size
                    .width
            }
        };
        if assumed_width >= floor {
            return self.dispatch_child_layout(node_id, inputs, block_ctx);
        }
        let inputs = with_known_width(inputs, floor);
        let Some(block_ctx) = block_ctx else {
            return self.dispatch_child_layout(node_id, inputs, None);
        };
        let extra = floor - assumed_width;
        let mut widened = block_ctx.sub_context(0.0, FloatInsets::new(0.0, -extra));
        let output = self.dispatch_child_layout(node_id, inputs, Some(&mut widened));
        let contribution = float_contribution(&widened);
        add_float_contribution(block_ctx, contribution);
        output
    }

    fn dispatch_child_layout(
        &mut self,
        node_id: NodeId,
        inputs: taffy::tree::LayoutInput,
        block_ctx: Option<&mut BlockContext<'_>>,
    ) -> taffy::tree::LayoutOutput {
        let orthogonal_percent_basis = self.orthogonal_percent_basis_of(dom_node_id(node_id));
        let form_control = self.form_control_metrics(dom_node_id(node_id));
        let legend_pass = self.needs_legend_pass(dom_node_id(node_id), &inputs);
        let node = &mut self.nodes[dom_node_id(node_id)];

        let font_styles = node.primary_styles().map(|style| {
            use style::values::computed::font::LineHeight;

            let font_size = style.slow_clone_font_size().used_size().px();
            let line_height = match style.clone_line_height() {
                LineHeight::Normal => font_size * 1.2,
                LineHeight::Number(num) => font_size * num.0,
                LineHeight::Length(value) => value.0.px(),
            };

            (font_size, line_height)
        });
        let font_size = font_styles.map(|s| s.0);
        let resolved_line_height = font_styles.map(|s| s.1);

        match &mut node.data {
            NodeData::Text(data) => {
                // With the new "inline context" architecture all text nodes should be wrapped in an "inline layout context"
                // and should therefore never be measured individually.
                #[cfg(feature = "tracing")]
                tracing::error!(
                    node_id = ?dom_node_id(node_id),
                    data = ?data,
                    "Tried to lay out text node individually",
                );

                #[cfg(not(feature = "tracing"))]
                let _ = data;

                taffy::LayoutOutput::HIDDEN
                // unreachable!();

                // compute_leaf_layout(inputs, &node.style, |known_dimensions, available_space| {
                //     let context = TextContext {
                //         text_content: &data.content.trim(),
                //         writing_mode: WritingMode::Horizontal,
                //     };
                //     let font_metrics = FontMetrics {
                //         char_width: 8.0,
                //         char_height: 16.0,
                //     };
                //     text_measure_function(
                //         known_dimensions,
                //         available_space,
                //         &context,
                //         &font_metrics,
                //     )
                // })
            }
            NodeData::Element(element_data) | NodeData::AnonymousBlock(element_data) => {
                // TODO: deduplicate with single-line text input
                if *element_data.name.local == *"textarea" {
                    let rows = positive_integer_attr(element_data, local_name!("rows"), 2);
                    let cols = positive_integer_attr(element_data, local_name!("cols"), 20);
                    let (text_width, line_height) = match &form_control {
                        Some(control) => (control.text_width(cols, 0.0), control.line_height),
                        None => (
                            font_size.unwrap_or(16.0) * 0.6 * cols,
                            resolved_line_height.unwrap_or(16.0),
                        ),
                    };

                    let output = compute_leaf_layout(
                        inputs,
                        &node.layout_style(),
                        resolve_calc_value,
                        |_known_size, _available_space| taffy::Size {
                            width: text_width + SCROLLBAR_WIDTH,
                            height: line_height * rows,
                        },
                    );
                    if inputs.run_mode == RunMode::PerformLayout {
                        let pb = {
                            let style = node.layout_style();
                            style
                                .padding()
                                .resolve_or_zero(inputs.parent_size.width, resolve_calc_value)
                                + style
                                    .border()
                                    .resolve_or_zero(inputs.parent_size.width, resolve_calc_value)
                        };
                        let content_width = (output.size.width - pb.horizontal_axis_sum()).max(0.0);
                        let doc = &mut *self.doc;
                        let scale = doc.viewport.scale();
                        let node = &mut doc.nodes[dom_node_id(node_id)];
                        if let Some(input) = node
                            .data
                            .downcast_element_mut()
                            .and_then(|el| el.text_input_data_mut())
                        {
                            input.set_width(Some(content_width * scale));
                            input.refresh_layout(
                                &mut doc.font_ctx.lock().unwrap(),
                                &mut doc.layout_ctx,
                            );
                        }
                    }
                    return output;
                }

                if matches!(element_data.name.local.as_ref(), "progress" | "meter") {
                    let mut output = compute_leaf_layout(
                        inputs,
                        &node.layout_style(),
                        resolve_calc_value,
                        |_known_size, _available_space| taffy::Size::ZERO,
                    );
                    output.baselines.first = Some(output.size.height);
                    output.baselines.last = Some(output.size.height);
                    return output;
                }

                if *element_data.name.local == *"input" {
                    match element_data.input_type().as_deref() {
                        // if the input type is hidden, hide it
                        Some("hidden") => {
                            return taffy::LayoutOutput::HIDDEN;
                        }
                        Some("checkbox" | "radio") => {
                            let mut output = compute_leaf_layout(
                                inputs,
                                &node.layout_style(),
                                resolve_calc_value,
                                |_known_size, _available_space| {
                                    let side = node
                                        .primary_styles()
                                        .filter(|style| {
                                            style.get_box().appearance != Appearance::None
                                        })
                                        .map_or(0.0, |style| {
                                            NATIVE_TOGGLE_SIDE * style.effective_zoom.value()
                                        });
                                    taffy::Size {
                                        width: side,
                                        height: side,
                                    }
                                },
                            );
                            output.baselines.first = Some(output.size.height);
                            output.baselines.last = Some(output.size.height);
                            return output;
                        }
                        Some("date" | "time" | "datetime-local" | "month" | "week") => {
                            return date_field::layout(node, inputs);
                        }
                        Some("range") => {
                            let mut output = compute_leaf_layout(
                                inputs,
                                &node.layout_style(),
                                resolve_calc_value,
                                |_known_size, _available_space| SLIDER_SIZE,
                            );
                            output.baselines.first = Some(output.size.height);
                            output.baselines.last = Some(output.size.height);
                            return output;
                        }
                        Some("color") => {
                            let style = node.layout_style();
                            let mut output = compute_leaf_layout(
                                inputs,
                                &style,
                                resolve_calc_value,
                                |_known_size, _available_space| taffy::Size::ZERO,
                            );
                            let bottom_inset = style
                                .border()
                                .bottom
                                .resolve_or_zero(inputs.parent_size.width, resolve_calc_value)
                                + style
                                    .padding()
                                    .bottom
                                    .resolve_or_zero(inputs.parent_size.width, resolve_calc_value);
                            let baseline =
                                (output.size.height - bottom_inset - COLOR_SWATCH_INSET).max(0.0);
                            output.baselines.first = Some(baseline);
                            output.baselines.last = Some(baseline);
                            return output;
                        }
                        None
                        | Some(
                            "text" | "password" | "email" | "tel" | "url" | "search" | "number",
                        ) => {
                            let size = positive_integer_attr(element_data, local_name!("size"), 20);
                            let (width, height) = match &form_control {
                                Some(control) => (
                                    control.text_width(size - 1.0, control.max_char_width),
                                    control.line_height,
                                ),
                                None => (
                                    font_size.unwrap_or(16.0) * 0.5 * size,
                                    resolved_line_height.unwrap_or(16.0),
                                ),
                            };
                            let style = node.layout_style();
                            let mut output = compute_leaf_layout(
                                inputs,
                                &style,
                                resolve_calc_value,
                                |_known_size, _available_space| taffy::Size { width, height },
                            );
                            if let Some(control) = &form_control {
                                let content_top = style
                                    .border()
                                    .top
                                    .resolve_or_zero(inputs.parent_size.width, resolve_calc_value)
                                    + style.padding().top.resolve_or_zero(
                                        inputs.parent_size.width,
                                        resolve_calc_value,
                                    );
                                let content_height = output.size.height
                                    - content_top
                                    - style.border().bottom.resolve_or_zero(
                                        inputs.parent_size.width,
                                        resolve_calc_value,
                                    )
                                    - style.padding().bottom.resolve_or_zero(
                                        inputs.parent_size.width,
                                        resolve_calc_value,
                                    );
                                let baseline = content_top
                                    + (content_height - control.line_height) / 2.0
                                    + control.baseline;
                                output.baselines.first = Some(baseline);
                                output.baselines.last = Some(baseline);
                            }
                            return output;
                        }
                        _ => {}
                    }
                }

                if is_replaced_element(&element_data.name.local) {
                    if matches!(
                        element_data.special_data,
                        SpecialElementData::ImageFallback(_)
                    ) {
                        return image_fallback::layout(node, inputs);
                    }
                    // Width/height attributes are presentational hints mapped to the CSS
                    // width/height properties by
                    // `synthesize_presentational_hints_for_legacy_attributes`, so they
                    // are already part of the style. They are only read here for the
                    // elements whose attributes determine their *intrinsic* size
                    // (canvas, and custom widgets on canvas tags).
                    let attr_size = taffy::Size {
                        width: element_data
                            .attr(local_name!("width"))
                            .and_then(|val| val.parse::<f32>().ok()),
                        height: element_data
                            .attr(local_name!("height"))
                            .and_then(|val| val.parse::<f32>().ok()),
                    };

                    // Get the element's intrinsic dimensions and default object size
                    let (intrinsic_sizes, default_object_size) = match &element_data.special_data {
                        SpecialElementData::Image(image_data) => match &**image_data {
                            ImageData::Raster(image) => {
                                let (width, height) = (image.width as f32, image.height as f32);
                                (
                                    IntrinsicSizes {
                                        width: Some(width),
                                        height: Some(height),
                                        ratio: Some(width / height),
                                    },
                                    DEFAULT_OBJECT_SIZE,
                                )
                            }
                            #[cfg(feature = "svg")]
                            ImageData::Svg(svg) => {
                                let mut width = svg.intrinsic_width();
                                let mut height = svg.intrinsic_height();
                                // An SVG with no declared dimensions and no viewBox has no
                                // intrinsic dimensions per CSS, but usvg still resolves a
                                // concrete size; use it in place of the default object size.
                                if width.is_none()
                                    && height.is_none()
                                    && svg.viewbox_aspect_ratio().is_none()
                                {
                                    let size = svg.tree.size();
                                    width = Some(size.width());
                                    height = Some(size.height());
                                }
                                (
                                    IntrinsicSizes {
                                        width,
                                        height,
                                        ratio: Some(svg.aspect_ratio()),
                                    },
                                    DEFAULT_OBJECT_SIZE,
                                )
                            }
                            ImageData::None => (IntrinsicSizes::default(), taffy::Size::ZERO),
                        },
                        SpecialElementData::Canvas(_)
                        | SpecialElementData::SubDocument(_)
                        | SpecialElementData::None => {
                            tag_intrinsic_sizes(&element_data.name.local, attr_size)
                        }
                        #[cfg(feature = "custom-widget")]
                        SpecialElementData::CustomWidget(widget_data) => {
                            let (fallback, default_object_size) =
                                tag_intrinsic_sizes(&element_data.name.local, attr_size);
                            // A canvas's content attributes determine its intrinsic size,
                            // overriding the widget-reported one; the widget-reported size
                            // in turn overrides the tag's fallback.
                            let attr_intrinsic =
                                if *element_data.name.local == local_name!("canvas") {
                                    attr_size
                                } else {
                                    taffy::Size::NONE
                                };
                            let attr_ratio = match (attr_intrinsic.width, attr_intrinsic.height) {
                                (Some(w), Some(h)) => Some(w / h),
                                _ => None,
                            };
                            let widget_sizes = widget_data.widget.intrinsic_sizes();
                            (
                                IntrinsicSizes {
                                    width: attr_intrinsic
                                        .width
                                        .or(widget_sizes.width)
                                        .or(fallback.width),
                                    height: attr_intrinsic
                                        .height
                                        .or(widget_sizes.height)
                                        .or(fallback.height),
                                    ratio: attr_ratio.or(widget_sizes.ratio).or(fallback.ratio),
                                },
                                default_object_size,
                            )
                        }
                        _ => unreachable!(),
                    };

                    #[cfg(feature = "writing-mode")]
                    let (intrinsic_sizes, default_object_size) =
                        if node.writing_mode().is_vertical() {
                            // Intrinsic sizes are physical; the element's algorithm runs in its own writing mode
                            (
                                crate::layout::replaced::IntrinsicSizes {
                                    width: intrinsic_sizes.height,
                                    height: intrinsic_sizes.width,
                                    ratio: intrinsic_sizes.ratio.map(|ratio| 1.0 / ratio),
                                },
                                default_object_size.transpose(),
                            )
                        } else {
                            (intrinsic_sizes, default_object_size)
                        };

                    let replaced_context = ReplacedContext {
                        intrinsic_sizes,
                        default_object_size,
                    };

                    let mut style = node.layout_style();
                    style.set_percent_basis(orthogonal_percent_basis);
                    return compute_replaced_layout(
                        inputs,
                        &style,
                        resolve_calc_value,
                        &replaced_context,
                    );
                }

                if node.flags.is_table_root() {
                    if let Some(layout_data) = node.try_layout_data_mut() {
                        layout_data.columns = None;
                    }
                    // The space available to an absolutely positioned table never exceeds
                    // the size of its containing block (less the table's margins), even
                    // when negative insets would otherwise add to it
                    // (https://drafts.csswg.org/css-tables-3/#abspos).
                    let mut inputs = inputs;
                    if self.nodes[dom_node_id(node_id)].is_out_of_flow() {
                        let margin = self.nodes[dom_node_id(node_id)]
                            .layout_style()
                            .margin()
                            .resolve_or_zero(inputs.parent_size.width, resolve_calc_value);
                        let max_available_space = inputs
                            .parent_size
                            .maybe_sub(margin.sum_axes())
                            .maybe_max(taffy::Size::ZERO);
                        inputs.available_space =
                            inputs.available_space.maybe_min(max_available_space);
                    }

                    let SpecialElementData::TableRoot(context) = &self.nodes[dom_node_id(node_id)]
                        .data
                        .downcast_element()
                        .unwrap()
                        .special_data
                    else {
                        panic!("Node marked as table root but doesn't have TableContext");
                    };
                    let context = Arc::clone(context);

                    let mut table_wrapper = TableTreeWrapper {
                        doc: self,
                        ctx: context,
                        style: None,
                        cell_baselines: Vec::new(),
                    };
                    table_wrapper.size_auto_columns(&inputs);
                    let inputs = table_wrapper.widen_for_captions(inputs);
                    let mut grid_inputs = inputs;
                    if table_wrapper.ctx.rows.iter().any(|row| row.is_collapsed)
                        || table_wrapper.ctx.has_collapsed_columns()
                    {
                        grid_inputs.run_mode = RunMode::PerformLayout;
                    }
                    let mut output = compute_grid_layout(&mut table_wrapper, node_id, grid_inputs);
                    if table_wrapper.size_spanning_rows(&grid_inputs) {
                        output = compute_grid_layout(&mut table_wrapper, node_id, grid_inputs);
                    }
                    if table_wrapper.spread_spare_height(&grid_inputs, &output) {
                        output = compute_grid_layout(&mut table_wrapper, node_id, grid_inputs);
                    }
                    table_wrapper.seat_on_first_row(&grid_inputs, &mut output);
                    let width = output.size.width;
                    table_wrapper.collapse_columns(&grid_inputs, &mut output);
                    if inputs.run_mode != RunMode::PerformLayout {
                        output.size.width = width;
                    }
                    table_wrapper.collapse_rows(&grid_inputs, &mut output);
                    table_wrapper.place_captions(&inputs, &mut output);
                    table_wrapper.offset_cells(dom_node_id(node_id), &grid_inputs);

                    // Absolutely positioned children take no part in the table grid. Their
                    // static position is the top-left of the table's content box.
                    let context = table_wrapper.ctx;
                    if inputs.run_mode == RunMode::PerformLayout && !context.oof_children.is_empty()
                    {
                        let style = &context.style;
                        let parent_width = inputs.parent_size.width;
                        let padding = style
                            .padding
                            .resolve_or_zero(parent_width, resolve_calc_value);
                        let border = style
                            .border
                            .resolve_or_zero(parent_width, resolve_calc_value);
                        let static_position = taffy::Point {
                            x: padding.left + border.left,
                            y: padding.top + border.top,
                        }
                        .map(|pos| AxisStaticPosition::from_edge(pos, AxisStaticEdge::Start));
                        let in_flow_count = context.cells.len();
                        for (idx, (child_id, position)) in context.oof_children.iter().enumerate() {
                            output.oof_candidates.push(OofCandidate {
                                node: taffy_node_id(*child_id),
                                order: (in_flow_count + idx) as u32,
                                position: *position,
                                static_position,
                            });
                        }
                    }

                    // HACK: Cap scrollable overflow at node size to prevent scrolling
                    let overflow = output.scrollable_overflow_rect;
                    output.scrollable_overflow_rect = taffy::ScrollableOverflow::new(
                        taffy::Rect {
                            left: 0.0,
                            top: 0.0,
                            right: overflow.right().min(output.size.width),
                            bottom: overflow.bottom().min(output.size.height),
                        },
                        output.scrollable_overflow_rect.is_reversed(),
                    );

                    return output;
                }

                if node.is_ruby_box() {
                    return self.compute_ruby_layout(dom_node_id(node_id), inputs);
                }

                let is_multicol = {
                    let style = node.primary_styles();
                    style.is_some_and(|style| style.is_multicol())
                } && (node.flags.is_inline_root()
                    || matches!(node.taffy_display(), Display::Block | Display::FlowRoot));
                if !is_multicol && let Some(layout_data) = node.try_layout_data_mut() {
                    layout_data.columns = None;
                }
                if node.flags.is_inline_root() {
                    if is_multicol {
                        return self.compute_multicol_layout(
                            dom_node_id(node_id),
                            inputs,
                            |doc, inputs| {
                                doc.compute_inline_layout(dom_node_id(node_id), inputs, None)
                            },
                        );
                    }
                    return self.compute_inline_layout(dom_node_id(node_id), inputs, block_ctx);
                }
                if is_multicol {
                    let mut output = self.compute_multicol_layout(
                        dom_node_id(node_id),
                        inputs,
                        |doc, inputs| compute_block_layout(doc, node_id, inputs, None),
                    );
                    self.clamp_child_lines(dom_node_id(node_id), &inputs, &mut output);
                    return output;
                }

                let is_fieldset = node.data.is_element_with_tag_name(&local_name!("fieldset"));
                // The default CSS file will set
                match node.taffy_display() {
                    Display::Block | Display::FlowRoot if legend_pass => {
                        let mut output = compute_block_layout(self, node_id, inputs, None);
                        self.place_rendered_legend(dom_node_id(node_id), &inputs, &mut output);
                        self.clamp_child_lines(dom_node_id(node_id), &inputs, &mut output);
                        output
                    }
                    Display::Block if !is_fieldset => {
                        let mut output = compute_block_layout(self, node_id, inputs, block_ctx);
                        self.clamp_child_lines(dom_node_id(node_id), &inputs, &mut output);
                        output
                    }
                    Display::Block | Display::FlowRoot => {
                        let mut output = compute_block_layout(self, node_id, inputs, None);
                        self.clamp_child_lines(dom_node_id(node_id), &inputs, &mut output);
                        output
                    }
                    Display::Flex => {
                        #[cfg(feature = "writing-mode")]
                        {
                            self.current_align_axis_is_inline = node.is_column_flex_container();
                        }
                        compute_flexbox_layout(self, node_id, inputs)
                    }
                    Display::Grid => compute_grid_layout(self, node_id, inputs),
                    Display::None => taffy::LayoutOutput::HIDDEN,
                }
            }
            NodeData::Document(_) => compute_block_layout(self, node_id, inputs, None),

            _ => taffy::LayoutOutput::HIDDEN,
        }
    }
}

impl TraversePartialTree for LayoutPassState<'_> {
    type ChildIter<'a>
        = RefCellChildIter<'a>
    where
        Self: 'a;

    fn child_ids(&self, node_id: NodeId) -> Self::ChildIter<'_> {
        let layout_children = self.node_from_id(node_id).layout_children.borrow(); //.unwrap().as_ref();
        RefCellChildIter::new(Ref::map(layout_children, |children| {
            children.as_ref().map(|c| c.as_slice()).unwrap_or(&[])
        }))
    }

    fn child_count(&self, node_id: NodeId) -> usize {
        self.node_from_id(node_id)
            .layout_children
            .borrow()
            .as_ref()
            .map(|c| c.len())
            .unwrap_or(0)
    }

    fn get_child_id(&self, node_id: NodeId, index: usize) -> NodeId {
        taffy_node_id(
            self.node_from_id(node_id)
                .layout_children
                .borrow()
                .as_ref()
                .unwrap()[index],
        )
    }
}
impl TraverseTree for LayoutPassState<'_> {}

impl LayoutPartialTree for LayoutPassState<'_> {
    type CoreContainerStyle<'a>
        = TaffyStyloStyle<ComputedStyleRef<'a>>
    where
        Self: 'a;

    type CustomIdent = Atom;

    fn get_core_container_style(&self, node_id: NodeId) -> Self::CoreContainerStyle<'_> {
        #[cfg(feature = "writing-mode")]
        {
            // Container styles are read by the node's own algorithm (or, for the containing block
            // of an out-of-flow box, by that box's algorithm) and are expressed in the node's writing mode.
            let dom_id = dom_node_id(node_id);
            let mut style = self.nodes[dom_id].layout_style_in(self.layout_wm_of(dom_id));
            style.set_percent_basis(self.orthogonal_percent_basis_of(dom_id));
            style
        }
        #[cfg(not(feature = "writing-mode"))]
        {
            self.node_from_id(node_id).layout_style()
        }
    }

    fn set_unrounded_layout(&mut self, node_id: NodeId, layout: &Layout) {
        *self.node_from_id_mut(node_id).unrounded_layout_mut() = *layout;
    }

    fn resolve_calc_value(&self, calc_ptr: *const (), parent_size: f32) -> f32 {
        resolve_calc_value(calc_ptr, parent_size)
    }

    #[inline(always)]
    fn compute_child_layout(
        &mut self,
        node_id: NodeId,
        inputs: taffy::LayoutInput,
    ) -> taffy::LayoutOutput {
        compute_cached_layout(self, node_id, inputs, |tree, node_id, inputs| {
            tree.compute_child_layout_internal(node_id, inputs, None)
        })
    }
}

impl LayoutContainingBlock for LayoutPassState<'_> {
    type OofItemStyle<'a>
        = TaffyStyloStyle<ComputedStyleRef<'a>>
    where
        Self: 'a;

    fn get_oof_item_style(&self, node_id: NodeId) -> Self::OofItemStyle<'_> {
        self.child_layout_style(self.node_from_id(node_id))
    }

    fn clear_hoisted_children(&mut self, node_id: NodeId) {
        let containing_block = dom_node_id(node_id);
        let mut hoisted = self.node_from_id(node_id).hoisted_children.borrow_mut();
        for &hoisted_id in hoisted.iter() {
            // A box which has since been claimed by another containing block
            // must keep pointing at that one.
            if let Some(node) = self.nodes.get(hoisted_id) {
                if node.oof_containing_block.get() == Some(containing_block) {
                    node.oof_containing_block.set(None);
                }
            }
        }
        hoisted.clear();
    }

    fn add_hoisted_children(&mut self, node_id: NodeId, hoisted: &[NodeId]) {
        let containing_block = dom_node_id(node_id);
        let node = self.node_from_id(node_id);
        node.hoisted_children
            .borrow_mut()
            .extend(hoisted.iter().copied().map(dom_node_id));
        for &hoisted_id in hoisted {
            self.node_from_id(hoisted_id)
                .oof_containing_block
                .set(Some(containing_block));
        }
    }

    fn get_detailed_layout_info(&self, node_id: NodeId) -> &DetailedLayoutInfo<Atom> {
        self.node_from_id(node_id)
            .element_data()
            .map(|element| &element.detailed_layout_info)
            .unwrap_or(&DetailedLayoutInfo::None)
    }
}

impl taffy::CacheTree for LayoutPassState<'_> {
    #[inline]
    fn cache_get(
        &mut self,
        node_id: NodeId,
        inputs: &taffy::LayoutInput,
    ) -> Option<taffy::LayoutOutput> {
        self.node_from_id_mut(node_id)
            .try_layout_data_mut()?
            .cache
            .get(inputs)
    }

    #[inline]
    fn cache_store(
        &mut self,
        node_id: NodeId,
        inputs: &taffy::LayoutInput,
        layout_output: taffy::LayoutOutput,
    ) {
        self.node_from_id_mut(node_id)
            .cache_mut()
            .store(inputs, layout_output);
    }

    #[inline]
    fn cache_clear(&mut self, node_id: NodeId) {
        self.node_from_id_mut(node_id).clear_layout_cache();
    }
}

impl taffy::LayoutBlockContainer for LayoutPassState<'_> {
    type BlockContainerStyle<'a>
        = TaffyStyloStyle<ComputedStyleRef<'a>>
    where
        Self: 'a;

    type BlockItemStyle<'a>
        = TaffyStyloStyle<ComputedStyleRef<'a>>
    where
        Self: 'a;

    fn get_block_container_style(&self, node_id: NodeId) -> Self::BlockContainerStyle<'_> {
        self.get_core_container_style(node_id)
    }

    fn get_block_child_style(&self, child_node_id: NodeId) -> Self::BlockItemStyle<'_> {
        self.child_layout_style(self.node_from_id(child_node_id))
    }

    #[inline(always)]
    fn compute_block_child_layout(
        &mut self,
        node_id: NodeId,
        inputs: taffy::LayoutInput,
        block_ctx: Option<&mut BlockContext<'_>>,
    ) -> taffy::LayoutOutput {
        compute_cached_layout(self, node_id, inputs, |tree, node_id, inputs| {
            tree.compute_child_layout_internal(node_id, inputs, block_ctx)
        })
    }
}

impl taffy::LayoutFlexboxContainer for LayoutPassState<'_> {
    type FlexboxContainerStyle<'a>
        = TaffyStyloStyle<ComputedStyleRef<'a>>
    where
        Self: 'a;

    type FlexboxItemStyle<'a>
        = TaffyStyloStyle<ComputedStyleRef<'a>>
    where
        Self: 'a;

    fn get_flexbox_container_style(&self, node_id: NodeId) -> Self::FlexboxContainerStyle<'_> {
        self.get_core_container_style(node_id)
    }

    fn get_flexbox_child_style(&self, child_node_id: NodeId) -> Self::FlexboxItemStyle<'_> {
        self.child_layout_style(self.node_from_id(child_node_id))
    }
}

impl taffy::LayoutGridContainer for LayoutPassState<'_> {
    type GridContainerStyle<'a>
        = TaffyStyloStyle<ComputedStyleRef<'a>>
    where
        Self: 'a;

    type GridItemStyle<'a>
        = TaffyStyloStyle<ComputedStyleRef<'a>>
    where
        Self: 'a;

    fn get_grid_container_style(&self, node_id: NodeId) -> Self::GridContainerStyle<'_> {
        self.get_core_container_style(node_id)
    }

    fn get_grid_child_style(&self, child_node_id: NodeId) -> Self::GridItemStyle<'_> {
        self.child_layout_style(self.node_from_id(child_node_id))
    }

    fn set_detailed_grid_info(
        &mut self,
        node_id: NodeId,
        detailed_grid_info: taffy::DetailedGridInfo<Atom>,
    ) {
        let node = self.node_from_id_mut(node_id);
        if let Some(element) = node.element_data_mut() {
            element.detailed_layout_info = DetailedLayoutInfo::Grid(Box::new(detailed_grid_info));
        }
    }
}

impl RoundTree for LayoutPassState<'_> {
    fn get_unrounded_layout(&self, node_id: NodeId) -> Layout {
        *self.node_from_id(node_id).unrounded_layout()
    }

    fn set_final_layout(&mut self, node_id: NodeId, layout: &Layout) {
        *self.node_from_id_mut(node_id).final_layout_mut() = *layout;
    }

    fn is_out_of_flow(&self, node_id: NodeId) -> bool {
        self.node_from_id(node_id)
            .out_of_flow
            .load(core::sync::atomic::Ordering::Relaxed)
    }

    fn hoisted_child_count(&self, node_id: NodeId) -> usize {
        self.node_from_id(node_id).hoisted_children.borrow().len()
    }

    fn get_hoisted_child_id(&self, node_id: NodeId, index: usize) -> NodeId {
        taffy_node_id(self.node_from_id(node_id).hoisted_children.borrow()[index])
    }
}

pub(crate) struct TaffyDebugTree<'doc>(pub(crate) &'doc BaseDocument);

impl TaffyDebugTree<'_> {
    fn node_from_id(&self, node_id: NodeId) -> &Node {
        &self.0.nodes[dom_node_id(node_id)]
    }
}

impl TraversePartialTree for TaffyDebugTree<'_> {
    type ChildIter<'a>
        = RefCellChildIter<'a>
    where
        Self: 'a;

    fn child_ids(&self, node_id: NodeId) -> Self::ChildIter<'_> {
        let layout_children = self.node_from_id(node_id).layout_children.borrow();
        RefCellChildIter::new(Ref::map(layout_children, |children| {
            children.as_ref().map(|c| c.as_slice()).unwrap_or(&[])
        }))
    }

    fn child_count(&self, node_id: NodeId) -> usize {
        self.node_from_id(node_id)
            .layout_children
            .borrow()
            .as_ref()
            .map(|c| c.len())
            .unwrap_or(0)
    }

    fn get_child_id(&self, node_id: NodeId, index: usize) -> NodeId {
        taffy_node_id(
            self.node_from_id(node_id)
                .layout_children
                .borrow()
                .as_ref()
                .unwrap()[index],
        )
    }
}

impl TraverseTree for TaffyDebugTree<'_> {}

impl PrintTree for TaffyDebugTree<'_> {
    fn get_debug_label(&self, node_id: NodeId) -> &'static str {
        let node = &self.node_from_id(node_id);

        match node.data {
            NodeData::Document(_) => "DOCUMENT",
            // NodeData::Doctype { .. } => return "DOCTYPE",
            NodeData::Text { .. } => node.node_debug_str().leak(),
            NodeData::Comment { .. } => "COMMENT",
            NodeData::AnonymousBlock(_) => "ANONYMOUS BLOCK",
            NodeData::Element(_) => {
                let style = node.layout_style();
                let display = match node.taffy_display() {
                    Display::Flex => match taffy::FlexboxContainerStyle::flex_direction(&style) {
                        FlexDirection::Row | FlexDirection::RowReverse => "FLEX ROW",
                        FlexDirection::Column | FlexDirection::ColumnReverse => "FLEX COL",
                    },
                    Display::Grid => "GRID",
                    Display::Block => "BLOCK",
                    Display::FlowRoot => "FLOW ROOT",
                    Display::None => "NONE",
                };
                format!("{} ({})", node.node_debug_str(), display).leak()
            } // NodeData::ProcessingInstruction { .. } => return "PROCESSING INSTRUCTION",
        }
    }

    fn get_final_layout(&self, node_id: NodeId) -> Layout {
        *self.node_from_id(node_id).final_layout()
    }
}

// pub struct ChildIter<'a>(std::slice::Iter<'a, usize>);
// impl<'a> Iterator for ChildIter<'a> {
//     type Item = NodeId;
//     fn next(&mut self) -> Option<Self::Item> {
//         self.0.next().copied().map(NodeId::from)
//     }
// }

pub struct RefCellChildIter<'a> {
    items: Ref<'a, [crate::NodeId]>,
    idx: usize,
}
impl<'a> RefCellChildIter<'a> {
    fn new(items: Ref<'a, [crate::NodeId]>) -> RefCellChildIter<'a> {
        RefCellChildIter { items, idx: 0 }
    }
}

impl Iterator for RefCellChildIter<'_> {
    type Item = NodeId;
    fn next(&mut self) -> Option<Self::Item> {
        self.items.get(self.idx).map(|id| {
            self.idx += 1;
            taffy_node_id(*id)
        })
    }
}

fn with_known_width(inputs: taffy::tree::LayoutInput, width: f32) -> taffy::tree::LayoutInput {
    taffy::tree::LayoutInput {
        known_dimensions: taffy::Size {
            width: Some(width),
            ..inputs.known_dimensions
        },
        ..inputs
    }
}

#[cfg(feature = "floats")]
fn float_contribution(block_ctx: &BlockContext<'_>) -> f32 {
    block_ctx.floated_content_height_contribution()
}

#[cfg(not(feature = "floats"))]
fn float_contribution(_block_ctx: &BlockContext<'_>) -> f32 {
    f32::NEG_INFINITY
}

#[cfg(feature = "floats")]
fn add_float_contribution(block_ctx: &mut BlockContext<'_>, contribution: f32) {
    block_ctx.add_child_floated_content_height_contribution(contribution);
}

#[cfg(not(feature = "floats"))]
fn add_float_contribution(_block_ctx: &mut BlockContext<'_>, _contribution: f32) {}
