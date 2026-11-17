use crate::Node;
use kurbo::{Affine, BezPath, Point, Rect, Shape, SvgArc, Vec2};
use pageframe_traits::node_id::NodeId;
use style::properties::ComputedValues;
use style::values::computed::basic_shape::{BasicShape, ClipPath};
use style::values::computed::{Angle, CSSPixelLength, LengthPercentage};
use style::values::generics::basic_shape::{
    ArcSize, ArcSweep, AxisEndPoint, CommandEndPoint, ControlPoint, ControlReference, FillRule,
    GenericBasicShape, GenericPathOrShapeFunction, GenericShapeCommand, GenericShapeRadius,
    ShapeBox, ShapeGeometryBox,
};
use style::values::generics::position::{GenericPosition, GenericPositionOrAuto};
use taffy::Layout;

pub struct ResolvedClipPath {
    pub path: BezPath,
    pub evenodd: bool,
}

impl ResolvedClipPath {
    pub fn contains(&self, point: Point) -> bool {
        let winding = self.path.winding(point);
        if self.evenodd {
            winding % 2 != 0
        } else {
            winding != 0
        }
    }
}

impl Node {
    pub fn clip_path(&self) -> Option<ResolvedClipPath> {
        self.clip_path_with(|id| self.node_with_id(id))
    }

    pub(crate) fn clip_path_with(
        &self,
        lookup: impl Fn(&str) -> Option<NodeId>,
    ) -> Option<ResolvedClipPath> {
        let style = self.primary_styles()?;
        if matches!(style.get_clip_path(), ClipPath::None) {
            return None;
        }
        let unsnapped = self.layout_data().unsnapped;
        let mut resolved = match style.get_clip_path() {
            ClipPath::None => return None,
            ClipPath::Url(url) => self.svg_clip(url, unsnapped.size, lookup)?,
            _ => {
                let mut layout = *self.final_layout();
                layout.size = unsnapped.size;
                layout.border = unsnapped.border;
                layout.padding = unsnapped.padding;
                ClipResolver {
                    style: &style,
                    layout: &layout,
                }
                .resolve()?
            }
        };
        resolved.path.apply_affine(Affine::translate((
            f64::from(unsnapped.offset.x),
            f64::from(unsnapped.offset.y),
        )));
        Some(resolved)
    }
}

struct ClipResolver<'a> {
    style: &'a ComputedValues,
    layout: &'a Layout,
}

impl ClipResolver<'_> {
    fn resolve(&self) -> Option<ResolvedClipPath> {
        let (path, evenodd) = match self.style.get_clip_path() {
            ClipPath::None | ClipPath::Url(_) => return None,
            ClipPath::Shape(shape, geometry_box) => {
                let reference = self.reference_box(geometry_box);
                let evenodd = match &**shape {
                    GenericBasicShape::Polygon(p) => p.fill == FillRule::Evenodd,
                    GenericBasicShape::PathOrShape(GenericPathOrShapeFunction::Path(p)) => {
                        p.fill == FillRule::Evenodd
                    }
                    GenericBasicShape::PathOrShape(GenericPathOrShapeFunction::Shape(p)) => {
                        p.fill == FillRule::Evenodd
                    }
                    _ => false,
                };
                (self.basic_shape_to_path(shape, reference)?, evenodd)
            }
            ClipPath::Box(geometry_box) => {
                let reference = self.reference_box(geometry_box);
                let w = f64::from(self.layout.size.width);
                let h = f64::from(self.layout.size.height);
                let b = self.style.get_border();
                let mut radii = [
                    &b.border_top_left_radius,
                    &b.border_top_right_radius,
                    &b.border_bottom_right_radius,
                    &b.border_bottom_left_radius,
                ]
                .map(|r| Vec2::new(resolve_lp(&r.0.width.0, w), resolve_lp(&r.0.height.0, h)));
                scale_radii(w, h, &mut radii);
                let right = w - reference.x - reference.width;
                let bottom = h - reference.y - reference.height;
                for (r, inset) in radii.iter_mut().zip([
                    Vec2::new(reference.x, reference.y),
                    Vec2::new(right, reference.y),
                    Vec2::new(right, bottom),
                    Vec2::new(reference.x, bottom),
                ]) {
                    r.x = (r.x - inset.x).max(0.0);
                    r.y = (r.y - inset.y).max(0.0);
                }
                (rounded_rect(reference.into(), radii), false)
            }
        };
        Some(ResolvedClipPath { path, evenodd })
    }

    fn reference_box(&self, geometry_box: &ShapeGeometryBox) -> ReferenceBox {
        let l = self.layout;
        let (left, top, right, bottom) = match geometry_box {
            ShapeGeometryBox::ShapeBox(ShapeBox::PaddingBox) => {
                (l.border.left, l.border.top, l.border.right, l.border.bottom)
            }
            ShapeGeometryBox::FillBox | ShapeGeometryBox::ShapeBox(ShapeBox::ContentBox) => (
                l.border.left + l.padding.left,
                l.border.top + l.padding.top,
                l.border.right + l.padding.right,
                l.border.bottom + l.padding.bottom,
            ),
            _ => (0.0, 0.0, 0.0, 0.0),
        };
        ReferenceBox {
            x: f64::from(left),
            y: f64::from(top),
            width: f64::from((l.size.width - left - right).max(0.0)),
            height: f64::from((l.size.height - top - bottom).max(0.0)),
        }
    }
    fn basic_shape_to_path(
        &self,
        shape: &BasicShape,
        reference_box: ReferenceBox,
    ) -> Option<BezPath> {
        let w = reference_box.width;
        let h = reference_box.height;
        let ox = reference_box.x;
        let oy = reference_box.y;

        match shape {
            GenericBasicShape::Circle(circle) => {
                let (cx, cy) = resolve_position(&circle.position, w, h, ox, oy);
                let r = resolve_circle_radius(&circle.radius, w, h, cx - ox, cy - oy);
                Some(oval(Point::new(cx, cy), Vec2::new(r, r)))
            }
            GenericBasicShape::Ellipse(ellipse) => {
                let (cx, cy) = resolve_position(&ellipse.position, w, h, ox, oy);
                let rx = resolve_axis_radius(&ellipse.semiaxis_x, w, cx - ox);
                let ry = resolve_axis_radius(&ellipse.semiaxis_y, h, cy - oy);
                Some(oval(Point::new(cx, cy), Vec2::new(rx, ry)))
            }
            GenericBasicShape::Polygon(polygon) => {
                let mut path = BezPath::new();
                let _fill = &polygon.fill;
                let coords = &polygon.coordinates;

                if coords.is_empty() {
                    return Some(BezPath::new());
                }

                for (i, coord) in coords.iter().enumerate() {
                    let x = ox + resolve_lp(&coord.0, w);
                    let y = oy + resolve_lp(&coord.1, h);
                    if i == 0 {
                        path.move_to(Point::new(x, y));
                    } else {
                        path.line_to(Point::new(x, y));
                    }
                }
                path.close_path();

                Some(path)
            }
            GenericBasicShape::Rect(inset_rect) => {
                let top = resolve_lp(&inset_rect.rect.0, h);
                let right = resolve_lp(&inset_rect.rect.1, w);
                let bottom = resolve_lp(&inset_rect.rect.2, h);
                let left = resolve_lp(&inset_rect.rect.3, w);

                let x0 = ox + left;
                let y0 = oy + top;
                let x1 = ox + w - right;
                let y1 = oy + h - bottom;

                if x1 <= x0 || y1 <= y0 {
                    return Some(BezPath::new());
                }

                let r = &inset_rect.round;
                let radii = [&r.top_left, &r.top_right, &r.bottom_right, &r.bottom_left]
                    .map(|r| Vec2::new(resolve_lp(&r.0.width.0, w), resolve_lp(&r.0.height.0, h)));
                Some(rounded_rect(Rect::new(x0, y0, x1, y1), radii))
            }
            GenericBasicShape::PathOrShape(path_or_shape) => match path_or_shape {
                GenericPathOrShapeFunction::Path(path) => svg_path_to_bezpath(
                    path.commands(),
                    |v| *v as f64,
                    |v| *v as f64,
                    |v| *v as f64,
                )
                .map(|mut p| {
                    p.apply_affine(kurbo::Affine::translate((ox, oy)));
                    p
                }),
                GenericPathOrShapeFunction::Shape(shape) => svg_path_to_bezpath(
                    &shape.commands,
                    move |v: &LengthPercentage| {
                        v.resolve(CSSPixelLength::new(w as f32)).px() as f64
                    },
                    move |v: &LengthPercentage| {
                        v.resolve(CSSPixelLength::new(h as f32)).px() as f64
                    },
                    move |v: &Angle| v.degrees() as f64,
                )
                .map(|mut p| {
                    p.apply_affine(kurbo::Affine::translate((ox, oy)));
                    p
                }),
            },
        }
    }
}

#[derive(Clone, Copy)]
struct ReferenceBox {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl From<ReferenceBox> for kurbo::Rect {
    fn from(reference_box: ReferenceBox) -> Self {
        kurbo::Rect::new(
            reference_box.x,
            reference_box.y,
            reference_box.x + reference_box.width,
            reference_box.y + reference_box.height,
        )
    }
}

fn resolve_lp(lp: &LengthPercentage, basis: f64) -> f64 {
    let basis_css = CSSPixelLength::new(basis as f32);
    lp.resolve(basis_css).px() as f64
}

fn resolve_position(
    position: &GenericPositionOrAuto<style::values::computed::Position>,
    w: f64,
    h: f64,
    ox: f64,
    oy: f64,
) -> (f64, f64) {
    match position {
        GenericPositionOrAuto::Auto => (ox + w / 2.0, oy + h / 2.0),
        GenericPositionOrAuto::Position(pos) => {
            let x = ox + resolve_lp(&pos.horizontal, w);
            let y = oy + resolve_lp(&pos.vertical, h);
            (x, y)
        }
    }
}

fn resolve_circle_radius(
    radius: &GenericShapeRadius<LengthPercentage>,
    primary_size: f64,
    secondary_size: f64,
    center_offset_primary: f64,
    center_offset_secondary: f64,
) -> f64 {
    match radius {
        GenericShapeRadius::Length(lp) => resolve_lp(
            &lp.0,
            primary_size.hypot(secondary_size) / std::f64::consts::SQRT_2,
        ),
        GenericShapeRadius::ClosestSide => center_offset_primary
            .abs()
            .min((primary_size - center_offset_primary).abs())
            .min(center_offset_secondary.abs())
            .min((secondary_size - center_offset_secondary).abs()),
        GenericShapeRadius::FarthestSide => center_offset_primary
            .max(primary_size - center_offset_primary)
            .max(center_offset_secondary)
            .max(secondary_size - center_offset_secondary),

        GenericShapeRadius::FarthestCorner => {
            let dp = center_offset_primary.max(primary_size - center_offset_primary);
            let ds = center_offset_secondary.max(secondary_size - center_offset_secondary);
            dp.hypot(ds)
        }
        GenericShapeRadius::ClosestCorner => {
            let dp = center_offset_primary
                .abs()
                .min((primary_size - center_offset_primary).abs());
            let ds = center_offset_secondary
                .abs()
                .min((secondary_size - center_offset_secondary).abs());
            dp.hypot(ds)
        }
    }
}

type GenericPathCommand<Angle, N> = GenericShapeCommand<Angle, N, GenericPosition<N, N>, N>;

fn svg_path_to_bezpath<Angle: Copy, N>(
    commands: &[GenericPathCommand<Angle, N>],
    resolve_x: impl Fn(&N) -> f64,
    resolve_y: impl Fn(&N) -> f64,
    resolve_angle: impl Fn(&Angle) -> f64,
) -> Option<BezPath> {
    if commands.is_empty() {
        return None;
    }

    let mut bez = BezPath::new();
    let mut cur = Point::ZERO;
    let mut subpath_start = cur;
    let mut last_control: Option<Point> = None;

    for cmd in commands {
        match &cmd {
            GenericShapeCommand::Close => {
                bez.close_path();
                cur = subpath_start;
                last_control = None;
            }
            GenericShapeCommand::Move { point } => {
                let p = resolve_endpoint(point, cur, &resolve_x, &resolve_y);
                bez.move_to(p);
                cur = p;
                subpath_start = p;
                last_control = None;
            }
            GenericShapeCommand::Line { point } => {
                let p = resolve_endpoint(point, cur, &resolve_x, &resolve_y);
                bez.line_to(p);
                cur = p;
                last_control = None;
            }
            GenericShapeCommand::HLine { x } => {
                cur.x = resolve_axis_endpoint(x, cur.x, &resolve_x);
                bez.line_to(cur);
                last_control = None;
            }
            GenericShapeCommand::VLine { y } => {
                cur.y = resolve_axis_endpoint(y, cur.y, &resolve_y);
                bez.line_to(cur);
                last_control = None;
            }
            GenericShapeCommand::CubicCurve {
                point,
                control1,
                control2,
            } => {
                let p = resolve_endpoint(point, cur, &resolve_x, &resolve_y);
                let c1 = resolve_control_point(control1, cur, p, &resolve_x, &resolve_y);
                let c2 = resolve_control_point(control2, cur, p, &resolve_x, &resolve_y);
                bez.curve_to(c1, c2, p);
                last_control = Some(c2);
                cur = p;
            }
            GenericShapeCommand::QuadCurve { point, control1 } => {
                let p = resolve_endpoint(point, cur, &resolve_x, &resolve_y);
                let c1 = resolve_control_point(control1, cur, p, &resolve_x, &resolve_y);
                bez.quad_to(c1, p);
                last_control = Some(c1);
                cur = p;
            }
            GenericShapeCommand::SmoothCubic { point, control2 } => {
                let p = resolve_endpoint(point, cur, &resolve_x, &resolve_y);
                let c2 = resolve_control_point(control2, cur, p, &resolve_x, &resolve_y);
                let c1 = reflect_point(last_control, cur);
                bez.curve_to(c1, c2, p);
                last_control = Some(c2);
                cur = p;
            }
            GenericShapeCommand::SmoothQuad { point } => {
                let p = resolve_endpoint(point, cur, &resolve_x, &resolve_y);
                let c1 = reflect_point(last_control, cur);
                bez.quad_to(c1, p);
                last_control = Some(c1);
                cur = p;
            }
            GenericShapeCommand::Arc {
                point,
                radii,
                arc_sweep,
                arc_size,
                rotate,
            } => {
                let p = resolve_endpoint(point, cur, &resolve_x, &resolve_y);
                let svg_arc = SvgArc {
                    from: cur,
                    to: p,
                    radii: Vec2 {
                        x: resolve_x(&radii.rx),
                        y: resolve_y(radii.ry.as_ref().unwrap_or(&radii.rx)),
                    },
                    x_rotation: resolve_angle(rotate),
                    large_arc: match arc_size {
                        ArcSize::Large => true,
                        ArcSize::Small => false,
                    },
                    sweep: match arc_sweep {
                        ArcSweep::Ccw => true,
                        ArcSweep::Cw => false,
                    },
                };
                let arc = kurbo::Arc::from_svg_arc(&svg_arc)?;
                for el in arc.append_iter(0.1) {
                    bez.push(el);
                }
                last_control = None;
                cur = p;
            }
        }
    }

    Some(bez)
}

fn resolve_endpoint<N>(
    endpoint: &CommandEndPoint<GenericPosition<N, N>, N>,
    cur: Point,
    resolve_x: impl Fn(&N) -> f64,
    resolve_y: impl Fn(&N) -> f64,
) -> Point {
    match endpoint {
        CommandEndPoint::ToPosition(pos) => {
            Point::new(resolve_x(&pos.horizontal), resolve_y(&pos.vertical))
        }
        CommandEndPoint::ByCoordinate(coord) => {
            Point::new(cur.x + resolve_x(&coord.x), cur.y + resolve_y(&coord.y))
        }
    }
}

fn resolve_control_point<N>(
    control_point: &ControlPoint<GenericPosition<N, N>, N>,
    cur: Point,
    end: Point,
    resolve_x: impl Fn(&N) -> f64,
    resolve_y: impl Fn(&N) -> f64,
) -> Point {
    match control_point {
        ControlPoint::Absolute(pos) => {
            Point::new(resolve_x(&pos.horizontal), resolve_y(&pos.vertical))
        }
        ControlPoint::Relative(rel) => {
            let base = match rel.reference {
                ControlReference::Start => cur,
                ControlReference::End => end,
                ControlReference::Origin => Point::ZERO,
            };
            Point::new(
                base.x + resolve_x(&rel.coord.x),
                base.y + resolve_y(&rel.coord.y),
            )
        }
    }
}

fn reflect_point(last_control: Option<Point>, cur: Point) -> Point {
    match last_control {
        Some(ctrl) => Point::new(2.0 * cur.x - ctrl.x, 2.0 * cur.y - ctrl.y),
        None => cur,
    }
}

fn resolve_axis_endpoint<N>(
    endpoint: &AxisEndPoint<N, N>,
    cur_val: f64,
    resolve: impl Fn(&N) -> f64,
) -> f64 {
    match endpoint {
        AxisEndPoint::ToPosition(pos) => resolve(pos),
        AxisEndPoint::ByCoordinate(val) => cur_val + resolve(val),
    }
}

fn resolve_axis_radius(
    radius: &GenericShapeRadius<LengthPercentage>,
    size: f64,
    center: f64,
) -> f64 {
    match radius {
        GenericShapeRadius::Length(lp) => resolve_lp(&lp.0, size),
        GenericShapeRadius::ClosestSide | GenericShapeRadius::ClosestCorner => {
            center.abs().min((size - center).abs())
        }
        GenericShapeRadius::FarthestSide | GenericShapeRadius::FarthestCorner => {
            center.abs().max((size - center).abs())
        }
    }
}

fn scale_radii(w: f64, h: f64, r: &mut [Vec2; 4]) {
    let factor = (w / (r[0].x + r[1].x))
        .min(w / (r[3].x + r[2].x))
        .min(h / (r[0].y + r[3].y))
        .min(h / (r[1].y + r[2].y))
        .min(1.0);
    if factor < 1.0 {
        for radius in r {
            *radius *= factor;
        }
    }
}

fn oval(center: Point, radii: Vec2) -> BezPath {
    let rect = Rect::from_center_size(center, (radii * 2.0).to_size());
    rounded_rect(rect, [radii; 4])
}

pub(crate) fn rounded_rect(rect: Rect, mut r: [Vec2; 4]) -> BezPath {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return BezPath::new();
    }
    scale_radii(rect.width(), rect.height(), &mut r);
    for radius in &mut r {
        if radius.x == 0.0 || radius.y == 0.0 {
            *radius = Vec2::ZERO;
        }
    }
    let Rect { x0, y0, x1, y1 } = rect;
    let k = 0.5522847498307936;
    let mut p = BezPath::new();
    p.move_to((x0 + r[0].x, y0));
    p.line_to((x1 - r[1].x, y0));
    p.curve_to(
        (x1 - r[1].x * (1.0 - k), y0),
        (x1, y0 + r[1].y * (1.0 - k)),
        (x1, y0 + r[1].y),
    );
    p.line_to((x1, y1 - r[2].y));
    p.curve_to(
        (x1, y1 - r[2].y * (1.0 - k)),
        (x1 - r[2].x * (1.0 - k), y1),
        (x1 - r[2].x, y1),
    );
    p.line_to((x0 + r[3].x, y1));
    p.curve_to(
        (x0 + r[3].x * (1.0 - k), y1),
        (x0, y1 - r[3].y * (1.0 - k)),
        (x0, y1 - r[3].y),
    );
    p.line_to((x0, y0 + r[0].y));
    p.curve_to(
        (x0, y0 + r[0].y * (1.0 - k)),
        (x0 + r[0].x * (1.0 - k), y0),
        (x0 + r[0].x, y0),
    );
    p.close_path();
    p
}
