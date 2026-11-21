mod color;
mod draw;
mod font;
mod image;
mod paint;
mod path;
mod raster;
mod shadow;
mod text;

use std::rc::Rc;
use std::sync::Arc;

pub use kurbo::{Affine, Cap, Join, Point, Rect, Vec2};
use markup5ever::local_name;
use parley::FontContext;

pub use color::Rgba;
pub use font::{FontSlant, FontSpec};
pub use image::ImageSource;
pub use paint::{CompositeOp, Gradient, GradientKind, Paint};
pub use path::Radii;
pub use raster::FillRule;
pub use text::{TextAlign, TextBaseline, TextMetrics};

use crate::BaseDocument;
use crate::layout::damage::ALL_DAMAGE;
use crate::node::{ImageData, RasterImageData, SpecialElementData};
use pageframe_traits::node_id::NodeId;

pub const MAX_AREA: u64 = 268_435_456;
pub const MAX_SIDE: u32 = 32_767;
const MAX_SAVES: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SmoothingQuality {
    #[default]
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug)]
pub struct DrawState {
    pub transform: Affine,
    pub fill: Paint,
    pub stroke: Paint,
    pub global_alpha: f64,
    pub composite: CompositeOp,
    pub composite_name: &'static str,
    pub line_width: f64,
    pub line_cap: Cap,
    pub line_join: Join,
    pub miter_limit: f64,
    pub dash: Rc<[f64]>,
    pub dash_offset: f64,
    pub font: FontSpec,
    pub text_align: TextAlign,
    pub text_baseline: TextBaseline,
    pub shadow_color: Rgba,
    pub shadow_blur: f64,
    pub shadow_offset_x: f64,
    pub shadow_offset_y: f64,
    pub image_smoothing: bool,
    pub smoothing_quality: SmoothingQuality,
    clip: Option<Rc<Vec<f32>>>,
}

impl Default for DrawState {
    fn default() -> Self {
        Self {
            transform: Affine::IDENTITY,
            fill: Paint::default(),
            stroke: Paint::default(),
            global_alpha: 1.0,
            composite: CompositeOp::SourceOver,
            composite_name: "source-over",
            line_width: 1.0,
            line_cap: Cap::Butt,
            line_join: Join::Miter,
            miter_limit: 10.0,
            dash: Rc::default(),
            dash_offset: 0.0,
            font: FontSpec::default(),
            text_align: TextAlign::Start,
            text_baseline: TextBaseline::Alphabetic,
            shadow_color: Rgba::TRANSPARENT,
            shadow_blur: 0.0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 0.0,
            image_smoothing: true,
            smoothing_quality: SmoothingQuality::Low,
            clip: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Canvas2D {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    state: DrawState,
    stack: Vec<DrawState>,
    overflow: usize,
    path: path::Path,
    dirty: bool,
}

impl Canvas2D {
    pub fn new(width: u32, height: u32) -> Self {
        let mut canvas = Self {
            width: 0,
            height: 0,
            pixels: Vec::new(),
            state: DrawState::default(),
            stack: Vec::new(),
            overflow: 0,
            path: path::Path::default(),
            dirty: true,
        };
        canvas.resize(width, height);
        canvas
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let usable = width <= MAX_SIDE
            && height <= MAX_SIDE
            && u64::from(width) * u64::from(height) <= MAX_AREA;
        let (mut width, mut height) = if usable { (width, height) } else { (0, 0) };
        self.pixels = Vec::new();
        if self
            .pixels
            .try_reserve_exact(width as usize * height as usize * 4)
            .is_err()
        {
            (width, height) = (0, 0);
        }
        self.width = width;
        self.height = height;
        self.pixels.resize(width as usize * height as usize * 4, 0);
        self.state = DrawState::default();
        self.stack.clear();
        self.overflow = 0;
        self.path = path::Path::default();
        self.dirty = true;
    }

    pub fn reset(&mut self) {
        self.resize(self.width, self.height);
    }

    pub fn state(&self) -> &DrawState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut DrawState {
        &mut self.state
    }

    pub fn save(&mut self) {
        if self.stack.len() < MAX_SAVES {
            self.stack.push(self.state.clone());
        } else {
            self.overflow = self.overflow.saturating_add(1);
        }
    }

    pub fn restore(&mut self) {
        if self.overflow > 0 {
            self.overflow -= 1;
        } else if let Some(state) = self.stack.pop() {
            self.state = state;
        }
    }

    pub fn set_composite(&mut self, name: &str) -> bool {
        let Some(name) = CompositeOp::NAMES.iter().find(|n| **n == name) else {
            return false;
        };
        self.state.composite_name = name;
        self.state.composite = CompositeOp::from_name(name);
        true
    }

    pub fn set_transform(&mut self, transform: Affine) {
        if transform.as_coeffs().iter().all(|c| c.is_finite()) {
            self.state.transform = transform;
        }
    }

    pub fn transform(&mut self, transform: Affine) {
        self.set_transform(self.state.transform * transform);
    }

    pub fn premultiplied_pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn to_straight_rgba(&self) -> Vec<u8> {
        let mut out = self.pixels.clone();
        for px in out.as_chunks_mut::<4>().0 {
            let a = px[3];
            if a != 0 && a != 255 {
                for c in &mut px[..3] {
                    *c = ((u32::from(*c) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8;
                }
            }
        }
        out
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    fn touch(&mut self) {
        self.dirty = true;
    }

    pub fn measure_text(&self, fonts: &mut FontContext, text: &str) -> TextMetrics {
        let shaped = text::shape(fonts, &self.state.font, &text::prepare(text));
        text::measure(&shaped, self.state.text_align, self.state.text_baseline)
    }
}

pub fn with_fonts<R>(doc: &BaseDocument, f: impl FnOnce(&mut FontContext) -> R) -> Option<R> {
    let mut fonts = doc.font_ctx.lock().ok()?;
    Some(f(&mut fonts))
}

pub fn present(doc: &mut BaseDocument, node_id: NodeId, canvas: &Canvas2D) {
    let Some(node) = doc.get_node_mut(node_id) else {
        return;
    };
    let Some(element) = node.element_data_mut() else {
        return;
    };
    if element.name.local != local_name!("canvas") || element.attr(local_name!("src")).is_some() {
        return;
    }
    let (width, height) = (canvas.width(), canvas.height());
    let same_size = element
        .raster_image_data()
        .is_some_and(|data| data.width == width && data.height == height);
    element.special_data = if width == 0 || height == 0 {
        SpecialElementData::None
    } else {
        let data = Arc::new(canvas.to_straight_rgba());
        SpecialElementData::Image(Box::new(ImageData::Raster(RasterImageData::new(
            width, height, data,
        ))))
    };
    if !same_size {
        node.clear_layout_cache();
        node.insert_damage(ALL_DAMAGE);
    }
}
