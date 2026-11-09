use super::ElementCx;
use super::pixels::{Pixels, Span, fill_pixels, snap_offset};
use crate::color::Color;
use anyrender::PaintScene;
use kurbo::{Point, Rect, Vec2};

const DARK: Color = Color::from_rgba8(0, 0, 0, 153);
const LIGHT: Color = Color::from_rgba8(255, 255, 255, 153);

impl ElementCx<'_, '_> {
    pub(super) fn draw_resizer(&self, scene: &mut impl PaintScene) {
        let Some(corner) = self.node.resizer_corner(self.scale) else {
            return;
        };
        let snap = snap_offset(self.transform, corner.origin());
        let corner = corner + snap;
        let on_left = self.node.resizer_on_left();
        let (dark, light) = grip_pixels(corner, self.scale, on_left);
        fill_pixels(scene, self.transform, &dark, DARK);
        fill_pixels(scene, self.transform, &light, LIGHT);
    }
}

fn grip_pixels(corner: Rect, scale: f64, on_left: bool) -> (Pixels, Pixels) {
    let width = corner.width().round();
    let height = corner.height().round();
    let default_spacing = (2.0 * scale).round();
    let default_size = (7.0 * scale).round();
    let spacing_x = if width > 0.0 {
        default_spacing.min((width / 4.0).trunc()).max(1.0)
    } else {
        0.0
    };
    let spacing_y = if height > 0.0 {
        default_spacing.min((height / 4.0).trunc()).max(1.0)
    } else {
        0.0
    };
    let size_x = default_size.min(width - spacing_x).max(1.0);
    let size_y = default_size.min(height - spacing_y).max(1.0);
    let spacing = spacing_x.min(spacing_y);
    let large = size_x.min(size_y);
    let small = (large / 2.0).trunc().max(1.0);

    let x = |inset: f64| {
        if on_left {
            corner.x0 + inset
        } else {
            corner.x1 - inset
        }
    };
    let bottom = corner.y1;
    let points = [
        Point::new(x(spacing), bottom - (spacing + large)),
        Point::new(x(spacing + large), bottom - spacing),
        Point::new(x(spacing), bottom - (spacing + small)),
        Point::new(x(spacing + small), bottom - spacing),
    ];

    let stroke = scale.ceil();
    let v_offset = scale.ceil();
    let h_offset = if on_left { -v_offset } else { v_offset };
    let mut dark = Pixels::new();
    let mut light = Pixels::new();
    for pair in [[0, 1], [2, 3]] {
        let (start, end) = (points[pair[0]], points[pair[1]]);
        segment_pixels(start, end, stroke, &mut dark);
        segment_pixels(
            start + Vec2::new(0.0, v_offset),
            end + Vec2::new(h_offset, 0.0),
            stroke,
            &mut light,
        );
    }
    (dark, light)
}

fn segment_pixels(start: Point, end: Point, width: f64, out: &mut Pixels) {
    let direction = end - start;
    let length = direction.hypot();
    if length == 0.0 {
        return;
    }
    let normal = Vec2::new(-direction.y, direction.x) * (width / 2.0 / length);
    let quad = [start + normal, end + normal, end - normal, start - normal];
    let top = quad.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let bottom = quad.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
    let mut row = top.floor() - 1.0;
    while row < bottom.ceil() + 1.0 {
        let center = row + 0.5;
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut crossings = 0;
        for i in 0..4 {
            let (a, b) = (quad[i], quad[(i + 1) % 4]);
            if a.y == b.y {
                continue;
            }
            let (low, high) = if a.y < b.y { (a.y, b.y) } else { (b.y, a.y) };
            if (low + 0.5).floor() <= row && row < (high + 0.5).floor() {
                let x = a.x + (center - a.y) * (b.x - a.x) / (b.y - a.y);
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                crossings += 1;
            }
        }
        let start = (min_x + 0.5).floor();
        let end = (max_x + 0.5).floor();
        if crossings >= 2 && end > start {
            out.push(Span { row, start, end });
        }
        row += 1.0;
    }
}
