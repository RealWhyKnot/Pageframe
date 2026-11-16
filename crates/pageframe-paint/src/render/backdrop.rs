use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use anyrender::{Filter, Glyph, NormalizedCoord, PaintRef, PaintScene, RenderContext, Scene};
use kurbo::{Affine, Rect, Shape, Stroke};
use peniko::{BlendMode, Color, Fill, FontData, StyleRef};
use style::properties::ComputedValues;
use style::values::computed::image::Image;

use super::{ElementCx, PageframeDomPainter};
use crate::blend::convert_blend_mode;
use crate::filters::{filtered_backdrop, pixel_filters};
use pageframe_dom::{CopyStyleGetters as _, Node};

const MAX_DEPTH: usize = 8;

#[derive(Clone)]
pub(super) struct Backdrop {
    image: peniko::ImageBrush,
    area: Rect,
}

struct Cutoff<'c> {
    scene: Scene,
    reached: &'c Cell<bool>,
    open: usize,
}

impl<'c> Cutoff<'c> {
    fn new(reached: &'c Cell<bool>) -> Self {
        Self {
            scene: Scene::new(),
            reached,
            open: 0,
        }
    }

    fn live(&self) -> bool {
        !self.reached.get()
    }

    fn finish(mut self) -> Scene {
        for _ in 0..self.open {
            self.scene.pop_layer();
        }
        self.scene
    }
}

impl RenderContext for Cutoff<'_> {}

impl PaintScene for Cutoff<'_> {
    fn reset(&mut self) {
        self.scene.reset();
        self.open = 0;
    }

    fn push_layer(
        &mut self,
        fill: Fill,
        blend: impl Into<BlendMode>,
        alpha: f32,
        transform: Affine,
        clip: &impl Shape,
        filter: Option<Arc<Filter>>,
        backdrop_filter: Option<Arc<Filter>>,
    ) {
        if self.live() {
            self.open += 1;
            self.scene
                .push_layer(fill, blend, alpha, transform, clip, filter, backdrop_filter);
        }
    }

    fn push_clip_layer(&mut self, fill: Fill, transform: Affine, clip: &impl Shape) {
        if self.live() {
            self.open += 1;
            self.scene.push_clip_layer(fill, transform, clip);
        }
    }

    fn pop_layer(&mut self) {
        if self.live() && self.open > 0 {
            self.open -= 1;
            self.scene.pop_layer();
        }
    }

    fn stroke<'a>(
        &mut self,
        style: &Stroke,
        transform: Affine,
        brush: impl Into<PaintRef<'a>>,
        brush_transform: Option<Affine>,
        shape: &impl Shape,
    ) {
        if self.live() {
            self.scene
                .stroke(style, transform, brush, brush_transform, shape);
        }
    }

    fn fill<'a>(
        &mut self,
        style: Fill,
        transform: Affine,
        brush: impl Into<PaintRef<'a>>,
        brush_transform: Option<Affine>,
        shape: &impl Shape,
    ) {
        if self.live() {
            self.scene
                .fill(style, transform, brush, brush_transform, shape);
        }
    }

    fn draw_glyphs<'a, 's: 'a>(
        &'s mut self,
        font: &'a FontData,
        font_size: f32,
        hint: bool,
        normalized_coords: &'a [NormalizedCoord],
        embolden: kurbo::Vec2,
        style: impl Into<StyleRef<'a>>,
        brush: impl Into<PaintRef<'a>>,
        brush_alpha: f32,
        transform: Affine,
        glyph_transform: Option<Affine>,
        glyphs: impl Iterator<Item = Glyph> + Clone,
    ) {
        if self.live() {
            self.scene.draw_glyphs(
                font,
                font_size,
                hint,
                normalized_coords,
                embolden,
                style,
                brush,
                brush_alpha,
                transform,
                glyph_transform,
                glyphs,
            );
        }
    }

    fn draw_box_shadow(
        &mut self,
        transform: Affine,
        rect: Rect,
        brush: Color,
        radius: f64,
        std_dev: f64,
    ) {
        if self.live() {
            self.scene
                .draw_box_shadow(transform, rect, brush, radius, std_dev);
        }
    }
}

impl PageframeDomPainter<'_, '_> {
    pub(super) fn backdrop(
        &self,
        node: &Node,
        styles: &ComputedValues,
        cx: &ElementCx<'_, '_>,
        projected: bool,
    ) -> Option<Backdrop> {
        let backdrop_filter = &styles.get_effects().backdrop_filter.0;
        if backdrop_filter.is_empty()
            || self.backdrop_depth >= MAX_DEPTH
            || self.has_backdrop_root_ancestor(node)
        {
            return None;
        }
        if let Some(cached) = self.backdrops.borrow().get(&node.id) {
            return cached.clone();
        }
        let backdrop = self.paint_backdrop(node, styles, cx, projected);
        self.backdrops
            .borrow_mut()
            .insert(node.id, backdrop.clone());
        backdrop
    }

    fn paint_backdrop(
        &self,
        node: &Node,
        styles: &ComputedValues,
        cx: &ElementCx<'_, '_>,
        projected: bool,
    ) -> Option<Backdrop> {
        let backdrop_filter = &styles.get_effects().backdrop_filter.0;
        let filters = pixel_filters(backdrop_filter, &styles.clone_color(), self.scale);
        if filters.is_empty() {
            return None;
        }
        let area = if projected {
            self.viewport_rect()
        } else {
            cx.transform
                .transform_rect_bbox(cx.frame.border_box)
                .intersect(self.viewport_rect())
        };
        if area.width() <= 0.0 || area.height() <= 0.0 {
            return None;
        }
        let mut painter = PageframeDomPainter::new(
            self.dom,
            self.scale,
            self.width,
            self.height,
            self.initial_x,
            self.initial_y,
            self.custom_widget_scenes,
        );
        painter.backdrop_stop = Some(node.id);
        painter.backdrop_depth = self.backdrop_depth + 1;
        painter.backdrops = Rc::clone(&self.backdrops);
        let mut cutoff = Cutoff::new(&painter.backdrop_reached);
        painter.paint_scene(&mut cutoff);
        let recording = cutoff.finish();
        let (image, area) = filtered_backdrop(recording, area, &filters)?;
        Some(Backdrop { image, area })
    }

    fn has_backdrop_root_ancestor(&self, node: &Node) -> bool {
        let tree = self.dom.as_ref().tree();
        let mut parent = node.parent;
        while let Some(id) = parent {
            let Some(ancestor) = tree.get(id) else {
                return false;
            };
            if let Some(style) = ancestor.primary_styles() {
                let effects = style.get_effects();
                let masked = style
                    .get_svg()
                    .mask_image
                    .0
                    .iter()
                    .any(|image| !matches!(image, Image::None));
                if effects.opacity < 1.0
                    || !effects.filter.0.is_empty()
                    || !effects.backdrop_filter.0.is_empty()
                    || convert_blend_mode(effects.mix_blend_mode) != BlendMode::default()
                    || masked
                    || self.dom.clip_path_of(ancestor).is_some()
                {
                    return true;
                }
            }
            parent = ancestor.parent;
        }
        false
    }

    pub(super) fn draw_backdrop(
        &self,
        scene: &mut impl PaintScene,
        cx: &ElementCx<'_, '_>,
        backdrop: Option<&Backdrop>,
    ) {
        let Some(backdrop) = backdrop else {
            return;
        };
        self.layer_manager.maybe_with_clip_layer(
            scene,
            true,
            Fill::NonZero,
            cx.transform,
            &cx.frame.border_box_path(),
            |scene| {
                scene.draw_image(
                    backdrop.image.as_ref(),
                    Affine::translate((backdrop.area.x0, backdrop.area.y0)),
                );
            },
        );
    }
}
