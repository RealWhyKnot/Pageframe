mod background;
mod border;
mod box_shadow;
mod clip_path;
mod form_controls;
mod mask;
mod miter;
mod pixels;
mod projection;
mod resizer;
mod ring;
mod text_field;

use blitz_dom::CopyStyleGetters;
use std::cell::RefCell;
use std::collections::HashMap;

use super::kurbo_css::CssBox;
use crate::blend::convert_blend_mode;
use crate::color::{Color, ToColorColor};
use crate::debug_overlay::render_debug_overlay;
use crate::filters::{FilterOutput, filter_reach, filtered_image, pixel_filters};
use crate::kurbo_css::NonUniformRoundedRectRadii;
use crate::layers::LayerManager;
use crate::sizing::compute_object_fit;
use crate::text::{DrawTextContext, LineEllipsis, TextPaintOptions};
use crate::{CustomWidgetSceneMap, SELECTION_COLOR};
use anyrender::{PaintScene, Scene};
use blitz_dom::node::{
    ListItemLayout, ListItemLayoutPosition, Marker, MarkerShape, MarkerSymbol, NodeData,
    RasterImageData, TextBrush, TextInputData, TextNodeData,
};
use blitz_dom::{BaseDocument, ElementData, Node, NodeId, local_name};
use blitz_traits::devtools::DevtoolSettings;

use style::values::computed::{BorderCornerRadius, ColorOrAuto};
use style::{
    dom::TElement,
    properties::{
        ComputedValues, generated::longhands::visibility::computed_value::T as StyloVisibility,
        style_structs::Font,
    },
    values::{
        computed::{CSSPixelLength, Contain, Overflow},
        specified::box_::{DisplayInside, DisplayOutside},
        specified::image::ImageRendering,
    },
};

use kurbo::{self, Affine, Insets, Point, Rect, Shape, Size, Stroke, Vec2};
use peniko::{self, BlendMode, Fill, ImageData, ImageSampler};
use style::computed_values::direction::T as Direction;
use style::computed_values::float::T as StyloFloat;
use style::computed_values::isolation::T as Isolation;
use style::computed_values::position::T as StyloPosition;
use style::values::generics::color::GenericColor;
use taffy::Layout;

/// An opacity just below 1.0, used to force the renderer to allocate an isolated
/// buffer for mask and text-clip isolation layers. Renderers (e.g. vello_cpu) may optimize
/// layers with `Mix::Normal`/`Compose::SrcOver` and an opacity of exactly 1.0 into
/// plain non-isolated clip layers, in which case the `Compose::DestIn` mask
/// compositing would erase the backdrop behind the element as well as the element's own
/// content.
pub(super) const ALMOST_OPAQUE: f32 = 1.0 - f32::EPSILON;

/// A view of the track positions reported by taffy in physical (left-to-right /
/// top-to-bottom) order. Taffy reports tracks in logical order, which for RTL grid
/// containers is physically reversed.
#[derive(Clone, Copy)]
pub(crate) struct PhysicalTracks<'a> {
    positions: &'a [taffy::Line<f32>],
    reversed: bool,
}

impl<'a> PhysicalTracks<'a> {
    pub(crate) fn from_tracks<S: taffy::CheapCloneStr>(
        tracks: &'a taffy::DetailedGridTracksInfo<S>,
    ) -> Self {
        let positions = tracks.positions.as_slice();
        let reversed = positions
            .first()
            .zip(positions.last())
            .is_some_and(|(first, last)| first.start > last.start);
        Self {
            positions,
            reversed,
        }
    }

    fn get(self, index: usize) -> taffy::Line<f32> {
        if self.reversed {
            self.positions[self.positions.len() - 1 - index]
        } else {
            self.positions[index]
        }
    }

    /// Iterate track positions in physical order
    pub(crate) fn iter(self) -> impl ExactSizeIterator<Item = taffy::Line<f32>> + 'a {
        (0..self.positions.len()).map(move |index| self.get(index))
    }

    /// The physical start position of the first track
    pub(crate) fn origin(self) -> f32 {
        if self.positions.is_empty() {
            0.0
        } else {
            self.get(0).start
        }
    }

    /// Total distance from the start of the first track to the end of the last track
    pub(crate) fn span(self) -> f32 {
        if self.positions.is_empty() {
            0.0
        } else {
            self.get(self.positions.len() - 1).end - self.get(0).start
        }
    }
}

/// A short-lived struct which holds a bunch of parameters for rendering a scene so
/// that we don't have to pass them down as parameters
pub struct BlitzDomPainter<'dom, 'a> {
    /// Input parameters (read only) for generating the Scene
    pub(crate) dom: &'dom BaseDocument,
    pub(crate) scale: f64,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) initial_x: f64,
    pub(crate) initial_y: f64,
    /// The id of the document's root element (cached to avoid re-resolving it for every element)
    pub(crate) root_element_id: Option<NodeId>,
    /// Scrollbar hover/drag state, resolved once per scene like the root element
    #[cfg(feature = "scrollbars")]
    pub(crate) hovered_scrollbar: Option<blitz_dom::node::ScrollbarRef>,
    #[cfg(feature = "scrollbars")]
    pub(crate) scrollbar_drag_target: Option<blitz_dom::node::ScrollbarRef>,
    pub(crate) layer_manager: LayerManager,
    /// Reusable scratch allocations shared by all text layouts painted for this document.
    pub(crate) draw_text_context: RefCell<DrawTextContext>,
    pub(crate) deferred_paints: RefCell<Vec<DeferredPaint>>,
    /// Cached selection ranges for O(1) lookup: node_id -> (start_offset, end_offset)
    pub(crate) selection_ranges: HashMap<NodeId, (usize, usize)>,

    // Pre-computed `Scene`s for each CustomWidget
    pub(crate) custom_widget_scenes: &'a CustomWidgetSceneMap,
}

impl<'dom, 'a> BlitzDomPainter<'dom, 'a> {
    /// Create a new BlitzDomPainter for the given document
    pub fn new(
        dom: &'dom BaseDocument,
        scale: f64,
        width: u32,
        height: u32,
        initial_x: f64,
        initial_y: f64,
        custom_widget_scenes: &'a CustomWidgetSceneMap,
    ) -> Self {
        let selection_ranges: HashMap<NodeId, (usize, usize)> = dom
            .get_text_selection_ranges()
            .into_iter()
            .map(|(node_id, start, end)| (node_id, (start, end)))
            .collect();

        let layer_manager = LayerManager::default();
        let root_element_id = dom.try_root_element().map(|el| el.id);

        Self {
            dom,
            scale,
            width,
            height,
            initial_x,
            initial_y,
            root_element_id,
            #[cfg(feature = "scrollbars")]
            hovered_scrollbar: dom.hovered_scrollbar(),
            #[cfg(feature = "scrollbars")]
            scrollbar_drag_target: dom.scrollbar_drag_target(),
            layer_manager,
            draw_text_context: RefCell::new(DrawTextContext::default()),
            deferred_paints: RefCell::new(Vec::new()),
            selection_ranges,
            custom_widget_scenes,
        }
    }

    /// Draw the current tree to current render surface
    /// Eventually we'll want the surface itself to be passed into the render function, along with things like the viewport
    ///
    /// This assumes styles are resolved and layout is complete.
    /// Make sure you do those before trying to render
    pub fn paint_scene(&self, scene: &mut impl PaintScene) {
        if self.dom.has_pending_critical_resources() {
            return;
        }

        // Simply render the document (the root element (note that this is not the same as the root node)))
        // scene.reset();
        let viewport_scroll = self.dom.as_ref().viewport_scroll();

        // A document without a root element (e.g. an empty iframe sub-document) has
        // nothing to paint.
        let Some(root_element) = self.dom.as_ref().try_root_element() else {
            return;
        };
        let root_id = root_element.id;
        let bg_width = (self.width as f32).max(root_element.final_layout().size.width);
        let bg_height = (self.height as f32).max(root_element.final_layout().size.height);

        let background_color = {
            let html_styles = root_element.primary_styles();
            let html_color = html_styles
                .as_ref()
                .map_or(&GenericColor::TRANSPARENT_BLACK, |s| {
                    s.get_background_color()
                });
            if *html_color == GenericColor::TRANSPARENT_BLACK {
                root_element
                    .children
                    .iter()
                    .find_map(|id| {
                        self.dom
                            .as_ref()
                            .get_node(*id)
                            .filter(|node| node.data.is_element_with_tag_name(&local_name!("body")))
                    })
                    .and_then(|body| body.primary_styles())
                    .map(|style| {
                        let current_color = style.clone_color();
                        style
                            .get_background_color()
                            .resolve_to_absolute(&current_color)
                    })
            } else {
                let current_color = html_styles.as_ref().unwrap().clone_color();
                Some(html_color.resolve_to_absolute(&current_color))
            }
        };

        if let Some(bg_color) = background_color {
            let bg_color = bg_color.as_srgb_color();
            let rect = Rect::from_origin_size(
                (self.initial_x, self.initial_y),
                (bg_width as f64, bg_height as f64),
            );
            scene.fill(Fill::NonZero, Affine::IDENTITY, bg_color, None, &rect);
        }

        // The root clip rectangle is the viewport (in screen coordinates, with the
        // initial offset already subtracted). Elements outside of this are culled, and
        // scrollports narrow this rectangle further for their descendants.
        let viewport_clip_rect = Rect::new(0.0, 0.0, self.width as f64, self.height as f64);

        self.render_element(
            scene,
            root_id,
            Affine::translate(Vec2 {
                x: self.initial_x - (viewport_scroll.x * self.scale),
                y: self.initial_y - (viewport_scroll.y * self.scale),
            }),
            viewport_clip_rect,
        );

        // Render debug overlay
        if self.dom.devtools().highlight_hover {
            if let Some(node_id) = self.dom.as_ref().get_hover_node_id() {
                render_debug_overlay(
                    scene,
                    self.dom,
                    node_id,
                    self.scale,
                    self.initial_x,
                    self.initial_y,
                );
            }
        }
        if let Some(node_id) = self.dom.devtools().highlight_node {
            // Only nodes with element data have a layout to visualise
            if self
                .dom
                .as_ref()
                .get_node(node_id)
                .is_some_and(|node| node.element_data().is_some())
            {
                render_debug_overlay(
                    scene,
                    self.dom,
                    node_id,
                    self.scale,
                    self.initial_x,
                    self.initial_y,
                );
            }
        }
    }

    fn node_transform(&self, node: &Node, parent: Affine) -> Affine {
        self.node_position(node, parent) * node.transform().as_deref().copied().unwrap_or_default()
    }

    fn node_position(&self, node: &Node, parent: Affine) -> Affine {
        let location = node.final_layout().location;
        parent * Affine::translate(Vec2::new(location.x as f64, location.y as f64) * self.scale)
    }

    fn scrolled_transform(&self, node: &Node, transform: Affine) -> Affine {
        transform.then_translate(Vec2 {
            x: -node.scroll_offset().x * self.scale,
            y: -node.scroll_offset().y * self.scale,
        })
    }

    fn should_clip_node(&self, node: &Node, styles: &ComputedValues) -> bool {
        let box_style = styles.get_box();
        let contain_paint = box_style.slow_clone_contain().contains(Contain::PAINT) && {
            let display = styles.clone_display();
            let is_internal_table_box_other_than_cell = display.outside()
                == DisplayOutside::InternalTable
                && display.inside() != DisplayInside::TableCell;
            !display.is_inline_flow() && !is_internal_table_box_other_than_cell
        };
        let element = node.element_data();
        self.root_element_id != Some(node.id)
            && (element.and_then(|e| e.raster_image_data()).is_some()
                || element.and_then(|e| e.sub_doc_data()).is_some()
                || element.and_then(|e| e.text_input_data()).is_some()
                || contain_paint
                || !matches!(box_style.overflow_x, Overflow::Visible)
                || !matches!(box_style.overflow_y, Overflow::Visible))
    }

    fn draw_text_mask(
        &self,
        scene: &mut impl PaintScene,
        node_id: NodeId,
        transform: Affine,
        root: bool,
    ) {
        let node = &self.dom.as_ref().tree()[node_id];
        if matches!(node.taffy_display(), taffy::Display::None) || (!root && node.is_out_of_flow())
        {
            return;
        }
        let Some(styles) = node.primary_styles() else {
            return;
        };
        if styles.get_inherited_box().visibility != StyloVisibility::Visible
            || node.element_data().is_none()
        {
            return;
        }
        let cx = self.element_cx(node, *node.final_layout(), transform, None);
        let clip = if cx.text_input.is_some() {
            cx.frame.text_input_clip_path()
        } else {
            cx.frame.padding_box_path()
        };
        let pushed = self.layer_manager.maybe_push_layer(
            scene,
            self.should_clip_node(node, &styles),
            BlendMode::default(),
            1.0,
            transform,
            &clip,
            None,
            None,
        );
        let transform = self.scrolled_transform(node, transform);
        if node.flags.is_inline_root() {
            if let Some(layout) = &cx.element.inline_layout_data {
                let layout_box = node.final_layout();
                let x = (layout_box.padding.left + layout_box.border.left) as f64
                    - node.scroll_offset().x;
                let y = (layout_box.padding.top + layout_box.border.top) as f64
                    - node.scroll_offset().y
                    + layout.block_offset as f64;
                let text_transform =
                    transform * Affine::translate((x * self.scale, y * self.scale));
                let (visible_lines, ellipsis) =
                    node.primary_styles().map_or((usize::MAX, None), |style| {
                        line_ellipsis(
                            &style,
                            &layout.layout,
                            (layout_box.content_box_width() as f64 * self.scale) as f32,
                            blitz_dom::util::inherited_clamp_line(node),
                        )
                    });
                crate::text::stroke_text(
                    scene,
                    layout.layout.lines().take(visible_lines),
                    self.dom,
                    text_transform,
                    TextPaintOptions {
                        scale: self.scale,
                        brush_override: Some(Color::BLACK),
                        ellipsis,
                    },
                    node.id,
                    &mut self.draw_text_context.borrow_mut(),
                );
            }
        }
        if let Some(children) = &*node.layout_children.borrow() {
            for &child_id in children {
                let child = &self.dom.as_ref().tree()[child_id];
                self.draw_text_mask(
                    scene,
                    child_id,
                    self.node_transform(child, transform),
                    false,
                );
            }
        }
        self.layer_manager.maybe_pop_layer(scene, pushed);
    }

    /// Renders a node, but is guaranteed that the node is an element
    /// This is because the font_size is calculated from layout resolution and all text is rendered directly here, instead
    /// of a separate text stroking phase.
    ///
    /// In Blitz, text styling gets its attributes from its container element/resolved styles
    /// In other libraries, text gets its attributes from a `text` element - this is not how HTML works.
    ///
    /// Text is drawn from its container element, so its styles are already at hand and we
    /// never walk back up to the parent or pass styles down.
    fn render_element(
        &self,
        scene: &mut impl PaintScene,
        node_id: NodeId,
        parent_style_transform: Affine,
        clip_rect: Rect,
    ) {
        let node = &self.dom.as_ref().tree()[node_id];
        match self.projection(node) {
            Some(projection) => self.render_projected_element(
                scene,
                node_id,
                parent_style_transform,
                clip_rect,
                &projection,
            ),
            None => {
                self.render_element_with(scene, node_id, parent_style_transform, clip_rect, None)
            }
        }
    }

    fn render_element_with(
        &self,
        scene: &mut impl PaintScene,
        node_id: NodeId,
        parent_style_transform: Affine,
        clip_rect: Rect,
        own_transform: Option<Affine>,
    ) {
        let node = &self.dom.as_ref().tree()[node_id];

        // Early return if the element is hidden
        if matches!(node.taffy_display(), taffy::Display::None) {
            return;
        }

        // Only draw elements with a style
        let Some(styles) = node.primary_styles() else {
            return;
        };

        // Hide inputs with type=hidden
        // Checked here because doing it in the style engine is slower.
        if node.local_name() == "input"
            && node
                .element_data()
                .and_then(|el| el.input_type())
                .as_deref()
                == Some("hidden")
        {
            return;
        }

        // Hide elements with a visibility style other than visible
        if styles.get_inherited_box().visibility != StyloVisibility::Visible {
            return;
        }

        let effects = styles.get_effects();
        let opacity = effects.opacity;
        if opacity == 0.0 {
            return;
        }
        let has_opacity = opacity < 1.0;
        let filters = pixel_filters(&effects.filter.0, &styles.clone_color(), self.scale);

        let should_clip = self.should_clip_node(node, &styles);
        let is_text_input = node
            .element_data()
            .and_then(|el| el.text_input_data())
            .is_some();

        // Apply padding/border offset to inline root
        let taffy::Layout {
            size,
            border,
            padding,
            ..
        } = *node.final_layout();
        let box_size = Size::new(
            size.width as f64 * self.scale,
            size.height as f64 * self.scale,
        );
        let border_box = Rect::from_origin_size(Point::ORIGIN, box_size);
        let scaled_pb = (padding + border).map(f64::from);
        let content_position = kurbo::Point {
            x: scaled_pb.left,
            y: scaled_pb.top,
        };
        let content_box_size = kurbo::Size {
            width: (size.width as f64 - scaled_pb.left - scaled_pb.right) * self.scale,
            height: (size.height as f64 - scaled_pb.top - scaled_pb.bottom) * self.scale,
        };

        // Don't render things that are out of view
        let overflow = *node.scrollable_overflow();
        let transform = match own_transform {
            Some(own) => self.node_position(node, parent_style_transform) * own,
            None => self.node_transform(node, parent_style_transform),
        };
        let screen_transform = Affine::translate(Vec2 {
            x: -self.initial_x,
            y: -self.initial_y,
        }) * transform;
        let screen_bbox = screen_transform.transform_rect_bbox(overflow.union(border_box));
        let reach = filter_reach(&filters);
        let screen_bbox = Rect::new(
            screen_bbox.x0 + reach.x0,
            screen_bbox.y0 + reach.y0,
            screen_bbox.x1 + reach.x1,
            screen_bbox.y1 + reach.y1,
        );

        // Cull elements that fall entirely outside the current clip rectangle. In addition to
        // the viewport, `clip_rect` is narrowed by any ancestor scrollport (see below), so this
        // also culls elements scrolled out of view inside a clipping/scrolling container.
        if screen_bbox.x1 < clip_rect.x0
            || screen_bbox.x0 > clip_rect.x1
            || screen_bbox.y1 < clip_rect.y0
            || screen_bbox.y0 > clip_rect.y1
        {
            return;
        }

        // Cull elements whose transformed geometry is not representable in f32 (e.g.
        // `transform: scale(99e99)`): the renderer works in f32 and flattening such paths
        // can attempt to allocate an unbounded number of line segments.
        let f32_max = f64::from(f32::MAX);
        if !screen_bbox.is_finite()
            || [
                screen_bbox.x0,
                screen_bbox.y0,
                screen_bbox.x1,
                screen_bbox.y1,
            ]
            .iter()
            .any(|c| c.abs() > f32_max)
        {
            return;
        }

        // Optimise zero-area (/very small area) clips by not rendering at all
        let clip_area = content_box_size.width * content_box_size.height;
        let overflow_area =
            node.scrollable_overflow().width() * node.scrollable_overflow().height();
        if should_clip && clip_area < 0.01 && overflow_area < 0.01 {
            return;
        }

        #[cfg(feature = "custom-widget")]
        let custom_widget_scene = self.custom_widget_scenes.get(&(self.dom.id(), node_id));
        #[cfg(not(feature = "custom-widget"))]
        let custom_widget_scene = None;

        // Apply CSS transform property (where transforms are 2d)

        let mut cx = self.element_cx(node, *node.final_layout(), transform, custom_widget_scene);

        // If this element clips its overflow it establishes a scrollport: narrow the clip
        // rectangle passed to descendants to the visible (clipped) region so that content
        // scrolled out of view is skipped instead of drawn and clipped away. The box used
        // here matches the clip applied to the content below.
        let child_clip_rect = if should_clip {
            let clip_box = if is_text_input {
                cx.frame.text_input_clip_path()
            } else {
                cx.frame.padding_box_path()
            };
            clip_rect.intersect(screen_transform.transform_rect_bbox(clip_box.bounding_box()))
        } else {
            clip_rect
        };

        // Compute clip-path (if any) and wrap all rendering in a clip layer
        let clip_path_shape = cx.clip_path_shape();
        let has_clip_path = clip_path_shape.is_some();
        let default_clip = cx.frame.border_box_path();
        let mut clip_path_for_layer = clip_path_shape.unwrap_or(default_clip);
        clip_path_for_layer.apply_affine(Affine::scale(self.scale));

        // CSS 2 `clip` property (absolutely positioned elements only). Applies to the whole of
        // the element's rendering (including outlines and shadows), so it is the outermost layer.
        let css_clip_rect = cx.css_clip_rect();
        if css_clip_rect.is_some_and(|rect| rect.is_zero_area()) {
            return;
        }
        let css_clip_layer_pushed = self.layer_manager.maybe_push_layer(
            scene,
            css_clip_rect.is_some(),
            BlendMode::default(),
            1.0,
            cx.transform,
            &css_clip_rect.unwrap_or(Rect::ZERO),
            None,
            None,
        );

        // Use the viewport in scene coordinates so outlines, shadows and overflowing
        // descendants are not clipped to the element's border box.
        let blend = convert_blend_mode(effects.mix_blend_mode);
        let isolate = styles.get_box().isolation == Isolation::Isolate;
        let blend_layer_pushed = self.layer_manager.maybe_push_layer(
            scene,
            isolate || blend != BlendMode::default(),
            blend,
            if isolate { 1.0 - f32::EPSILON } else { 1.0 },
            Affine::IDENTITY,
            &self.viewport_rect(),
            None,
            None,
        );

        let paints_own_box = !node.is_inline_with_only_block_children();
        let parent = node
            .layout_parent
            .get()
            .and_then(|parent_id| self.dom.as_ref().tree().get(parent_id));
        let parent_is_flex_or_grid = parent.is_some_and(is_flex_or_grid);
        let is_paint_scope = parent.is_none_or(|parent| parent.flags.is_inline_root())
            || parent_is_flex_or_grid
            || node.stacking_context.is_some()
            || is_painted_above_text(node, parent_is_flex_or_grid)
            || styles.clone_float() != StyloFloat::None
            || node.flags.is_table_root()
            || should_clip
            || !filters.is_empty()
            || has_clip_path
            || css_clip_rect.is_some()
            || own_transform.is_some()
            || custom_widget_scene.is_some();
        let body = EffectBody {
            node,
            is_paint_scope,
            paints_own_box,
            is_text_input,
            should_clip,
            content_position,
            child_clip_rect,
        };

        if filters.is_empty() {
            cx.draw_outline(scene);
            if paints_own_box {
                cx.draw_outset_box_shadow(scene);
            }
        }

        self.layer_manager.maybe_with_layer(
            scene,
            has_clip_path,
            BlendMode::default(),
            1.0,
            cx.transform,
            &clip_path_for_layer,
            None,
            None,
            |scene| {
                let mask_layer_pushed = cx.maybe_push_css_mask_layer(scene);
                let unscrolled_transform = cx.transform;

                if filters.is_empty() {
                    self.layer_manager.maybe_with_layer(
                        scene,
                        has_opacity,
                        BlendMode::default(),
                        opacity,
                        Affine::IDENTITY,
                        &self.viewport_rect(),
                        None,
                        None,
                        |scene| self.paint_effect_body(&mut cx, scene, &body),
                    );
                    cx.transform = unscrolled_transform;
                    cx.draw_resizer(scene);
                    cx.draw_focus_ring(scene);
                } else {
                    let mut recording = Scene::new();
                    cx.draw_outline(&mut recording);
                    if paints_own_box {
                        cx.draw_outset_box_shadow(&mut recording);
                    }
                    self.paint_effect_body(&mut cx, &mut recording, &body);
                    cx.transform = unscrolled_transform;
                    cx.draw_resizer(&mut recording);
                    cx.draw_focus_ring(&mut recording);
                    match filtered_image(recording, &filters, self.viewport_rect()) {
                        FilterOutput::Image(image, region) => {
                            self.layer_manager.maybe_with_layer(
                                scene,
                                has_opacity,
                                BlendMode::default(),
                                opacity,
                                Affine::IDENTITY,
                                &region,
                                None,
                                None,
                                |scene| {
                                    scene.draw_image(
                                        image.as_ref(),
                                        Affine::translate((region.x0, region.y0)),
                                    );
                                },
                            );
                        }
                        FilterOutput::Unfiltered(recording) => {
                            self.layer_manager.maybe_with_layer(
                                scene,
                                has_opacity,
                                BlendMode::default(),
                                opacity,
                                Affine::IDENTITY,
                                &self.viewport_rect(),
                                None,
                                None,
                                |scene| scene.append_scene(recording, Affine::IDENTITY),
                            );
                        }
                    }
                }

                cx.maybe_pop_css_mask_layer(scene, mask_layer_pushed);
            },
        );

        self.layer_manager
            .maybe_pop_layer(scene, blend_layer_pushed);
        self.layer_manager
            .maybe_pop_layer(scene, css_clip_layer_pushed);
    }

    fn viewport_rect(&self) -> Rect {
        Rect::new(
            self.initial_x,
            self.initial_y,
            self.initial_x + f64::from(self.width),
            self.initial_y + f64::from(self.height),
        )
    }

    fn paint_effect_body(
        &self,
        cx: &mut ElementCx<'_, '_>,
        scene: &mut impl PaintScene,
        body: &EffectBody<'_>,
    ) {
        #[cfg(feature = "scrollbars")]
        let unscrolled_transform = cx.transform;
        let native_text_field = cx.has_native_text_field();
        if body.paints_own_box {
            if native_text_field {
                cx.draw_native_text_field(scene);
            } else {
                cx.draw_background(scene);
            }
            cx.draw_inset_box_shadow(scene);
        }
        cx.draw_table_row_backgrounds(scene);
        cx.draw_table_borders(scene);
        if body.paints_own_box && !native_text_field {
            cx.draw_border(scene);
        }
        cx.stroke_devtools(scene);

        let clip = if body.is_text_input {
            &cx.frame.text_input_clip_path()
        } else {
            &cx.frame.padding_box_path()
        };

        self.layer_manager.maybe_with_layer(
            scene,
            body.should_clip,
            BlendMode::default(),
            1.0,
            cx.transform,
            clip,
            None,
            None,
            |scene| {
                let content_position = Point {
                    x: body.content_position.x - body.node.scroll_offset().x,
                    y: body.content_position.y - body.node.scroll_offset().y,
                };

                cx.transform = self.scrolled_transform(body.node, cx.transform);
                cx.draw_image(scene);
                #[cfg(feature = "svg")]
                cx.draw_svg(scene);
                #[cfg(feature = "custom-widget")]
                cx.draw_custom_widget(scene);
                cx.draw_sub_document(scene);
                cx.draw_input(scene);
                cx.draw_text_input_text(scene, content_position);
                if !body.is_paint_scope && body.node.flags.is_inline_root() {
                    self.deferred_paints
                        .borrow_mut()
                        .push(DeferredPaint::Foreground {
                            node_id: body.node.id,
                            transform: cx.transform,
                            content_position,
                            clip_rect: body.child_clip_rect,
                        });
                    return;
                }
                let flush_from = body
                    .is_paint_scope
                    .then(|| self.deferred_paints.borrow().len());
                cx.draw_float_children(scene, cx.transform, body.child_clip_rect);
                cx.draw_inline_layout(scene, content_position);
                cx.draw_marker(scene, content_position);
                cx.draw_children(scene, cx.transform, body.child_clip_rect, flush_from);
            },
        );

        #[cfg(feature = "scrollbars")]
        {
            cx.transform = unscrolled_transform;
            cx.draw_scrollbars(scene);
        }
    }

    fn render_node(
        &self,
        scene: &mut impl PaintScene,
        node_id: NodeId,
        parent_style_transform: Affine,
        clip_rect: Rect,
    ) {
        let node = &self.dom.as_ref().tree()[node_id];

        match &node.data {
            NodeData::Element(_) | NodeData::AnonymousBlock(_) => {
                self.render_element(scene, node_id, parent_style_transform, clip_rect)
            }
            NodeData::Text(TextNodeData { .. }) => {
                // Text nodes should never be rendered directly
                // (they should always be rendered as part of an inline layout)
                // unreachable!()
            }
            NodeData::Document(_) => {}
            // NodeData::Doctype => {}
            NodeData::Comment { .. } => {} // NodeData::ProcessingInstruction { .. } => {}
        }
    }

    fn flush_deferred_paints(&'dom self, scene: &mut impl PaintScene, base: usize) {
        loop {
            let pending = self.deferred_paints.borrow_mut().split_off(base);
            if pending.is_empty() {
                return;
            }
            let (foregrounds, elements): (Vec<_>, Vec<_>) =
                pending.into_iter().partition(|paint| {
                    !matches!(
                        paint,
                        DeferredPaint::Element {
                            level: ElementLevel::Positioned,
                            ..
                        }
                    )
                });
            let batch = if foregrounds.is_empty() {
                elements
            } else {
                self.deferred_paints.borrow_mut().extend(elements);
                foregrounds
            };
            for paint in batch {
                match paint {
                    DeferredPaint::Foreground {
                        node_id,
                        transform,
                        content_position,
                        clip_rect,
                    } => {
                        let node = &self.dom.as_ref().tree()[node_id];
                        let cx = self.element_cx(node, *node.final_layout(), transform, None);
                        cx.draw_float_children(scene, cx.transform, clip_rect);
                        cx.draw_inline_layout(scene, content_position);
                        cx.draw_marker(scene, content_position);
                        cx.draw_children(scene, cx.transform, clip_rect, None);
                    }
                    DeferredPaint::Element {
                        node_id,
                        transform,
                        clip_rect,
                        ..
                    } => self.render_node(scene, node_id, transform, clip_rect),
                }
            }
        }
    }

    fn element_cx(
        &'dom self,
        node: &'dom Node,
        layout: Layout,
        transform: Affine,
        custom_widget_scene: Option<&'a Scene>,
    ) -> ElementCx<'dom, 'a> {
        let style = node
            .stylo_element_data()
            .primary_styles()
            .as_ref()
            .map(|styles| (*styles).clone())
            .unwrap_or_else(|| {
                ComputedValues::initial_values_with_font_override(Font::initial_values()).to_arc()
            });

        let scale = self.scale;

        // todo: maybe cache this so we don't need to constantly be figuring it out
        // It is quite a bit of math to calculate during render/traverse
        // Also! we can cache the bezpaths themselves, saving us a bunch of work
        let frame = create_css_rect(&style, &layout, scale);

        let element = node.element_data().unwrap();

        ElementCx {
            context: self,
            frame,
            scale,
            style,
            node,
            element,
            transform,
            #[cfg(feature = "svg")]
            svg: match element.image_data() {
                Some(blitz_dom::node::ImageData::Svg(svg)) => Some(svg),
                _ => None,
            },
            text_input: element.text_input_data(),
            list_item: element.list_item_data.as_deref(),
            devtools: self.dom.devtools(),
            custom_widget_scene,
        }
    }
}

fn to_image_quality(image_rendering: ImageRendering) -> peniko::ImageQuality {
    match image_rendering {
        ImageRendering::Auto => peniko::ImageQuality::Medium,
        ImageRendering::CrispEdges => peniko::ImageQuality::Low,
        ImageRendering::Pixelated => peniko::ImageQuality::Low,
    }
}

/// The transform from an SVG's rendered tree to a viewport of the given size.
#[cfg(feature = "svg")]
fn svg_viewport_affine(svg: &blitz_dom::node::SvgImageData, width: f32, height: f32) -> Affine {
    let t = svg.viewport_transform(width, height);
    Affine::new([t.sx, t.ky, t.kx, t.sy, t.tx, t.ty].map(f64::from))
}

/// Ensure that the `resized_image` field has a correctly sized image
fn to_peniko_image(image: &RasterImageData, quality: peniko::ImageQuality) -> peniko::ImageBrush {
    peniko::ImageBrush {
        image: ImageData {
            data: image.data.clone(),
            format: peniko::ImageFormat::Rgba8,
            width: image.width,
            height: image.height,
            alpha_type: peniko::ImageAlphaType::Alpha,
        },
        sampler: ImageSampler {
            x_extend: peniko::Extend::Repeat,
            y_extend: peniko::Extend::Repeat,
            quality,
            alpha: 1.0,
        },
    }
}

pub(crate) enum DeferredPaint {
    Foreground {
        node_id: NodeId,
        transform: Affine,
        content_position: Point,
        clip_rect: Rect,
    },
    Element {
        node_id: NodeId,
        transform: Affine,
        clip_rect: Rect,
        level: ElementLevel,
    },
}

#[derive(Clone, Copy)]
pub(crate) enum ElementLevel {
    Foreground,
    Positioned,
}

fn is_flex_or_grid(node: &Node) -> bool {
    matches!(
        node.taffy_display(),
        taffy::Display::Flex | taffy::Display::Grid
    )
}

fn is_painted_above_text(node: &Node, is_flex_or_grid_item: bool) -> bool {
    node.is_stacking_context_root(is_flex_or_grid_item)
        || node
            .primary_styles()
            .is_some_and(|style| style.clone_position() != StyloPosition::Static)
}

struct EffectBody<'n> {
    node: &'n Node,
    is_paint_scope: bool,
    paints_own_box: bool,
    is_text_input: bool,
    should_clip: bool,
    content_position: Point,
    child_clip_rect: Rect,
}

const DISCLOSURE_OUTSIDE_GAP: f64 = 8.0;
const MARKER_LINE_SEARCH_DEPTH: usize = 8;

/// A context of loaded and hot data to draw the element from
struct ElementCx<'dom, 'a> {
    context: &'dom BlitzDomPainter<'dom, 'a>,
    frame: CssBox,
    style: style::servo_arc::Arc<ComputedValues>,
    scale: f64,
    node: &'dom Node,
    element: &'dom ElementData,
    transform: Affine,
    #[cfg(feature = "svg")]
    svg: Option<&'dom blitz_dom::node::SvgImageData>,
    text_input: Option<&'dom TextInputData>,
    list_item: Option<&'dom ListItemLayout>,
    devtools: &'dom DevtoolSettings,
    #[cfg_attr(not(feature = "custom-widget"), expect(unused))]
    custom_widget_scene: Option<&'a Scene>,
}

/// Converts parley BoundingBox into peniko Rect
fn convert_rect(rect: &parley::BoundingBox) -> kurbo::Rect {
    peniko::kurbo::Rect::new(rect.x0, rect.y0, rect.x1, rect.y1)
}

impl ElementCx<'_, '_> {
    /// Paint overlay scrollbar thumbs for scroll containers: `overflow:
    /// scroll`, or `auto` when the content overflows (never `hidden`/`clip`,
    /// which scroll only programmatically). Thumbs appear on scroll and fade
    /// out after a delay ([`BaseDocument::scrollbar_opacity`]); never-scrolled
    /// containers paint nothing, keeping thumbs out of static reftest
    /// screenshots.
    ///
    /// Geometry comes from [`Node::scrollbar_thumb`], shared with the
    /// thumb-drag hit testing in blitz-dom.
    #[cfg(feature = "scrollbars")]
    fn draw_scrollbars(&self, scene: &mut impl PaintScene) {
        // css-scrollbars-1 scrollbar-color: author thumb/track colors
        use blitz_dom::node::{ScrollbarColor, ScrollbarRef};
        use taffy::AbsoluteAxis;
        let (custom_thumb, custom_track) = match self.node.scrollbar_color() {
            ScrollbarColor::Auto => (None, None),
            ScrollbarColor::Colors { thumb, track } => {
                (Some(thumb.as_srgb_color()), Some(track.as_srgb_color()))
            }
        };

        let drag_target = self.context.scrollbar_drag_target;
        let hovered_thumb = self.context.hovered_scrollbar;

        // scrollbar-color doesn't affect overlay visibility: persistence is
        // UA policy, not author styling.
        let node_id = self.node.id;
        let opacity = self.context.dom.scrollbar_opacity(node_id);
        if opacity == 0.0 {
            return;
        }

        // Default thumb palette for the used color scheme; thumbs paint as
        // fill plus a thin contrast stroke so they read over same-colored
        // content.
        let dark_scheme =
            self.context.dom.viewport().color_scheme == blitz_traits::shell::ColorScheme::Dark;
        let (thumb_rest, thumb_hover, thumb_active, stroke_color) = if dark_scheme {
            (
                Color::from_rgba8(214, 214, 214, 178),
                Color::from_rgba8(190, 190, 190, 222),
                Color::from_rgba8(172, 172, 172, 255),
                Color::from_rgba8(0, 0, 0, 102),
            )
        } else {
            (
                Color::from_rgba8(128, 128, 128, 178),
                Color::from_rgba8(152, 152, 152, 222),
                Color::from_rgba8(170, 170, 170, 255),
                Color::from_rgba8(255, 255, 255, 102),
            )
        };

        // Chromium's hovered/pressed scrollbar contrast ratios.
        const HOVER_CONTRAST: f32 = 1.8;
        const ACTIVE_CONTRAST: f32 = 1.3;

        for axis in [AbsoluteAxis::Vertical, AbsoluteAxis::Horizontal] {
            if !self.node.wants_scrollbar(axis) {
                continue;
            }
            let Some(thumb) = self.node.scrollbar_thumb(axis) else {
                continue;
            };

            let rect = thumb.scale_from_origin(self.scale);

            // Track (only when the author specified a track color)
            if let Some(track_color) = custom_track {
                let padding_box = self.frame.padding_box;
                let track_rect = match axis {
                    AbsoluteAxis::Horizontal => {
                        Rect::new(padding_box.x0, rect.y0, padding_box.x1, rect.y1)
                    }
                    AbsoluteAxis::Vertical => {
                        Rect::new(rect.x0, padding_box.y0, rect.x1, padding_box.y1)
                    }
                };
                scene.fill(
                    Fill::NonZero,
                    self.transform,
                    track_color.multiply_alpha(opacity),
                    None,
                    &track_rect,
                );
            }

            let this = ScrollbarRef { node_id, axis };
            let is_active = drag_target == Some(this);
            let is_hovered = hovered_thumb == Some(this);
            let color = match custom_thumb {
                Some(base) if is_active => crate::color::blend_for_contrast(base, ACTIVE_CONTRAST),
                Some(base) if is_hovered => crate::color::blend_for_contrast(base, HOVER_CONTRAST),
                Some(base) => base,
                None if is_active => thumb_active,
                None if is_hovered => thumb_hover,
                None => thumb_rest,
            };
            let radius = match axis {
                AbsoluteAxis::Horizontal => rect.height() / 2.0,
                AbsoluteAxis::Vertical => rect.width() / 2.0,
            };
            scene.fill(
                Fill::NonZero,
                self.transform,
                color.multiply_alpha(opacity),
                None,
                &rect.to_rounded_rect(radius),
            );
            // Contrast stroke, default thumbs only: an author-specified
            // scrollbar-color is rendered exactly as given.
            if custom_thumb.is_none() {
                let stroke_width = self.scale;
                let stroke_rect = rect.inset(-stroke_width / 2.0);
                scene.stroke(
                    &Stroke::new(stroke_width),
                    self.transform,
                    stroke_color.multiply_alpha(opacity),
                    None,
                    &stroke_rect.to_rounded_rect(radius - stroke_width / 2.0),
                );
            }
        }
    }

    fn draw_inline_layout(&self, scene: &mut impl PaintScene, pos: Point) {
        if self.node.flags.is_inline_root() {
            let text_layout = self.element
                .inline_layout_data
                .as_ref()
                .unwrap_or_else(|| {
                    panic!("Tried to render node marked as inline root that does not have an inline layout: {:?}", self.node);
                });

            // Text keeps its unrounded position; only box edges are snapped to whole pixels.
            let subpixel_offset = text_layout.content_box_subpixel_offset;
            let pos = Point {
                x: pos.x + subpixel_offset.x as f64,
                y: pos.y + subpixel_offset.y as f64 + text_layout.block_offset as f64,
            };
            let transform =
                self.transform * Affine::translate((pos.x * self.scale, pos.y * self.scale));

            let (visible_lines, ellipsis) = line_ellipsis(
                &self.style,
                &text_layout.layout,
                (self.frame.content_box.width() - subpixel_offset.x as f64 * self.scale) as f32,
                blitz_dom::util::inherited_clamp_line(self.node),
            );

            // Render inline element backgrounds (e.g. `<span style="background: ...">`)
            // behind the text and selection highlight.
            crate::text::draw_inline_backgrounds(
                scene,
                text_layout.layout.lines().take(visible_lines),
                self.context.dom,
                transform,
                self.node.id,
            );

            // Render text selection highlight (if any) using cached selection ranges
            if let Some(&(sel_start, sel_end)) = self.context.selection_ranges.get(&self.node.id) {
                crate::text::draw_text_selection(
                    scene,
                    &text_layout.layout,
                    transform,
                    sel_start,
                    sel_end,
                    visible_lines,
                );
            }

            // Render text
            let mut draw_text_context = self.context.draw_text_context.borrow_mut();
            crate::text::stroke_text(
                scene,
                text_layout.layout.lines().take(visible_lines),
                self.context.dom,
                transform,
                TextPaintOptions {
                    scale: self.scale,
                    brush_override: None,
                    ellipsis,
                },
                self.node.id,
                &mut draw_text_context,
            );
        }
    }

    fn draw_text_input_text(&self, scene: &mut impl PaintScene, pos: Point) {
        // Render the text in text inputs
        if let Some(input_data) = self.text_input {
            // For single-line inputs, add an offset to vertically center the text input layout
            // within the content box of it's node.
            let y_offset = self.node.text_input_v_centering_offset(self.scale);
            let subpixel_offset = input_data.content_box_subpixel_offset;
            let pos = Point {
                x: pos.x + subpixel_offset.x as f64,
                y: pos.y + subpixel_offset.y as f64 + y_offset,
            };

            // Apply the scroll offset (stored in CSS pixels, scaled here to device pixels) so
            // that the caret stays visible. Single-line inputs scroll horizontally; multi-line
            // inputs scroll vertically.
            let scroll_offset = input_data.scroll_offset as f64 * self.scale;
            let (scroll_x, scroll_y) = if input_data.is_multiline {
                (0.0, scroll_offset)
            } else {
                (scroll_offset, 0.0)
            };

            let transform = self.transform
                * Affine::translate((pos.x * self.scale - scroll_x, pos.y * self.scale - scroll_y));

            if self.node.is_focussed() {
                // Render selection/caret
                for (rect, _line_idx) in input_data.editor.selection_geometry().iter() {
                    scene.fill(
                        Fill::NonZero,
                        transform,
                        SELECTION_COLOR,
                        None,
                        &convert_rect(rect),
                    );
                }
                if let Some(cursor) = input_data.editor.cursor_geometry(0.0)
                    && let Some(layout) = input_data.editor.try_layout()
                {
                    let origin =
                        Vec2::new(pos.x * self.scale - scroll_x, pos.y * self.scale - scroll_y);
                    let centred =
                        !input_data.is_multiline && input_data.editor.raw_text().is_empty();
                    let caret =
                        text_field::caret_rect(layout, &cursor, self.scale, origin, centred);
                    let color = self.style.get_inherited_text().color;
                    let caret_color = match &self.style.get_inherited_ui().caret_color.0 {
                        ColorOrAuto::Auto => color,
                        ColorOrAuto::Color(caret_color) => caret_color.resolve_to_absolute(&color),
                    };

                    scene.fill(
                        Fill::NonZero,
                        transform,
                        caret_color.as_srgb_color(),
                        None,
                        &caret,
                    );
                };
            }

            // Render text
            let mut draw_text_context = self.context.draw_text_context.borrow_mut();
            crate::text::stroke_text(
                scene,
                input_data.editor.try_layout().unwrap().lines(),
                self.context.dom,
                transform,
                TextPaintOptions {
                    scale: self.scale,
                    brush_override: None,
                    ellipsis: None,
                },
                self.node.id,
                &mut draw_text_context,
            );
        }
    }

    fn draw_marker(&self, scene: &mut impl PaintScene, pos: Point) {
        let Some(list_item) = self.list_item else {
            return;
        };
        let subpixel_offset = list_item.content_box_subpixel_offset;
        let origin = Point {
            x: pos.x + f64::from(subpixel_offset.x),
            y: pos.y + f64::from(subpixel_offset.y),
        };
        match (&list_item.marker, &list_item.position) {
            (Marker::Symbol(symbol), ListItemLayoutPosition::Outside(layout)) => {
                self.draw_outside_symbol(scene, pos, origin, symbol, layout);
            }
            (Marker::Symbol(symbol), ListItemLayoutPosition::Inside) => {
                self.draw_inside_symbol(scene, origin, symbol);
            }
            (marker, ListItemLayoutPosition::Outside(layout)) => {
                self.draw_outside_marker_text(scene, pos, origin, marker, layout);
            }
            (_, ListItemLayoutPosition::Inside) => {}
        }
    }

    fn marker_color(&self) -> Option<Color> {
        let color = self.list_item?.marker_color.as_ref()?;
        Some(color.as_srgb_color())
    }

    fn is_rtl(&self) -> bool {
        self.style.clone_direction() == Direction::Rtl
    }

    fn border_box_inline_edges(&self, pos: Point) -> (f64, f64) {
        let item_layout = self.node.final_layout();
        let left = pos.x - f64::from(item_layout.padding.left + item_layout.border.left);
        (left, left + f64::from(item_layout.size.width))
    }

    fn first_line_baseline(&self) -> Option<f64> {
        let own_layout = self.node.final_layout();
        let mut node = self.node;
        let mut top = -f64::from(own_layout.border.top + own_layout.padding.top);
        for _ in 0..MARKER_LINE_SEARCH_DEPTH {
            let layout = node.final_layout();
            let content_top = top + f64::from(layout.border.top + layout.padding.top);
            if node.flags.is_inline_root() {
                let text_layout = node.element_data()?.inline_layout_data.as_ref()?;
                let line = text_layout.layout.lines().next()?;
                let line_top = content_top + f64::from(text_layout.block_offset);
                return Some(f64::from(line.metrics().baseline) + line_top * self.scale);
            }
            let first_child = *node.layout_children.borrow().as_ref()?.first()?;
            node = &self.context.dom.as_ref().tree()[first_child];
            top += f64::from(node.final_layout().location.y);
        }
        None
    }

    fn draw_outside_symbol(
        &self,
        scene: &mut impl PaintScene,
        pos: Point,
        origin: Point,
        symbol: &MarkerSymbol,
        layout: &parley::Layout<TextBrush>,
    ) {
        let own_baseline = layout
            .lines()
            .next()
            .map(|line| f64::from(line.metrics().baseline));
        let Some(baseline) = self.first_line_baseline().or(own_baseline) else {
            return;
        };
        let size = f64::from(symbol.size());
        let reach = f64::from(symbol.offset() + MarkerSymbol::OUTSIDE_GAP);
        let (left, right) = self.border_box_inline_edges(pos);
        let x = if self.is_rtl() {
            right * self.scale + reach - size
        } else {
            left * self.scale - reach
        };
        let y = origin.y * self.scale + baseline + f64::from(symbol.top_from_baseline());
        self.draw_symbol(scene, symbol, x, y);
    }

    fn draw_inside_symbol(
        &self,
        scene: &mut impl PaintScene,
        origin: Point,
        symbol: &MarkerSymbol,
    ) {
        let Some(text_layout) = self.element.inline_layout_data.as_ref() else {
            return;
        };
        let Some(line) = text_layout.layout.lines().next() else {
            return;
        };
        let size = f64::from(symbol.size());
        let marker_run = line.items().find_map(|item| match item {
            parley::PositionedLayoutItem::GlyphRun(glyph_run)
                if glyph_run.run().text_range().start == 0 =>
            {
                let start = f64::from(glyph_run.offset());
                let x = if glyph_run.run().is_rtl() {
                    start + f64::from(glyph_run.advance()) - size
                } else {
                    start
                };
                Some((x, f64::from(glyph_run.baseline())))
            }
            _ => None,
        });
        let Some((x, baseline)) = marker_run else {
            return;
        };
        let top = (origin.y + f64::from(text_layout.block_offset)) * self.scale;
        self.draw_symbol(
            scene,
            symbol,
            origin.x * self.scale + x,
            top + baseline + f64::from(symbol.top_from_baseline()),
        );
    }

    fn draw_symbol(&self, scene: &mut impl PaintScene, symbol: &MarkerSymbol, x: f64, y: f64) {
        let size = f64::from(symbol.size());
        let rect = Rect::new(x.round(), y.round(), x.round() + size, y.round() + size);
        let color = self
            .marker_color()
            .unwrap_or_else(|| self.style.clone_color().as_srgb_color());
        match symbol.shape {
            MarkerShape::Disc => scene.fill(
                Fill::NonZero,
                self.transform,
                color,
                None,
                &kurbo::Ellipse::from_rect(rect),
            ),
            MarkerShape::Circle => scene.stroke(
                &Stroke::new(1.0),
                self.transform,
                color,
                None,
                &kurbo::Ellipse::from_rect(rect),
            ),
            MarkerShape::Square => {
                scene.fill(Fill::NonZero, self.transform, color, None, &rect);
            }
        }
    }

    fn draw_outside_marker_text(
        &self,
        scene: &mut impl PaintScene,
        pos: Point,
        origin: Point,
        marker: &Marker,
        layout: &parley::Layout<TextBrush>,
    ) {
        let Some(marker_line) = layout.lines().next() else {
            return;
        };
        let layout_scale = f64::from(layout.scale());
        let gap = match marker {
            Marker::Char(_) => DISCLOSURE_OUTSIDE_GAP / self.scale,
            Marker::Symbol(_) | Marker::String(_) => 0.0,
        };
        let (left, right) = self.border_box_inline_edges(pos);
        let x = if self.is_rtl() {
            right + gap
        } else {
            left - gap - f64::from(layout.full_width()) / layout_scale
        };
        let marker_baseline = f64::from(marker_line.metrics().baseline);
        let y = match self.first_line_baseline() {
            Some(baseline) => origin.y + (baseline - marker_baseline) / layout_scale,
            None => origin.y,
        };

        let transform = self.transform * Affine::translate((x * self.scale, y * self.scale));

        let mut draw_text_context = self.context.draw_text_context.borrow_mut();
        crate::text::stroke_text(
            scene,
            layout.lines(),
            self.context.dom,
            transform,
            TextPaintOptions {
                scale: self.scale,
                brush_override: self.marker_color(),
                ellipsis: None,
            },
            self.node.id,
            &mut draw_text_context,
        );
    }

    fn is_float_under_text(&self, child: &Node) -> bool {
        self.node.flags.is_inline_root()
            && !is_painted_above_text(child, is_flex_or_grid(self.node))
            && child
                .primary_styles()
                .is_some_and(|style| style.clone_float() != StyloFloat::None)
    }

    fn child_transform(&self, child: &Node, parent_style_transform: Affine) -> Affine {
        if child.taffy_position() == taffy::Position::Fixed
            && Some(self.node.id) == self.context.root_element_id
        {
            let scroll = self.context.dom.as_ref().viewport_scroll();
            parent_style_transform.pre_translate(kurbo::Vec2 {
                x: scroll.x * self.scale,
                y: scroll.y * self.scale,
            })
        } else if let Some(offset) = child.lifted_paint_offset(self.node.id) {
            parent_style_transform.pre_translate(kurbo::Vec2 {
                x: offset.x as f64 * self.scale,
                y: offset.y as f64 * self.scale,
            })
        } else {
            parent_style_transform
        }
    }

    fn draw_float_children(
        &self,
        scene: &mut impl PaintScene,
        parent_style_transform: Affine,
        clip_rect: Rect,
    ) {
        if !self.node.flags.is_inline_root() {
            return;
        }
        if let Some(children) = &*self.node.paint_children.borrow() {
            for child_id in children {
                let child = &self.context.dom.as_ref().tree()[*child_id];
                if self.is_float_under_text(child) {
                    let transform = self.child_transform(child, parent_style_transform);
                    self.render_node(scene, *child_id, transform, clip_rect);
                }
            }
        }
    }

    fn draw_children(
        &self,
        scene: &mut impl PaintScene,
        parent_style_transform: Affine,
        clip_rect: Rect,
        flush_from: Option<usize>,
    ) {
        // Negative z_index hoisted nodes

        if let Some(hoisted) = &self.node.stacking_context {
            for hoisted_child in hoisted.neg_z_hoisted_children() {
                let position = hoisted_child.position(self.node.tree(), self.node.id);
                let pos = kurbo::Vec2 {
                    x: position.x as f64 * self.scale,
                    y: position.y as f64 * self.scale,
                };
                self.render_node(
                    scene,
                    hoisted_child.node_id,
                    parent_style_transform.pre_translate(pos),
                    clip_rect,
                );
            }
        }

        // Inline boxes on lines hidden by `line-clamp` are not painted.
        let clamped_boxes: Vec<u64> = self
            .element
            .inline_layout_data
            .as_ref()
            .filter(|_| self.node.flags.is_inline_root())
            .filter(|_| blitz_dom::util::clamp_hides_lines(&self.style))
            .and_then(|text_layout| {
                let visible_lines =
                    blitz_dom::util::clamped_line_count(&self.style, &text_layout.layout)?;
                Some(
                    text_layout
                        .layout
                        .lines()
                        .skip(visible_lines)
                        .flat_map(|line| {
                            line.items()
                                .filter_map(|item| match item {
                                    parley::PositionedLayoutItem::InlineBox(inline_box)
                                        if inline_box.kind == parley::InlineBoxKind::InFlow =>
                                    {
                                        Some(inline_box.id)
                                    }
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                        })
                        .collect(),
                )
            })
            .unwrap_or_default();

        // Regular children
        if let Some(children) = &*self.node.paint_children.borrow() {
            for child_id in children {
                if clamped_boxes.contains(&child_id.as_u64()) {
                    continue;
                }
                let child = &self.context.dom.as_ref().tree()[*child_id];
                if self.is_float_under_text(child) {
                    continue;
                }
                let child_transform = self.child_transform(child, parent_style_transform);
                let is_item = is_flex_or_grid(self.node);
                let level = if is_painted_above_text(child, is_item) {
                    Some(ElementLevel::Positioned)
                } else {
                    is_item.then_some(ElementLevel::Foreground)
                };
                if let Some(level) = level {
                    self.context
                        .deferred_paints
                        .borrow_mut()
                        .push(DeferredPaint::Element {
                            node_id: *child_id,
                            transform: child_transform,
                            clip_rect,
                            level,
                        });
                } else {
                    self.render_node(scene, *child_id, child_transform, clip_rect);
                }
            }
        }

        if let Some(base) = flush_from {
            self.context.flush_deferred_paints(scene, base);
        }

        // Positive z_index hoisted nodes
        if let Some(hoisted) = &self.node.stacking_context {
            for hoisted_child in hoisted.pos_z_hoisted_children() {
                let position = hoisted_child.position(self.node.tree(), self.node.id);
                let pos = kurbo::Vec2 {
                    x: position.x as f64 * self.scale,
                    y: position.y as f64 * self.scale,
                };
                self.render_node(
                    scene,
                    hoisted_child.node_id,
                    parent_style_transform.pre_translate(pos),
                    clip_rect,
                );
            }
        }
    }

    #[cfg(feature = "svg")]
    fn draw_svg(&self, scene: &mut impl PaintScene) {
        use style::properties::generated::longhands::object_fit::computed_value::T as ObjectFit;

        let Some(svg) = self.svg else {
            return;
        };

        let width = self.frame.content_box.width() as f32;
        let height = self.frame.content_box.height() as f32;
        let svg_size = svg.tree.size();

        let x = self.frame.content_box.origin().x;
        let y = self.frame.content_box.origin().y;

        // An SVG with an aspect ratio but no natural size is sized by the default sizing
        // algorithm, which for `none` and `scale-down` contains it within the content box.
        let object_fit = match self.style.clone_object_fit() {
            ObjectFit::None | ObjectFit::ScaleDown
                if svg.intrinsic_width().is_none()
                    && svg.intrinsic_height().is_none()
                    && svg.viewbox_aspect_ratio().is_some() =>
            {
                ObjectFit::Contain
            }
            object_fit => object_fit,
        };
        let object_position = self.style.get_object_position();

        // Apply object-fit algorithm
        let container_size = taffy::Size { width, height };
        let object_size = taffy::Size {
            width: svg_size.width(),
            height: svg_size.height(),
        };
        let paint_size = compute_object_fit(container_size, Some(object_size), object_fit);

        // Compute object-position
        let x_offset = object_position.horizontal.resolve(
            CSSPixelLength::new(container_size.width - paint_size.width) / self.scale as f32,
        ) * self.scale as f32;
        let y_offset = object_position.vertical.resolve(
            CSSPixelLength::new(container_size.height - paint_size.height) / self.scale as f32,
        ) * self.scale as f32;
        let x = x + x_offset.px() as f64;
        let y = y + y_offset.px() as f64;

        let viewport = svg_viewport_affine(svg, paint_size.width, paint_size.height);
        let transform = (self.transform * viewport).then_translate(Vec2 { x, y });

        self.with_replaced_content_clip(
            scene,
            container_size,
            paint_size,
            x_offset.px(),
            y_offset.px(),
            |scene| {
                anyrender_svg::render_svg_tree(scene, &svg.tree, transform);
            },
        );
    }

    /// Clip replaced content to the content box when its object-fit/object-position
    /// rect overflows it.
    fn with_replaced_content_clip<S: PaintScene>(
        &self,
        scene: &mut S,
        container_size: taffy::Size<f32>,
        paint_size: taffy::Size<f32>,
        x_offset: f32,
        y_offset: f32,
        paint: impl FnOnce(&mut S),
    ) {
        let overflows = x_offset < 0.0
            || y_offset < 0.0
            || x_offset + paint_size.width > container_size.width
            || y_offset + paint_size.height > container_size.height;
        self.context.layer_manager.maybe_with_layer(
            scene,
            overflows,
            BlendMode::default(),
            1.0,
            self.transform,
            &self.frame.content_box_path(),
            None,
            None,
            paint,
        );
    }

    fn draw_image(&self, scene: &mut impl PaintScene) {
        if let Some(image) = self.element.raster_image_data() {
            let width = self.frame.content_box.width() as u32;
            let height = self.frame.content_box.height() as u32;
            let x = self.frame.content_box.origin().x;
            let y = self.frame.content_box.origin().y;

            let object_fit = self.style.clone_object_fit();
            let object_position = self.style.get_object_position();
            let image_rendering = self.style.clone_image_rendering();
            let quality = to_image_quality(image_rendering);

            // Apply object-fit algorithm
            let container_size = taffy::Size {
                width: width as f32,
                height: height as f32,
            };
            let object_size = taffy::Size {
                width: image.width as f32,
                height: image.height as f32,
            };
            let paint_size = compute_object_fit(container_size, Some(object_size), object_fit);

            // Compute object-position
            let x_offset = object_position.horizontal.resolve(
                CSSPixelLength::new(container_size.width - paint_size.width) / self.scale as f32,
            ) * self.scale as f32;
            let y_offset = object_position.vertical.resolve(
                CSSPixelLength::new(container_size.height - paint_size.height) / self.scale as f32,
            ) * self.scale as f32;
            let x = x + x_offset.px() as f64;
            let y = y + y_offset.px() as f64;

            let x_scale = paint_size.width as f64 / object_size.width as f64;
            let y_scale = paint_size.height as f64 / object_size.height as f64;
            let transform = self
                .transform
                .pre_translate(Vec2 { x, y })
                .pre_scale_non_uniform(x_scale, y_scale);

            self.with_replaced_content_clip(
                scene,
                container_size,
                paint_size,
                x_offset.px(),
                y_offset.px(),
                |scene| {
                    scene.draw_image(to_peniko_image(image, quality).as_ref(), transform);
                },
            );
        }
    }

    #[cfg(feature = "custom-widget")]
    fn draw_custom_widget(&self, scene: &mut impl PaintScene) {
        if let Some(widget_scene) = self.custom_widget_scene {
            let x = self.frame.content_box.origin().x;
            let y = self.frame.content_box.origin().y;
            let transform = self.transform.then_translate(Vec2 { x, y });

            // TODO: eliminate clone
            scene.append_scene(widget_scene.clone(), transform);
        }
    }

    fn draw_sub_document(&self, scene: &mut impl PaintScene) {
        if let Some(sub_doc) = self.element.sub_doc_data().map(|doc| doc.inner()) {
            let scale = self.scale;
            let width = self.frame.content_box.width() as u32;
            let height = self.frame.content_box.height() as u32;

            // TODO: Support arbitrary transforms of subdocuments
            let translation = self.transform.translation();
            let initial_x = translation.x + self.frame.content_box.origin().x;
            let initial_y = translation.y + self.frame.content_box.origin().y;
            // let transform = self.transform.then_translate(Vec2 { x, y });

            let painter = BlitzDomPainter::new(
                &sub_doc,
                scale,
                width,
                height,
                initial_x,
                initial_y,
                self.custom_widget_scenes,
            );
            painter.paint_scene(scene);
        }
    }

    fn stroke_devtools(&self, scene: &mut impl PaintScene) {
        if self.devtools.show_layout {
            let shape = &self.frame.border_box;
            let stroke = Stroke::new(self.scale);

            let stroke_color = match self.node.taffy_display() {
                taffy::Display::Block | taffy::Display::FlowRoot => {
                    Color::new([1.0, 0.0, 0.0, 1.0])
                }
                taffy::Display::Flex => Color::new([0.0, 1.0, 0.0, 1.0]),
                taffy::Display::Grid => Color::new([0.0, 0.0, 1.0, 1.0]),
                taffy::Display::None => Color::new([0.0, 0.0, 1.0, 1.0]),
            };

            scene.stroke(&stroke, self.transform, stroke_color, None, &shape);
        }
    }
}
impl<'dom, 'a> std::ops::Deref for ElementCx<'dom, 'a> {
    type Target = BlitzDomPainter<'dom, 'a>;
    fn deref(&self) -> &Self::Target {
        self.context
    }
}

fn insets_from_taffy_rect(input: taffy::Rect<f64>) -> Insets {
    Insets {
        x0: input.left,
        y0: input.top,
        x1: input.right,
        y1: input.bottom,
    }
}

/// How many lines of an inline layout are painted, and where lines are cut short with an
/// ellipsis. `line-clamp` hides the lines after the last kept one, which ends in an ellipsis.
/// `text-overflow: ellipsis` cuts any line that overflows a clipping box. `width` is the
/// content box width in device pixels.
fn line_ellipsis(
    computed: &ComputedValues,
    layout: &parley::Layout<blitz_dom::node::TextBrush>,
    width: f32,
    inherited_line: Option<usize>,
) -> (usize, Option<LineEllipsis>) {
    let clamped_lines = blitz_dom::util::clamped_line_count(computed, layout);
    let on_overflow = matches!(
        computed.get_text().text_overflow.second,
        style::values::specified::text::TextOverflowSide::Ellipsis
    ) && computed.get_box().overflow_x
        != style::values::computed::Overflow::Visible;
    let on_line = clamped_lines.map(|lines| lines - 1).or(inherited_line);
    let ellipsis = (on_overflow || on_line.is_some()).then(|| LineEllipsis {
        width,
        on_overflow,
        on_line,
        rtl: computed.get_inherited_box().direction == style::computed_values::direction::T::Rtl,
    });
    let visible_lines = clamped_lines
        .filter(|_| blitz_dom::util::clamp_hides_lines(computed))
        .unwrap_or(usize::MAX);
    (visible_lines, ellipsis)
}

/// Convert Stylo and Taffy types into Kurbo types
fn create_css_rect(style: &ComputedValues, layout: &Layout, scale: f64) -> CssBox {
    // Resolve and rescale
    // We have to scale since document pixels are not same same as rendered pixels
    let width: f64 = layout.size.width as f64;
    let height: f64 = layout.size.height as f64;
    let border_box = Rect::new(0.0, 0.0, width * scale, height * scale);
    let border = insets_from_taffy_rect(layout.border.map(|p| p as f64 * scale));
    let padding = insets_from_taffy_rect(layout.padding.map(|p| p as f64 * scale));
    let outline_width = style.get_outline().outline_width.0.to_f64_px() * scale;

    // Resolve the radii to a length. need to downscale since the radii are in document pixels
    let resolve_w = CSSPixelLength::new(width as _);
    let resolve_h = CSSPixelLength::new(height as _);
    let resolve_radii = |radius: &BorderCornerRadius| -> Vec2 {
        Vec2 {
            x: scale * radius.0.width.0.resolve(resolve_w).px() as f64,
            y: scale * radius.0.height.0.resolve(resolve_h).px() as f64,
        }
    };
    let s_border = style.get_border();
    let border_radii = NonUniformRoundedRectRadii {
        top_left: resolve_radii(&s_border.border_top_left_radius),
        top_right: resolve_radii(&s_border.border_top_right_radius),
        bottom_right: resolve_radii(&s_border.border_bottom_right_radius),
        bottom_left: resolve_radii(&s_border.border_bottom_left_radius),
    };

    CssBox::new(border_box, border, padding, outline_width, border_radii)
}
