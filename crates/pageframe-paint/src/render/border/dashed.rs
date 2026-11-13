use anyrender::PaintScene;
use kurbo::{BezPath, Cap, Join, Point, Shape as _, Stroke, Vec2};
use peniko::{Color, Fill};

use super::{contour::dashed_contour, open_dash_gap};
use crate::{kurbo_css::Edge, render::ElementCx};

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
        let (centerline, dash, gap) = {
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
            (line, dash, gap)
        };

        scene.push_clip_layer(Fill::NonZero, self.transform, clip);
        scene.stroke(
            &dash_stroke(thickness, dash, gap),
            self.transform,
            color,
            None,
            &centerline,
        );
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
