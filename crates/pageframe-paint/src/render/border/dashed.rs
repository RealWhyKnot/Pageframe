use anyrender::PaintScene;
use kurbo::{BezPath, Cap, Join, Point, Rect, Shape as _, Stroke, Vec2};
use peniko::{Color, Fill};

use super::{contour::dashed_contour, open_dash_gap};
use crate::{kurbo_css::Edge, render::ElementCx};

const MAX_SNAPPED_DASHES: f64 = 4096.0;

pub(super) fn dashed_ratios(thickness: f64) -> (f64, f64) {
    if thickness >= 3.0 {
        (2.0, 1.0)
    } else {
        (3.0, 2.0)
    }
}

fn dash_stroke(thickness: f64, dash: f64, gap: f64) -> Stroke {
    let solid = Stroke::new(thickness).with_caps(Cap::Butt);
    if gap > 0.0 {
        solid.with_dashes(0.0, [dash, gap])
    } else {
        solid
    }
}

fn quarter(value: f64, phase: f64) -> f64 {
    ((value + phase) * 4.0).round() / 4.0 - phase
}

fn quarter_dashes(
    band: Rect,
    horizontal: bool,
    dash: f64,
    gap: f64,
    phase: Vec2,
) -> Option<BezPath> {
    let (from, to) = if horizontal {
        (band.x0, band.x1)
    } else {
        (band.y0, band.y1)
    };
    let period = dash + gap;
    if gap <= 0.0 || period <= 0.0 || (to - from) / period > MAX_SNAPPED_DASHES {
        return None;
    }
    let mut path = BezPath::new();
    let mut index = 0.0;
    while from + index * period < to {
        let start = from + index * period;
        let end = (start + dash).min(to);
        let rect = if horizontal {
            Rect::new(start, band.y0, end, band.y1)
        } else {
            Rect::new(band.x0, start, band.x1, end)
        };
        let (top, bottom) = (quarter(rect.y0, phase.y), quarter(rect.y1, phase.y));
        if rect.x1 > rect.x0 && bottom > top {
            path.extend(Rect::new(rect.x0, top, rect.x1, bottom).path_elements(0.0));
        }
        index += 1.0;
    }
    Some(path)
}

impl ElementCx<'_, '_> {
    fn dashed_contour(&self, thickness: f64) -> Option<BezPath> {
        let offset = if self.frame.pixel_phase.is_some() {
            self.transform.translation()
        } else {
            Vec2::ZERO
        };
        dashed_contour(&self.frame, thickness, offset)
    }

    pub(super) fn draw_dashed_border_edge(
        &self,
        scene: &mut impl PaintScene,
        edge: Edge,
        color: Color,
        clip: &BezPath,
    ) {
        let thickness = self.edge_width(edge);
        if thickness <= 0.0 {
            return;
        }

        if self.frame.has_border_radius() {
            let widths = self.frame.border_width;
            let adjacent = match edge {
                Edge::Top | Edge::Bottom => widths.x0.max(widths.x1),
                Edge::Left | Edge::Right => widths.y0.max(widths.y1),
            };
            let Some(centerline) = self.dashed_contour(thickness) else {
                return;
            };
            scene.push_clip_layer(Fill::NonZero, self.transform, clip);
            scene.stroke(
                &Stroke::new(2.2 * thickness.max(adjacent))
                    .with_caps(Cap::Butt)
                    .with_join(Join::Miter),
                self.transform,
                color,
                None,
                &centerline,
            );
            scene.pop_layer();
            return;
        }
        let (dash_ratio, gap_ratio) = dashed_ratios(thickness);
        let (centerline, dash, gap, band) = {
            let bb = self.frame.border_box;
            let half = thickness / 2.0;
            let (start, end, length) = match edge {
                Edge::Top => (
                    Point::new(bb.x0, bb.y0 + half),
                    Point::new(bb.x1, bb.y0 + half),
                    bb.width(),
                ),
                Edge::Bottom => (
                    Point::new(bb.x0, bb.y1 - half),
                    Point::new(bb.x1, bb.y1 - half),
                    bb.width(),
                ),
                Edge::Left => (
                    Point::new(bb.x0 + half, bb.y0),
                    Point::new(bb.x0 + half, bb.y1),
                    bb.height(),
                ),
                Edge::Right => (
                    Point::new(bb.x1 - half, bb.y0),
                    Point::new(bb.x1 - half, bb.y1),
                    bb.height(),
                ),
            };
            if length <= 0.0 {
                return;
            }
            let dash = dash_ratio * thickness;
            let gap = gap_ratio * thickness;
            let whole = length.round();
            let (dash, gap) = if whole <= 2.0 * dash {
                (dash, 0.0)
            } else if whole <= 2.0 * dash + gap {
                let stretch = whole / (2.0 * dash + gap);
                (dash * stretch, gap * stretch)
            } else {
                (dash, open_dash_gap(whole, dash, gap))
            };
            let mut line = BezPath::new();
            line.move_to(start);
            line.line_to(end);
            let band = match edge {
                Edge::Top => Rect::new(bb.x0, bb.y0, bb.x1, bb.y0 + thickness),
                Edge::Bottom => Rect::new(bb.x0, bb.y1 - thickness, bb.x1, bb.y1),
                Edge::Left => Rect::new(bb.x0, bb.y0, bb.x0 + thickness, bb.y1),
                Edge::Right => Rect::new(bb.x1 - thickness, bb.y0, bb.x1, bb.y1),
            };
            (line, dash, gap, band)
        };

        scene.push_clip_layer(Fill::NonZero, self.transform, clip);
        let horizontal = matches!(edge, Edge::Top | Edge::Bottom);
        let snapped = self.frame.pixel_phase.and_then(|_| {
            quarter_dashes(band, horizontal, dash, gap, self.transform.translation())
        });
        match snapped {
            Some(dashes) => scene.fill(Fill::NonZero, self.transform, color, None, &dashes),
            None => scene.stroke(
                &dash_stroke(thickness, dash, gap),
                self.transform,
                color,
                None,
                &centerline,
            ),
        }
        scene.pop_layer();
    }

    pub(super) fn draw_uniform_round_dashed_border(
        &self,
        scene: &mut impl PaintScene,
        color: Color,
        outer_clipped: bool,
    ) {
        let thickness = self.frame.border_width.x0;
        let Some(centerline) = self.dashed_contour(thickness) else {
            return;
        };
        let mut ring = if outer_clipped {
            self.frame.border_box.to_path(0.1)
        } else {
            self.frame.border_box_path()
        };
        ring.extend(self.frame.padding_box_path());
        scene.push_clip_layer(Fill::EvenOdd, self.transform, &ring);
        scene.stroke(
            &Stroke::new(2.2 * thickness)
                .with_caps(Cap::Butt)
                .with_join(Join::Miter),
            self.transform,
            color,
            None,
            &centerline,
        );
        scene.pop_layer();
    }
}
