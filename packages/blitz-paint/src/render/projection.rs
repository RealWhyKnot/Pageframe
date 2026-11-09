use super::BlitzDomPainter;
use anyrender::{PaintScene, Scene};
use blitz_dom::{Node, NodeId};
use euclid::default::{Point2D, Rect as EuclidRect, Size2D, Transform3D};
use kurbo::{Affine, BezPath, Point, Rect, Shape, Vec2};
use peniko::{BlendMode, Color, Compose, Fill, Mix};
use style::values::computed::CSSPixelLength;

const MIN_W: f64 = 1.0 / 1024.0;
const MAX_ERROR: f64 = 0.25;
const MAX_CELLS: usize = 64;
const LAYERS_KEPT: u32 = 256;
const ADD: BlendMode = BlendMode {
    mix: Mix::Normal,
    compose: Compose::Plus,
};
const ERASE: BlendMode = BlendMode {
    mix: Mix::Normal,
    compose: Compose::DestOut,
};

pub(super) struct Projection {
    matrix: Transform3D<f64>,
    scale: f64,
}

impl Projection {
    fn w(&self, point: Point) -> f64 {
        let m = &self.matrix;
        (point.x * m.m14 + point.y * m.m24) / self.scale + m.m44
    }

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
        corners(cell)
            .iter()
            .all(|&corner| !in_front(self.w(corner)))
    }

    fn subdivide(&self, domain: Rect, visible: Rect, max_cells: usize) -> Vec<Rect> {
        let mut pieces = Vec::new();
        if !self.is_behind(domain) {
            pieces.push(self.assess(domain, visible));
        }
        while pieces.len() < max_cells {
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

        let domain = node
            .scrollable_overflow()
            .union(border_box)
            .inflate(2.0, 2.0);
        if screen.determinant().abs() < 1e-12 {
            return;
        }
        let visible = screen.inverse().transform_rect_bbox(clip_rect);
        let left = self.layer_manager.layers_left();
        let mut max_cells = if left > LAYERS_KEPT { MAX_CELLS } else { 0 };
        while max_cells > 0 {
            let cells = projection.subdivide(domain, visible, max_cells);
            let cell_count = cells.len();
            let mut recorded = Scene::new();
            let used = self.layer_manager.count_layers(|| {
                self.render_triangles(
                    &mut recorded,
                    node_id,
                    parent_style_transform,
                    clip_rect,
                    projection,
                    cells,
                );
            });
            if used + LAYERS_KEPT <= left {
                self.layer_manager.take(used);
                scene.append_scene(recorded, Affine::IDENTITY);
                return;
            }
            max_cells = cell_count / 2;
        }
        self.render_element_with(scene, node_id, parent_style_transform, clip_rect, None);
    }

    fn render_triangles(
        &self,
        scene: &mut Scene,
        node_id: NodeId,
        parent_style_transform: Affine,
        clip_rect: Rect,
        projection: &Projection,
        cells: Vec<Rect>,
    ) {
        let node = &self.dom.as_ref().tree()[node_id];
        let position = self.node_position(node, parent_style_transform);
        let screen = Affine::translate(Vec2::new(-self.initial_x, -self.initial_y)) * position;
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
                    if self.layer_manager.layers_left() < 2 {
                        continue;
                    }
                    let pushed = self
                        .layer_manager
                        .maybe_push_layer(scene, true, ADD, 1.0, position, &bounds, None, None);
                    if !pushed {
                        continue;
                    }
                    self.layer_manager.reserve(1);
                    self.render_element_with(
                        scene,
                        node_id,
                        parent_style_transform,
                        visible,
                        Some(own_transform),
                    );
                    self.layer_manager.release(1);
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

fn in_front(w: f64) -> bool {
    w >= MIN_W
}
