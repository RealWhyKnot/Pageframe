use super::{ALMOST_OPAQUE, ElementCx, to_image_quality, to_peniko_image};
use crate::color::{Color, ToColorColor};
use crate::gradient::to_peniko_gradient;
use anyrender::PaintScene;
use kurbo::{self, Affine, BezPath, Point, Rect, Shape as _, Size, Vec2};
use num_traits::ToPrimitive as _;
use pageframe_dom::CopyStyleGetters;
use pageframe_dom::node::{ImageData, ImageResourceData, SpecialElementData, SvgImageData};
use pageframe_dom::{BaseDocument, Node, NodeId, local_name};
use peniko::{self, BlendMode, Compose, Fill, Mix};
use std::sync::Arc;
use style::values::generics::color::GenericColor;
use style::{
    properties::{
        generated::longhands::{
            background_attachment::single_value::computed_value::T as StyloBackgroundAttachment,
            background_clip::single_value::computed_value::T as StyloBackgroundClip,
            background_origin::single_value::computed_value::T as StyloBackgroundOrigin,
            visibility::computed_value::T as Visibility,
        },
        style_structs::{Background, SVG},
    },
    values::{
        computed::{
            BackgroundRepeat, Gradient as StyloGradient, Image as ComputedImage, LengthPercentage,
            background::BackgroundSize,
        },
        generics::image::GenericImage,
        specified::{background::BackgroundRepeatKeyword, box_::DisplayInside},
    },
};
use taffy::Layout;

#[cfg(feature = "tracing")]
use tracing::warn;

const MAX_TILES: u64 = 500;
const MAX_TILE_SIDE: u32 = 4096;

/// A box from the CSS box model. Abstracts over the (structurally identical)
/// computed value types of the `background-clip`/`background-origin` and
/// `mask-clip`/`mask-origin` properties.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)] // The variants are named after the CSS keywords
pub(super) enum BoxModelBox {
    BorderBox,
    PaddingBox,
    ContentBox,
}

// Also covers MaskClip as the type is the same
impl From<StyloBackgroundClip> for BoxModelBox {
    fn from(value: StyloBackgroundClip) -> Self {
        match value {
            StyloBackgroundClip::BorderBox => Self::BorderBox,
            StyloBackgroundClip::PaddingBox => Self::PaddingBox,
            StyloBackgroundClip::ContentBox => Self::ContentBox,

            // TODO: support BorderArea
            StyloBackgroundClip::BorderArea => Self::BorderBox,
            StyloBackgroundClip::Text => Self::BorderBox,
        }
    }
}

// Also covers MaskOrigin as the type is the same
impl From<StyloBackgroundOrigin> for BoxModelBox {
    fn from(value: StyloBackgroundOrigin) -> Self {
        match value {
            StyloBackgroundOrigin::BorderBox => Self::BorderBox,
            StyloBackgroundOrigin::PaddingBox => Self::PaddingBox,
            StyloBackgroundOrigin::ContentBox => Self::ContentBox,
        }
    }
}

/// The styles and image data for a single layer of a CSS image layer list
/// (`background-image` or `mask-image`). The `background-*` and `mask-*`
/// properties share computed value types, which allows the layer painting code
/// to be shared.
pub(super) struct ImageLayerStyles<'a> {
    /// The computed value of the `background-image`/`mask-image` layer
    pub stylo_image: &'a ComputedImage,
    /// The loaded image resource if `stylo_image` is a `url()` image
    pub image_data: Option<&'a ImageResourceData>,
    pub position_x: &'a LengthPercentage,
    pub position_y: &'a LengthPercentage,
    pub repeat: &'a BackgroundRepeat,
    pub size: &'a BackgroundSize,
    pub clip: BoxModelBox,
    pub text_clip: bool,
    pub origin: BoxModelBox,
    pub attachment: StyloBackgroundAttachment,
}

impl<'a> ImageLayerStyles<'a> {
    pub(super) fn from_background(
        bg_styles: &'a Background,
        image_data: &'a [Option<ImageResourceData>],
        idx: usize,
    ) -> Self {
        Self {
            stylo_image: &bg_styles.background_image.0[idx],
            image_data: image_data.get(idx).and_then(Option::as_ref),
            position_x: get_cyclic(&bg_styles.background_position_x.0, idx),
            position_y: get_cyclic(&bg_styles.background_position_y.0, idx),
            repeat: get_cyclic(&bg_styles.background_repeat.0, idx),
            size: get_cyclic(&bg_styles.background_size.0, idx),
            clip: (*get_cyclic(&bg_styles.background_clip.0, idx)).into(),
            text_clip: *get_cyclic(&bg_styles.background_clip.0, idx) == StyloBackgroundClip::Text,
            origin: (*get_cyclic(&bg_styles.background_origin.0, idx)).into(),
            attachment: *get_cyclic(&bg_styles.background_attachment.0, idx),
        }
    }

    pub(super) fn from_svg(
        svg_styles: &'a SVG,
        image_data: &'a [Option<ImageResourceData>],
        idx: usize,
    ) -> Self {
        Self {
            stylo_image: &svg_styles.mask_image.0[idx],
            image_data: image_data.get(idx).and_then(Option::as_ref),
            position_x: get_cyclic(&svg_styles.mask_position_x.0, idx),
            position_y: get_cyclic(&svg_styles.mask_position_y.0, idx),
            repeat: get_cyclic(&svg_styles.mask_repeat.0, idx),
            size: get_cyclic(&svg_styles.mask_size.0, idx),
            clip: (*get_cyclic(&svg_styles.mask_clip.0, idx)).into(),
            text_clip: false,
            origin: (*get_cyclic(&svg_styles.mask_origin.0, idx)).into(),
            // There is no `mask-attachment` property
            attachment: StyloBackgroundAttachment::Scroll,
        }
    }
}

impl ElementCx<'_, '_> {
    pub(super) fn draw_background(&self, scene: &mut impl PaintScene, outer_clipped: bool) {
        if Some(self.node.id) == self.context.root_element_id {
            self.draw_canvas_background(scene);
            return;
        }
        if Some(self.node.id) == self.context.canvas_source {
            return;
        }
        self.draw_lifted_cell_tracks(scene);
        let bg_styles = &self.style.get_background();
        let image_data = &self.element.background_images;
        let layer_count = bg_styles.background_image.0.len();

        // The background color is clipped by the clip of the last layer in the list
        let last_clip = *get_cyclic(&bg_styles.background_clip.0, layer_count - 1);
        let background_clip_path = self.background_path(
            self.clip_box(last_clip.into()),
            outer_clipped && last_clip != StyloBackgroundClip::Text,
        );

        // Draw background color (if any)
        let mut text_run = last_clip == StyloBackgroundClip::Text;
        if text_run {
            self.begin_text_clip_group(scene);
        }
        self.draw_solid_bg(scene, &background_clip_path);

        for idx in (0..layer_count).rev() {
            let layer = ImageLayerStyles::from_background(bg_styles, image_data, idx);
            if layer.text_clip != text_run {
                if text_run {
                    self.end_text_clip_group(scene);
                } else {
                    self.begin_text_clip_group(scene);
                }
                text_run = layer.text_clip;
            }
            if layer_paints_nothing(&layer) {
                continue;
            }
            let background_clip_path =
                self.background_path(self.clip_box(layer.clip), outer_clipped && !layer.text_clip);

            self.context.layer_manager.maybe_with_layer(
                scene,
                true,
                peniko::BlendMode::default(),
                1.0,
                self.transform,
                &background_clip_path,
                None,
                None,
                |scene| {
                    self.draw_image_layer(scene, &layer, self.box_rect(layer.clip));
                },
            );
        }
        if text_run {
            self.end_text_clip_group(scene);
        }
    }

    pub(super) fn draw_canvas_background(&self, scene: &mut impl PaintScene) {
        let Some(source) = self
            .context
            .canvas_source
            .and_then(|id| self.context.dom.get_node(id))
        else {
            return;
        };
        let canvas = self.canvas_rect();
        if source.id == self.node.id {
            self.draw_canvas_layers(scene, canvas);
            return;
        }
        let mut donor = self
            .context
            .element_cx(source, Layout::new(), self.transform, None);
        donor.frame = self.frame.clone();
        donor.draw_canvas_layers(scene, canvas);
    }

    fn draw_canvas_layers(&self, scene: &mut impl PaintScene, canvas: Rect) {
        let bg_styles = &self.style.get_background();
        let image_data = &self.element.background_images;
        let clip = canvas.to_path(0.1);
        for idx in (0..bg_styles.background_image.0.len()).rev() {
            let layer = ImageLayerStyles::from_background(bg_styles, image_data, idx);
            if layer_paints_nothing(&layer) {
                continue;
            }
            self.context.layer_manager.maybe_with_layer(
                scene,
                true,
                peniko::BlendMode::default(),
                1.0,
                self.transform,
                &clip,
                None,
                None,
                |scene| self.draw_image_layer(scene, &layer, canvas),
            );
        }
    }

    fn canvas_rect(&self) -> Rect {
        let context = self.context;
        let viewport = Rect::from_origin_size(
            (context.initial_x, context.initial_y),
            (f64::from(context.width), f64::from(context.height)),
        );
        let border_box = self.frame.border_box;
        if self.transform.determinant().abs() < f64::EPSILON {
            return border_box;
        }
        self.transform
            .inverse()
            .transform_rect_bbox(viewport)
            .union(border_box)
    }

    /// Starts an isolated group for `background-clip: text` layers; layers drawn
    /// until [`end_text_clip_group`](Self::end_text_clip_group) are clipped to text.
    fn begin_text_clip_group(&self, scene: &mut impl PaintScene) {
        scene.push_layer(
            Fill::NonZero,
            Mix::Normal,
            ALMOST_OPAQUE,
            self.transform,
            &self.frame.border_box_path(),
            None,
            None,
        );
    }

    /// Ends the group started by [`begin_text_clip_group`](Self::begin_text_clip_group):
    /// draws the text mask in a `DestIn` layer, then pops both layers.
    fn end_text_clip_group(&self, scene: &mut impl PaintScene) {
        scene.push_layer(
            Fill::NonZero,
            BlendMode::new(Mix::Normal, Compose::DestIn),
            1.0,
            self.transform,
            &self.frame.border_box_path(),
            None,
            None,
        );
        self.context
            .draw_text_mask(scene, self.node.id, self.transform, true);
        scene.pop_layer();
        scene.pop_layer();
    }

    fn clip_box(&self, clip: BoxModelBox) -> BoxModelBox {
        let is_cell = self.style.clone_display().inside() == DisplayInside::TableCell;
        if clip == BoxModelBox::BorderBox
            && is_cell
            && self.has_collapsed_borders()
            && !self.paints_under_cell_borders()
        {
            BoxModelBox::PaddingBox
        } else {
            clip
        }
    }

    fn background_path(&self, css_box: BoxModelBox, outer_clipped: bool) -> BezPath {
        if css_box == BoxModelBox::BorderBox && outer_clipped {
            return self.frame.border_box.to_path(0.1);
        }
        match (css_box, self.covered_border_share()) {
            (BoxModelBox::BorderBox, Some(share)) => {
                self.frame.border_slice(0.0, share).padding_box_path()
            }
            _ => self.box_path(css_box),
        }
    }

    /// The path of the given CSS box model box for this element
    pub(super) fn box_path(&self, css_box: BoxModelBox) -> BezPath {
        match css_box {
            BoxModelBox::BorderBox => self.frame.border_box_path(),
            BoxModelBox::PaddingBox => self.frame.padding_box_path(),
            BoxModelBox::ContentBox => self.frame.content_box_path(),
        }
    }

    /// The rect of the given CSS box model box for this element
    pub(super) fn box_rect(&self, css_box: BoxModelBox) -> Rect {
        match css_box {
            BoxModelBox::BorderBox => self.frame.border_box,
            BoxModelBox::PaddingBox => self.frame.padding_box,
            BoxModelBox::ContentBox => self.frame.content_box,
        }
    }

    /// Draw a single layer of a CSS image layer list (`background-image` or `mask-image`)
    pub(super) fn draw_image_layer(
        &self,
        scene: &mut impl PaintScene,
        layer: &ImageLayerStyles,
        clip_rect: Rect,
    ) {
        match layer.stylo_image {
            GenericImage::None => {
                // Do nothing
            }
            GenericImage::Gradient(gradient) => {
                self.draw_gradient_layer(scene, gradient, layer, clip_rect);
            }
            GenericImage::Url(_) => {
                self.draw_raster_image_layer(scene, layer, clip_rect);
                #[cfg(feature = "svg")]
                self.draw_svg_image_layer(scene, layer, clip_rect);
            }
            GenericImage::LightDark(_) => {
                #[cfg(feature = "tracing")]
                warn!("Implement image layer drawing for ImageLightDark")
            }
            GenericImage::PaintWorklet(_) => {
                #[cfg(feature = "tracing")]
                warn!("Implement image layer drawing for Image::PaintWorklet")
            }
            GenericImage::CrossFade(_) => {
                #[cfg(feature = "tracing")]
                warn!("Implement image layer drawing for Image::CrossFade")
            }
            GenericImage::Image(_) => {
                #[cfg(feature = "tracing")]
                warn!("Implement image layer drawing for Image::Image")
            }
            GenericImage::ImageSet(_) => {
                #[cfg(feature = "tracing")]
                warn!("Implement image layer drawing for Image::ImageSet")
            }
        }
    }

    fn draw_lifted_cell_tracks(&self, scene: &mut impl PaintScene) {
        if !self.node.is_in_lifted_row() {
            return;
        }
        let dom = &self.context.dom;
        let Some(table_node) = self
            .node
            .layout_parent
            .get()
            .and_then(|id| dom.get_node(id))
        else {
            return;
        };
        let Some(SpecialElementData::TableRoot(table)) = table_node
            .element_data()
            .map(|element| &element.special_data)
        else {
            return;
        };
        let Some(cell) = table.cells.iter().find(|cell| cell.node_id == self.node.id) else {
            return;
        };
        let column = table.columns.get(cell.column);
        let row = table.rows.get(usize::from(cell.row));
        let parts = [
            column.and_then(|column| column.group),
            column.map(|column| column.node_id),
            row.and_then(|row| row.group),
            row.map(|row| row.node_id),
        ];
        let layout = self.node.final_layout();
        let border = if self.has_collapsed_borders() {
            layout.border
        } else {
            taffy::Rect::ZERO
        };
        let shape = Rect::new(
            f64::from(border.left),
            f64::from(border.top),
            f64::from(layout.size.width - border.right),
            f64::from(layout.size.height - border.bottom),
        )
        .scale_from_origin(self.scale);
        for part in parts.into_iter().flatten() {
            let color = dom.get_node(part).and_then(|node| {
                let style = node.primary_styles()?;
                let color = style
                    .get_background()
                    .background_color
                    .resolve_to_absolute(&style.clone_color())
                    .as_srgb_color();
                (color != Color::TRANSPARENT).then_some(color)
            });
            if let Some(color) = color {
                scene.fill(Fill::NonZero, self.transform, color, None, &shape);
            }
        }
    }

    /// Paint the backgrounds of the table's column groups, columns, row groups and rows
    /// (in that order, bottom to top: CSS 2.1 §17.5.1).
    ///
    /// These backgrounds are only painted behind the table's cells (not in the
    /// `border-spacing` between them, nor where a row has no cell), so they are
    /// painted cell by cell.
    pub(super) fn draw_table_row_backgrounds(&self, scene: &mut impl PaintScene) {
        let SpecialElementData::TableRoot(table) = &self.element.special_data else {
            return;
        };

        let background_color = |node_id: NodeId| -> Option<Color> {
            let node = self.context.dom.get_node(node_id)?;
            let style = node.primary_styles()?;
            let current_color = style.clone_color();
            let color = style
                .get_background()
                .background_color
                .resolve_to_absolute(&current_color)
                .as_srgb_color();
            (color != Color::TRANSPARENT).then_some(color)
        };

        // Resolve the (group, track) background colors once per track rather than once per cell
        let column_colors: Vec<[Option<Color>; 2]> = table
            .columns
            .iter()
            .map(|col| {
                [
                    col.group.and_then(background_color),
                    background_color(col.node_id),
                ]
            })
            .collect();
        let row_colors: Vec<[Option<Color>; 2]> = table
            .rows
            .iter()
            .map(|row| {
                [
                    row.group.and_then(background_color),
                    background_color(row.node_id),
                ]
            })
            .collect();
        let has_color =
            |colors: &[[Option<Color>; 2]]| colors.iter().flatten().any(Option::is_some);
        if !has_color(&column_colors) && !has_color(&row_colors) {
            return;
        }

        let is_collapsed = self.has_collapsed_borders();
        let cell_rect = |cell_id: NodeId| -> Option<Rect> {
            let cell = self.context.dom.get_node(cell_id)?;
            let layout = cell.final_layout();
            let border = if is_collapsed && self.has_cell_layer(cell) {
                layout.border
            } else {
                taffy::Rect::ZERO
            };
            Some(Rect::new(
                f64::from(layout.location.x + border.left),
                f64::from(layout.location.y + border.top),
                f64::from(layout.location.x + layout.size.width - border.right),
                f64::from(layout.location.y + layout.size.height - border.bottom),
            ))
        };

        for cell in &table.cells {
            let no_colors = [None, None];
            let column = column_colors.get(cell.column).unwrap_or(&no_colors);
            let row = row_colors.get(cell.row as usize).unwrap_or(&no_colors);
            let mut colors = column.iter().chain(row).flatten().peekable();
            if colors.peek().is_none() {
                continue;
            }
            let Some(cell_node) = self.context.dom.get_node(cell.node_id) else {
                continue;
            };
            let is_hidden = cell_node
                .primary_styles()
                .is_some_and(|style| style.get_inherited_box().visibility != Visibility::Visible);
            if cell_node.is_in_lifted_row() || is_hidden {
                continue;
            }
            let Some(rect) = cell_rect(cell.node_id) else {
                continue;
            };

            let shape = rect.scale_from_origin(self.scale);
            for color in colors {
                scene.fill(Fill::NonZero, self.transform, *color, None, &shape);
            }
        }
    }

    fn draw_solid_bg(&self, scene: &mut impl PaintScene, shape: &BezPath) {
        let current_color = self.style.clone_color();
        let background_color = &self.style.get_background().background_color;
        let bg_color = background_color
            .resolve_to_absolute(&current_color)
            .as_srgb_color();

        if bg_color != Color::TRANSPARENT {
            // Fill the color
            scene.fill(Fill::NonZero, self.transform, bg_color, None, shape);
        }
    }

    /// Whether the layer is positioned against the viewport
    /// (`background-attachment: fixed`) rather than the element's origin box.
    /// `fixed` behaves as `scroll` on elements affected by a CSS transform
    /// (the transformed element acts as the layer's containing block).
    fn layer_is_fixed(&self, layer: &ImageLayerStyles) -> bool {
        layer.attachment == StyloBackgroundAttachment::Fixed && !self.is_transformed()
    }

    /// The background positioning area and the transform from its coordinate
    /// space to the scene for a fixed layer: the viewport, unaffected by any
    /// scrolling.
    fn fixed_positioning_area(&self) -> (Rect, Affine) {
        let viewport_rect = Rect::new(
            0.0,
            0.0,
            self.context.width as f64,
            self.context.height as f64,
        );
        let transform = Affine::translate((self.context.initial_x, self.context.initial_y));
        (viewport_rect, transform)
    }

    /// Whether this element or any of its ancestors has a CSS transform
    ///
    /// TODO: this misses transformed elements whose resolved transform is the
    /// identity (e.g. `transform: translate(0)`), and elements with
    /// `will-change: transform`, both of which should also degrade `fixed`
    /// to `scroll` (see WPT css/css-transforms/transform-fixed-bg-005/008)
    fn is_transformed(&self) -> bool {
        let mut current = Some(self.node.id);
        while let Some(node) = current.and_then(|id| self.context.dom.get_node(id)) {
            if node.transform().is_some() {
                return true;
            }
            current = node.parent;
        }
        false
    }

    #[cfg(feature = "svg")]
    fn draw_svg_image_layer(
        &self,
        scene: &mut impl PaintScene,
        layer: &ImageLayerStyles,
        clip_rect: Rect,
    ) {
        let Some(bg_image) = layer.image_data else {
            return;
        };
        let ImageData::Svg(svg) = &bg_image.image else {
            return;
        };

        // A zero-sized `viewBox` disables rendering of the SVG
        if svg.intrinsic_dimensions.degenerate_view_box {
            return;
        }

        let (origin_rect, base_transform, clip_rect) = if self.layer_is_fixed(layer) {
            let (viewport_rect, transform) = self.fixed_positioning_area();
            (viewport_rect, transform, viewport_rect)
        } else {
            (self.box_rect(layer.origin), self.transform, clip_rect)
        };

        // Size the SVG per the CSS default sizing algorithm
        // (https://drafts.csswg.org/css-images/#default-sizing). An SVG image
        // may lack an intrinsic width, height, and/or aspect ratio, so each is
        // passed separately (usvg's resolved `Tree::size` is only used as the
        // source coordinate space of the rendered tree).
        let intrinsic_width = svg.intrinsic_width().filter(|w| w.is_finite() && *w > 0.0);
        let intrinsic_height = svg.intrinsic_height().filter(|h| h.is_finite() && *h > 0.0);
        let aspect_ratio = match (intrinsic_width, intrinsic_height) {
            (Some(w), Some(h)) => Some(w / h),
            _ => svg.viewbox_aspect_ratio(),
        }
        .filter(|r| r.is_finite() && *r > 0.0);

        let (bg_pos, bg_size) = compute_layer_position_and_size(
            layer,
            origin_rect.width() / self.scale,
            origin_rect.height() / self.scale,
            BackgroundSizeComputeMode::Intrinsic {
                width: intrinsic_width,
                height: intrinsic_height,
                ratio: aspect_ratio,
            },
        );

        if !(bg_size.width > 0.0 && bg_size.height > 0.0) {
            return;
        }

        let bg_pos = (bg_pos.to_vec2() * self.scale).to_point();
        let bg_size = bg_size * self.scale;
        let (Some(viewport_width), Some(viewport_height)) =
            (bg_size.width.to_f32(), bg_size.height.to_f32())
        else {
            return;
        };
        let viewport = super::svg_viewport_affine(svg, viewport_width, viewport_height);

        let BackgroundRepeat(repeat_x, repeat_y) = layer.repeat;
        let horizontal = gradient_axis_tiling(
            *repeat_x,
            (origin_rect.x0, origin_rect.width()),
            (clip_rect.x0, clip_rect.width()),
            bg_pos.x,
            bg_size.width,
        );
        let vertical = gradient_axis_tiling(
            *repeat_y,
            (origin_rect.y0, origin_rect.height()),
            (clip_rect.y0, clip_rect.height()),
            bg_pos.y,
            bg_size.height,
        );
        if u64::from(horizontal.count) * u64::from(vertical.count) <= MAX_TILES {
            for offset in tile_offsets(&horizontal, &vertical) {
                let transform = base_transform * Affine::translate(offset) * viewport;
                anyrender_svg::render_svg_tree(scene, &svg.tree, transform);
            }
            return;
        }

        if let Some(tile) = svg_tile_image(svg, bg_size) {
            fill_repeated_tile(scene, base_transform, tile, bg_size, &horizontal, &vertical);
        }
    }

    fn draw_raster_image_layer(
        &self,
        scene: &mut impl PaintScene,
        layer: &ImageLayerStyles,
        clip_rect: Rect,
    ) {
        let Some(bg_image) = layer.image_data else {
            return;
        };
        let ImageData::Raster(image_data) = &bg_image.image else {
            return;
        };

        let image_rendering = self.style.clone_image_rendering();
        let quality = to_image_quality(image_rendering);

        let (origin_rect, base_transform, clip_rect) = if self.layer_is_fixed(layer) {
            let (viewport_rect, transform) = self.fixed_positioning_area();
            (viewport_rect, transform, viewport_rect)
        } else {
            (self.box_rect(layer.origin), self.transform, clip_rect)
        };

        let image_width = image_data.width as f64;
        let image_height = image_data.height as f64;

        let (bg_pos, bg_size) = compute_layer_position_and_size(
            layer,
            origin_rect.width() / self.scale,
            origin_rect.height() / self.scale,
            BackgroundSizeComputeMode::Size(image_width as f32, image_height as f32),
        );

        let bg_pos = (bg_pos.to_vec2() * self.scale).to_point();
        let bg_size = bg_size * self.scale;

        let x_ratio = bg_size.width / image_width;
        let y_ratio = bg_size.height / image_height;

        let BackgroundRepeat(repeat_x, repeat_y) = layer.repeat;

        let x = raster_axis_tiling(
            *repeat_x,
            (origin_rect.x0, origin_rect.width()),
            (clip_rect.x0, clip_rect.width()),
            bg_pos.x,
            bg_size.width,
            image_width,
            x_ratio,
        );
        let y = raster_axis_tiling(
            *repeat_y,
            (origin_rect.y0, origin_rect.height()),
            (clip_rect.y0, clip_rect.height()),
            bg_pos.y,
            bg_size.height,
            image_height,
            y_ratio,
        );

        let transform = base_transform
            .pre_scale_non_uniform(x_ratio, y_ratio)
            .then_translate(Vec2 {
                x: x.translate,
                y: y.translate,
            });
        let tile_rect = Rect::new(0.0, 0.0, x.rect_len, y.rect_len);

        for hc in 0..y.count {
            for wc in 0..x.count {
                let transform = transform.then_translate(Vec2 {
                    x: wc as f64 * x.stride,
                    y: hc as f64 * y.stride,
                });

                scene.fill(
                    peniko::Fill::NonZero,
                    transform,
                    to_peniko_image(image_data, quality).as_ref(),
                    None,
                    &tile_rect,
                );
            }
        }
    }

    fn draw_gradient_layer(
        &self,
        scene: &mut impl PaintScene,
        gradient: &StyloGradient,
        layer: &ImageLayerStyles,
        clip_rect: Rect,
    ) {
        // For a fixed layer the positioning area (the viewport) already covers
        // everything visible, so it also serves as the clip rect (no extension
        // towards the clip box is needed).
        let (origin_rect, base_transform, clip_rect) = if self.layer_is_fixed(layer) {
            let (viewport_rect, transform) = self.fixed_positioning_area();
            (viewport_rect, transform, viewport_rect)
        } else {
            (self.box_rect(layer.origin), self.transform, clip_rect)
        };

        let (bg_pos, bg_size) = compute_layer_position_and_size(
            layer,
            origin_rect.width() / self.scale,
            origin_rect.height() / self.scale,
            BackgroundSizeComputeMode::Auto,
        );

        let bg_pos = (bg_pos.to_vec2() * self.scale).to_point();
        let bg_size = bg_size * self.scale;

        let BackgroundRepeat(repeat_x, repeat_y) = layer.repeat;

        let x = gradient_axis_tiling(
            *repeat_x,
            (origin_rect.x0, origin_rect.width()),
            (clip_rect.x0, clip_rect.width()),
            bg_pos.x,
            bg_size.width,
        );
        let y = gradient_axis_tiling(
            *repeat_y,
            (origin_rect.y0, origin_rect.height()),
            (clip_rect.y0, clip_rect.height()),
            bg_pos.y,
            bg_size.height,
        );

        let tile_rect = Rect::new(0.0, 0.0, x.rect_len, y.rect_len);
        let current_color = self.style.clone_color();

        let (gradient, gradient_transform) =
            to_peniko_gradient(gradient, tile_rect, self.scale, &current_color);

        if u64::from(x.count) * u64::from(y.count) > MAX_TILES {
            if let Some(tile) = gradient_tile_image(&gradient, gradient_transform, tile_rect.size())
            {
                fill_repeated_tile(scene, base_transform, tile, tile_rect.size(), &x, &y);
            }
            return;
        }

        let brush = anyrender::Paint::Gradient(&gradient);

        let transform = base_transform.then_translate(Vec2 {
            x: x.translate,
            y: y.translate,
        });

        for hc in 0..y.count {
            for wc in 0..x.count {
                let transform = transform.then_translate(Vec2 {
                    x: wc as f64 * x.stride,
                    y: hc as f64 * y.stride,
                });

                scene.fill(
                    peniko::Fill::NonZero,
                    transform,
                    brush.clone(),
                    gradient_transform,
                    &tile_rect,
                );
            }
        }
    }
}

fn compute_layer_position_and_size(
    layer: &ImageLayerStyles,
    container_w: f64,
    container_h: f64,
    size_mode: BackgroundSizeComputeMode,
) -> (Point, Size) {
    use BackgroundRepeatKeyword::*;

    let bg_size = compute_layer_size(layer, container_w as f32, container_h as f32, size_mode);

    let bg_pos = compute_layer_position(
        layer,
        (container_w - bg_size.width) as f32,
        (container_h - bg_size.height) as f32,
    );

    let BackgroundRepeat(repeat_x, repeat_y) = layer.repeat;

    let bg_size = if matches!(repeat_x, Round) && matches!(repeat_y, Round) {
        let count = (container_w / bg_size.width).round();
        let width = container_w / count;

        let count = (container_h / bg_size.height).round();
        let height = container_h / count;

        Size::new(width, height)
    } else if matches!(repeat_x, Round) {
        let count = (container_w / bg_size.width).round();
        let width = container_w / count;
        Size::new(width, bg_size.height)
    } else if matches!(repeat_y, Round) {
        let count = (container_h / bg_size.height).round();
        let height = container_h / count;
        Size::new(bg_size.width, height)
    } else {
        bg_size
    };

    (bg_pos, bg_size)
}

#[inline]
fn compute_layer_position(layer: &ImageLayerStyles, width: f32, height: f32) -> Point {
    use style::values::computed::Length;

    let bg_pos_x = layer.position_x.resolve(Length::new(width)).px() as f64;
    let bg_pos_y = layer.position_y.resolve(Length::new(height)).px() as f64;

    Point::new(bg_pos_x, bg_pos_y)
}

fn compute_layer_size(
    layer: &ImageLayerStyles,
    container_w: f32,
    container_h: f32,
    mode: BackgroundSizeComputeMode,
) -> kurbo::Size {
    use style::values::computed::Length;
    use style::values::generics::length::GenericLengthPercentageOrAuto as Lpa;

    let (width, height): (f32, f32) = match layer.size {
        BackgroundSize::ExplicitSize { width, height } => {
            let width = width.map(|w| w.0.resolve(Length::new(container_w)));
            let height = height.map(|h| h.0.resolve(Length::new(container_h)));

            match (width, height) {
                (Lpa::LengthPercentage(width), Lpa::LengthPercentage(height)) => {
                    let width = width.px();
                    let height = height.px();
                    match mode {
                        BackgroundSizeComputeMode::Auto => (width, height),
                        BackgroundSizeComputeMode::Size(_, _) => (width, height),
                        BackgroundSizeComputeMode::Intrinsic { .. } => (width, height),
                    }
                }
                (Lpa::LengthPercentage(width), Lpa::Auto) => {
                    let width = width.px();
                    let height = match mode {
                        BackgroundSizeComputeMode::Auto => container_h,
                        BackgroundSizeComputeMode::Size(bg_w, bg_h) => bg_h / bg_w * width,
                        BackgroundSizeComputeMode::Intrinsic { height, ratio, .. } => ratio
                            .map(|ratio| width / ratio)
                            .or(height)
                            .unwrap_or(container_h),
                    };
                    (width, height)
                }
                (Lpa::Auto, Lpa::LengthPercentage(height)) => {
                    let height = height.px();
                    let width = match mode {
                        BackgroundSizeComputeMode::Auto => container_w,
                        BackgroundSizeComputeMode::Size(bg_w, bg_h) => bg_w / bg_h * height,
                        BackgroundSizeComputeMode::Intrinsic { width, ratio, .. } => ratio
                            .map(|ratio| height * ratio)
                            .or(width)
                            .unwrap_or(container_w),
                    };
                    (width, height)
                }
                (Lpa::Auto, Lpa::Auto) => match mode {
                    BackgroundSizeComputeMode::Auto => (container_w, container_h),
                    BackgroundSizeComputeMode::Size(bg_w, bg_h) => (bg_w, bg_h),
                    BackgroundSizeComputeMode::Intrinsic {
                        width,
                        height,
                        ratio,
                    } => default_sizing(width, height, ratio, container_w, container_h),
                },
            }
        }
        BackgroundSize::Cover => match mode {
            BackgroundSizeComputeMode::Auto => (container_w, container_h),
            BackgroundSizeComputeMode::Size(bg_w, bg_h) => {
                // Scale to the smallest size that covers both axes
                let ratio = (container_w / bg_w).max(container_h / bg_h);
                (bg_w * ratio, bg_h * ratio)
            }
            BackgroundSizeComputeMode::Intrinsic { ratio, .. } => match ratio {
                // Scale the aspect ratio to the smallest size that covers both axes
                Some(ratio) => {
                    let scale = (container_w / ratio).max(container_h);
                    (scale * ratio, scale)
                }
                // No intrinsic aspect ratio: fill the positioning area
                None => (container_w, container_h),
            },
        },
        BackgroundSize::Contain => match mode {
            BackgroundSizeComputeMode::Auto => (container_w, container_h),
            BackgroundSizeComputeMode::Size(bg_w, bg_h) => {
                // Scale to the largest size contained by both axes
                let ratio = (container_w / bg_w).min(container_h / bg_h);
                (bg_w * ratio, bg_h * ratio)
            }
            BackgroundSizeComputeMode::Intrinsic { ratio, .. } => match ratio {
                // Scale the aspect ratio to the largest size contained by both axes
                Some(ratio) => {
                    let scale = (container_w / ratio).min(container_h);
                    (scale * ratio, scale)
                }
                // No intrinsic aspect ratio: fill the positioning area
                None => (container_w, container_h),
            },
        },
    };

    kurbo::Size {
        width: width as f64,
        height: height as f64,
    }
}

enum BackgroundSizeComputeMode {
    Auto,
    Size(f32, f32),
    /// Intrinsic dimensions of an image which may lack an intrinsic width,
    /// height, and/or aspect ratio (e.g. SVG), sized per the CSS default
    /// sizing algorithm (https://drafts.csswg.org/css-images/#default-sizing)
    Intrinsic {
        width: Option<f32>,
        height: Option<f32>,
        ratio: Option<f32>,
    },
}

/// The CSS default sizing algorithm for the unconstrained (`auto auto`) case:
/// resolve the concrete object size from whichever intrinsic dimensions exist,
/// falling back to the default object size (the background positioning area).
pub(super) fn default_sizing(
    width: Option<f32>,
    height: Option<f32>,
    ratio: Option<f32>,
    container_w: f32,
    container_h: f32,
) -> (f32, f32) {
    match (width, height) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => (w, ratio.map(|r| w / r).unwrap_or(container_h)),
        (None, Some(h)) => (ratio.map(|r| h * r).unwrap_or(container_w), h),
        (None, None) => match ratio {
            // Intrinsic aspect ratio only: size as if `contain` were specified
            Some(ratio) => {
                let scale = (container_w / ratio).min(container_h);
                (scale * ratio, scale)
            }
            None => (container_w, container_h),
        },
    }
}

/// The placement and tiling of a background layer along one axis: a
/// translation applied to the layer as a whole, the length of each filled
/// rect, and the number of explicit tiles with the stride between them.
struct AxisTiling {
    /// Translation (in device pixels) positioning the first tile
    translate: f64,
    /// Length of each filled rect, in the coordinate space of the fill's transform
    rect_len: f64,
    /// Number of explicitly drawn tiles
    count: u32,
    /// Stride (in device pixels) between the starts of consecutive tiles
    stride: f64,
}

/// Per-axis placement and tiling for a raster image layer. `Repeat`/`Round`
/// produce a single fill covering the whole positioning area (relying on the
/// image brush repeating), while `Space` produces `count` explicit tiles
/// spaced `stride` apart.
///
/// The fill rect is in image pixel coordinates (the drawing transform is
/// pre-scaled by `ratio`), while translations are in device pixels.
fn raster_axis_tiling(
    repeat: BackgroundRepeatKeyword,
    (origin_start, origin_len): (f64, f64),
    clip: (f64, f64),
    bg_pos: f64,
    tile_len: f64,
    image_len: f64,
    ratio: f64,
) -> AxisTiling {
    use BackgroundRepeatKeyword::*;

    match repeat {
        Repeat | Round => {
            let (area_start, area_len) = tiled_area((origin_start, origin_len), clip);
            let extend_len = extend((origin_start - area_start) + bg_pos, tile_len);
            AxisTiling {
                translate: area_start - extend_len,
                rect_len: (area_len + extend_len) / ratio,
                count: 1,
                stride: 0.0,
            }
        }
        Space => {
            let (count, stride) = compute_space_count_and_stride(origin_len, tile_len);
            AxisTiling {
                translate: origin_start + if count == 1 { bg_pos } else { 0.0 },
                rect_len: image_len,
                count,
                stride,
            }
        }
        NoRepeat => AxisTiling {
            translate: origin_start + bg_pos,
            rect_len: image_len,
            count: 1,
            stride: 0.0,
        },
    }
}

/// Per-axis placement and tiling for a gradient layer. Unlike raster images,
/// gradients cannot rely on brush repetition, so `Repeat`/`Round` also produce
/// explicit tiles. When the clip box extends beyond the origin box, tiling
/// starts from the clip box edge so the pattern covers the whole clipped area.
fn gradient_axis_tiling(
    repeat: BackgroundRepeatKeyword,
    (origin_start, origin_len): (f64, f64),
    clip: (f64, f64),
    bg_pos: f64,
    tile_len: f64,
) -> AxisTiling {
    use BackgroundRepeatKeyword::*;

    match repeat {
        Repeat | Round => {
            let (area_start, area_len) = tiled_area((origin_start, origin_len), clip);
            let extend_len = extend((origin_start - area_start) + bg_pos, tile_len);
            let count = ((area_len + extend_len) / tile_len).ceil() as u32;
            AxisTiling {
                translate: area_start - extend_len,
                rect_len: tile_len,
                count,
                stride: tile_len,
            }
        }
        Space => {
            let (count, stride) = compute_space_count_and_stride(origin_len, tile_len);
            AxisTiling {
                translate: origin_start + if count == 1 { bg_pos } else { 0.0 },
                rect_len: tile_len,
                count,
                stride,
            }
        }
        NoRepeat => AxisTiling {
            translate: origin_start + bg_pos,
            rect_len: tile_len,
            count: 1,
            stride: 0.0,
        },
    }
}

fn tiled_area(
    (origin_start, origin_len): (f64, f64),
    (clip_start, clip_len): (f64, f64),
) -> (f64, f64) {
    let clip_is_outer =
        clip_start < origin_start || clip_start + clip_len > origin_start + origin_len;
    if clip_is_outer {
        (clip_start, clip_len)
    } else {
        (origin_start, origin_len)
    }
}

fn tile_offsets<'a>(
    horizontal: &'a AxisTiling,
    vertical: &'a AxisTiling,
) -> impl Iterator<Item = Vec2> + 'a {
    (0..vertical.count).flat_map(move |row| {
        (0..horizontal.count).map(move |column| {
            Vec2::new(
                horizontal.translate + f64::from(column) * horizontal.stride,
                vertical.translate + f64::from(row) * vertical.stride,
            )
        })
    })
}

struct TileRun {
    start: f64,
    length: f64,
}

fn tile_runs(axis: &AxisTiling) -> Vec<TileRun> {
    if axis.count <= 1 || (axis.stride - axis.rect_len).abs() < 1e-6 {
        let gaps = f64::from(axis.count.saturating_sub(1));
        return vec![TileRun {
            start: axis.translate,
            length: axis.rect_len + axis.stride * gaps,
        }];
    }
    (0..axis.count)
        .map(|index| TileRun {
            start: axis.translate + f64::from(index) * axis.stride,
            length: axis.rect_len,
        })
        .collect()
}

fn fill_repeated_tile(
    scene: &mut impl PaintScene,
    base_transform: Affine,
    tile: peniko::ImageData,
    tile_size: Size,
    horizontal: &AxisTiling,
    vertical: &AxisTiling,
) {
    let tile_scale = Affine::scale_non_uniform(
        tile_size.width / f64::from(tile.width),
        tile_size.height / f64::from(tile.height),
    );
    let brush = peniko::ImageBrush::new(tile).with_extend(peniko::Extend::Repeat);
    let columns = tile_runs(horizontal);
    for row in tile_runs(vertical) {
        for column in &columns {
            scene.fill(
                Fill::NonZero,
                base_transform * Affine::translate((column.start, row.start)),
                brush.as_ref(),
                Some(tile_scale),
                &Rect::new(0.0, 0.0, column.length, row.length),
            );
        }
    }
}

fn tile_side(length: f64) -> Option<u32> {
    length
        .ceil()
        .to_u32()
        .filter(|pixels| (1..=MAX_TILE_SIDE).contains(pixels))
}

fn tile_image(pixels: Vec<u8>, width: u32, height: u32) -> peniko::ImageData {
    peniko::ImageData {
        data: peniko::Blob::new(Arc::new(pixels)),
        format: peniko::ImageFormat::Rgba8,
        alpha_type: peniko::ImageAlphaType::AlphaPremultiplied,
        width,
        height,
    }
}

fn gradient_tile_image(
    gradient: &peniko::Gradient,
    gradient_transform: Option<Affine>,
    size: Size,
) -> Option<peniko::ImageData> {
    let (width, height) = (tile_side(size.width)?, tile_side(size.height)?);
    let fit = Affine::scale_non_uniform(
        f64::from(width) / size.width,
        f64::from(height) / size.height,
    );
    let pixels = anyrender::render_to_buffer::<anyrender_vello_cpu::VelloCpuImageRenderer, _>(
        |tile| {
            tile.fill(
                Fill::NonZero,
                fit,
                anyrender::Paint::Gradient(gradient),
                gradient_transform,
                &size.to_rect(),
            );
        },
        width,
        height,
    );
    Some(tile_image(pixels, width, height))
}

fn svg_tile_image(svg: &SvgImageData, size: Size) -> Option<peniko::ImageData> {
    let (width, height) = (tile_side(size.width)?, tile_side(size.height)?);
    let viewport =
        super::svg_viewport_affine(svg, f64::from(width).to_f32()?, f64::from(height).to_f32()?);
    let pixels = anyrender::render_to_buffer::<anyrender_vello_cpu::VelloCpuImageRenderer, _>(
        |tile| anyrender_svg::render_svg_tree(tile, &svg.tree, viewport),
        width,
        height,
    );
    Some(tile_image(pixels, width, height))
}

fn compute_space_count_and_stride(bg_size: f64, size: f64) -> (u32, f64) {
    let modulo = bg_size % size;
    let count = (((bg_size - modulo) / size) as u32).max(1);
    let stride = if count > 1 {
        modulo / (count - 1) as f64
    } else {
        0.0
    } + size;

    (count, stride)
}

/// Whether drawing the layer is guaranteed to paint nothing, making its clip
/// layer unnecessary
fn layer_paints_nothing(layer: &ImageLayerStyles) -> bool {
    match layer.stylo_image {
        GenericImage::None => true,
        GenericImage::Url(_) => layer.image_data.is_none(),
        _ => false,
    }
}

pub(super) fn canvas_background_source(dom: &BaseDocument) -> Option<NodeId> {
    let root = dom.try_root_element()?;
    if has_background(root) || !root.data.is_element_with_tag_name(&local_name!("html")) {
        return Some(root.id);
    }
    let body = root.children.iter().find_map(|id| {
        dom.get_node(*id).filter(|node| {
            node.data.is_element_with_tag_name(&local_name!("body"))
                && !matches!(node.taffy_display(), taffy::Display::None)
        })
    });
    Some(body.unwrap_or(root).id)
}

fn has_background(node: &Node) -> bool {
    node.primary_styles().is_some_and(|style| {
        let background = style.get_background();
        background.background_color != GenericColor::TRANSPARENT_BLACK
            || background
                .background_image
                .0
                .iter()
                .any(|image| !matches!(image, GenericImage::None))
    })
}

#[inline]
pub(super) fn get_cyclic<T>(values: &[T], layer_index: usize) -> &T {
    &values[layer_index % values.len()]
}

fn extend(offset: f64, length: f64) -> f64 {
    let extend_length = offset % length;
    if extend_length > 0.0 {
        length - extend_length
    } else {
        -extend_length
    }
}
