//! Painting elements whose transform has perspective.
//!
//! The scene only takes affine transforms, so the element is cut into triangles that
//! are small enough for an affine map to stay within a fraction of a pixel of the
//! projection. Each triangle is painted with the affine map that matches the
//! projection exactly at its three corners, clipped to where those corners land.
//! Neighbouring triangles agree along their shared edges, so the content lines up.

use super::BlitzDomPainter;
use anyrender::PaintScene;
use blitz_dom::{Node, NodeId};
use euclid::default::{Point2D, Rect as EuclidRect, Size2D, Transform3D};
use kurbo::{Affine, BezPath, Point, Rect, Shape, Vec2};
use peniko::{BlendMode, Color, Compose, Fill, Mix};
use style::values::computed::CSSPixelLength;

/// Points this close to the viewer's plane (in homogeneous w) are clipped away,
/// so that nothing is projected to infinity.
const MIN_W: f64 = 1.0 / 1024.0;
/// How far, in device pixels, an affine piece may stray from the projection.
const MAX_ERROR: f64 = 0.25;
/// The most cells an element is split into. Each cell takes up to four layers.
const MAX_CELLS: usize = 64;
/// Triangles are added together rather than drawn over each other: the coverage
/// of two triangles along their shared edge sums to one, so the edge doesn't show.
const ADD: BlendMode = BlendMode {
    mix: Mix::Normal,
    compose: Compose::Plus,
};
const ERASE: BlendMode = BlendMode {
    mix: Mix::Normal,
    compose: Compose::DestOut,
};

pub(super) struct Projection {
    /// The element's transform in CSS pixels.
    matrix: Transform3D<f64>,
    /// Device pixels per CSS pixel.
    scale: f64,
}

impl Projection {
    fn w(&self, point: Point) -> f64 {
        let m = &self.matrix;
        (point.x * m.m14 + point.y * m.m24) / self.scale + m.m44
    }

    /// Where a point in the element's local device pixels is drawn, or `None` if it
    /// is behind the viewer.
    fn project(&self, point: Point) -> Option<Point> {
        let m = &self.matrix;
        let w = self.w(point);
        if !in_front(w) {
            return None;
        }
        let (x, y) = (point.x / self.scale, point.y / self.scale);
        Some(Point::new(
            (x * m.m11 + y * m.m21 + m.m41) / w * self.scale,
            (x * m.m12 + y * m.m22 + m.m42) / w * self.scale,
        ))
    }

    /// How far an affine piece covering `cell` would stray from the projection, and
    /// whether halving it across x (rather than y) would help most. Projective maps
    /// bend quadratically, so the error along a line is about twice the distance
    /// between its projected midpoint and the midpoint of its projected ends.
    ///
    /// Cells that land wholly outside `visible` don't need to be accurate. Cells that
    /// reach behind the viewer are split along w until they are a pixel thin, since
    /// their part close to the viewer's plane is projected far off.
    fn assess(&self, cell: Rect, visible: Rect) -> Piece {
        let m = &self.matrix;
        let split_x_for_w = (m.m14 * cell.width()).abs() >= (m.m24 * cell.height()).abs();
        let footprint = self
            .clip_to_front(&corners(cell))
            .into_iter()
            .filter_map(|point| self.project(point))
            .fold(None, |bounds: Option<Rect>, point| {
                Some(bounds.map_or(Rect::from_points(point, point), |bounds| {
                    bounds.union_pt(point)
                }))
            });
        if footprint.is_none_or(|footprint| footprint.intersect(visible).area() <= 0.0) {
            return Piece {
                cell,
                error: 0.0,
                split_x: split_x_for_w,
            };
        }

        let [a, b, c, d] = corners(cell);
        let bend = |p: Point, q: Point| -> Option<f64> {
            let (start, end) = (self.project(p)?, self.project(q)?);
            let middle = self.project(p.midpoint(q))?;
            Some(2.0 * (middle - start.midpoint(end)).hypot())
        };
        let bends = (|| {
            let across_x = bend(a, b)?.max(bend(d, c)?);
            let across_y = bend(a, d)?.max(bend(b, c)?);
            let diagonal = bend(a, c)?.max(bend(b, d)?);
            Some((across_x, across_y, diagonal))
        })();
        match bends {
            Some((across_x, across_y, diagonal)) => Piece {
                cell,
                error: across_x.max(across_y).max(diagonal),
                split_x: across_x >= across_y,
            },
            None => {
                let thickness = if split_x_for_w {
                    cell.width()
                } else {
                    cell.height()
                };
                Piece {
                    cell,
                    error: if thickness > 1.0 { f64::INFINITY } else { 0.0 },
                    split_x: split_x_for_w,
                }
            }
        }
    }

    fn is_behind(&self, cell: Rect) -> bool {
        // w is affine, so a cell whose corners are all behind the viewer is wholly behind.
        corners(cell)
            .iter()
            .all(|&corner| !in_front(self.w(corner)))
    }

    /// Split `domain` into cells, always halving the cell that strays furthest,
    /// until every cell is close enough or the budget is spent.
    fn subdivide(&self, domain: Rect, visible: Rect) -> Vec<Rect> {
        let mut pieces = Vec::new();
        if !self.is_behind(domain) {
            pieces.push(self.assess(domain, visible));
        }
        while pieces.len() < MAX_CELLS {
            let Some((worst, _)) = pieces
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.error.total_cmp(&b.error))
                .filter(|(_, piece)| piece.error > MAX_ERROR)
            else {
                break;
            };
            let Piece { cell, split_x, .. } = pieces.swap_remove(worst);
            let centre = cell.center();
            let halves = if split_x {
                [
                    Rect::new(cell.x0, cell.y0, centre.x, cell.y1),
                    Rect::new(centre.x, cell.y0, cell.x1, cell.y1),
                ]
            } else {
                [
                    Rect::new(cell.x0, cell.y0, cell.x1, centre.y),
                    Rect::new(cell.x0, centre.y, cell.x1, cell.y1),
                ]
            };
            for half in halves {
                if !self.is_behind(half) {
                    pieces.push(self.assess(half, visible));
                }
            }
        }
        pieces.into_iter().map(|piece| piece.cell).collect()
    }

    /// The part of a convex polygon that is in front of the viewer
    /// (Sutherland-Hodgman against the plane w = MIN_W).
    fn clip_to_front(&self, polygon: &[Point]) -> Vec<Point> {
        let mut clipped = Vec::with_capacity(polygon.len() + 1);
        for (index, &current) in polygon.iter().enumerate() {
            let next = polygon[(index + 1) % polygon.len()];
            let (w_current, w_next) = (self.w(current), self.w(next));
            if in_front(w_current) {
                clipped.push(current);
            }
            if in_front(w_current) != in_front(w_next) {
                let t = (MIN_W - w_current) / (w_next - w_current);
                clipped.push(current.lerp(next, t));
            }
        }
        clipped
    }
}

struct Piece {
    cell: Rect,
    error: f64,
    split_x: bool,
}

fn corners(rect: Rect) -> [Point; 4] {
    [
        Point::new(rect.x0, rect.y0),
        Point::new(rect.x1, rect.y0),
        Point::new(rect.x1, rect.y1),
        Point::new(rect.x0, rect.y1),
    ]
}

/// The affine map that takes each point of `from` to the matching point of `to`.
fn affine_through(from: [Point; 3], to: [Point; 3]) -> Option<Affine> {
    let frame = |[a, b, c]: [Point; 3]| {
        let (u, v) = (b - a, c - a);
        Affine::new([u.x, u.y, v.x, v.y, a.x, a.y])
    };
    let source = frame(from);
    if source.determinant().abs() < 1e-9 {
        return None;
    }
    let affine = frame(to) * source.inverse();
    affine.is_finite().then_some(affine)
}

fn triangle_path(triangle: [Point; 3]) -> BezPath {
    let mut path = BezPath::new();
    path.move_to(triangle[0]);
    path.line_to(triangle[1]);
    path.line_to(triangle[2]);
    path.close_path();
    path
}

impl BlitzDomPainter<'_, '_> {
    /// The element's transform, if it has perspective that `Node::transform` can only
    /// approximate.
    pub(super) fn projection(&self, node: &Node) -> Option<Projection> {
        node.transform().as_ref()?;
        let styles = node.primary_styles()?;
        let size = node.final_layout().size;
        let reference_box = EuclidRect::new(
            Point2D::new(CSSPixelLength::new(0.0), CSSPixelLength::new(0.0)),
            Size2D::new(
                CSSPixelLength::new(size.width),
                CSSPixelLength::new(size.height),
            ),
        );
        let matrix = blitz_dom::resolve_projective_transform(styles.get_box(), reference_box)?;
        Some(Projection {
            matrix,
            scale: self.scale,
        })
    }

    pub(super) fn render_projected_element(
        &self,
        scene: &mut impl PaintScene,
        node_id: NodeId,
        parent_style_transform: Affine,
        clip_rect: Rect,
        projection: &Projection,
    ) {
        let node = &self.dom.as_ref().tree()[node_id];
        let size = node.final_layout().size;
        let border_box = Rect::new(
            0.0,
            0.0,
            size.width as f64 * self.scale,
            size.height as f64 * self.scale,
        );
        let position = self.node_position(node, parent_style_transform);
        let screen = Affine::translate(Vec2::new(-self.initial_x, -self.initial_y)) * position;

        // Grown so that the triangles' outer edges don't cut into the element's own
        // anti-aliased edges.
        let domain = node
            .scrollable_overflow()
            .union(border_box)
            .inflate(2.0, 2.0);
        if screen.determinant().abs() < 1e-12 {
            return;
        }
        let visible = screen.inverse().transform_rect_bbox(clip_rect);
        let cells = projection.subdivide(domain, visible);

        // The triangles are added together in a group of their own, so that
        // nothing underneath the element is added in too.
        let group_pushed = self.layer_manager.maybe_push_layer(
            scene,
            true,
            BlendMode::default(),
            1.0 - f32::EPSILON,
            Affine::IDENTITY,
            &Rect::new(
                self.initial_x,
                self.initial_y,
                self.initial_x + self.width as f64,
                self.initial_y + self.height as f64,
            ),
            None,
            None,
        );
        if !group_pushed {
            return;
        }

        for cell in cells {
            let [top_left, top_right, bottom_right, bottom_left] = corners(cell);
            for half in [
                [top_left, top_right, bottom_right],
                [top_left, bottom_right, bottom_left],
            ] {
                let polygon = projection.clip_to_front(&half);
                for index in 1..polygon.len().saturating_sub(1) {
                    let triangle = [polygon[0], polygon[index], polygon[index + 1]];
                    let Some(projected) = triangle
                        .iter()
                        .map(|&point| projection.project(point))
                        .collect::<Option<Vec<_>>>()
                    else {
                        continue;
                    };
                    let projected = [projected[0], projected[1], projected[2]];
                    let Some(own_transform) = affine_through(triangle, projected) else {
                        continue;
                    };
                    let triangle_shape = triangle_path(projected);
                    let bounds = triangle_shape.bounding_box().inflate(1.0, 1.0);
                    let visible = clip_rect.intersect(screen.transform_rect_bbox(bounds));
                    if visible.width() <= 0.0 || visible.height() <= 0.0 {
                        continue;
                    }
                    if !self.layer_manager.has_room_for(2) {
                        continue;
                    }
                    let pushed = self
                        .layer_manager
                        .maybe_push_layer(scene, true, ADD, 1.0, position, &bounds, None, None);
                    if !pushed {
                        continue;
                    }
                    self.render_element_with(
                        scene,
                        node_id,
                        parent_style_transform,
                        visible,
                        Some(own_transform),
                    );
                    // Cut the piece down to the triangle by its coverage, rather than with a
                    // clip, which would be applied after adding it and saturate. Everything
                    // around the triangle is erased, reaching past the layer's own clip so
                    // that none of the piece survives along the clip's edge.
                    let erase_bounds = bounds.inflate(1.0, 1.0);
                    let mut around_triangle = erase_bounds.to_path(0.1);
                    around_triangle.extend(triangle_shape.iter());
                    let mask_pushed = self.layer_manager.maybe_push_layer(
                        scene,
                        true,
                        ERASE,
                        1.0,
                        position,
                        &erase_bounds,
                        None,
                        None,
                    );
                    if mask_pushed {
                        scene.fill(
                            Fill::EvenOdd,
                            position,
                            Color::BLACK,
                            None,
                            &around_triangle,
                        );
                    }
                    self.layer_manager.maybe_pop_layer(scene, mask_pushed);
                    self.layer_manager.maybe_pop_layer(scene, true);
                }
            }
        }
        self.layer_manager.maybe_pop_layer(scene, true);
    }
}

/// Whether a point with this w is in front of the viewer. False for NaN.
fn in_front(w: f64) -> bool {
    w >= MIN_W
}
