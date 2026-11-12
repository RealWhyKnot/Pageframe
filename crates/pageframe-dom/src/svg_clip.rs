use crate::clip_path::{ResolvedClipPath, rounded_rect};
use crate::node::ElementData;
use crate::{BaseDocument, Node};
use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape, Vec2};
use markup5ever::{local_name, ns};
use pageframe_traits::node_id::NodeId;
use style::computed_values::visibility::T as Visibility;
use style::values::computed::url::ComputedUrl;

impl BaseDocument {
    pub fn clip_path_of(&self, node: &Node) -> Option<ResolvedClipPath> {
        node.clip_path_with(|id| self.get_element_by_id(id))
    }
}

impl Node {
    pub(crate) fn node_with_id(&self, id: &str) -> Option<NodeId> {
        let mut root = self;
        while let Some(parent) = root.parent {
            root = self.with(parent);
        }
        let mut stack = vec![root.id];
        while let Some(node_id) = stack.pop() {
            let node = self.with(node_id);
            if node.attr(local_name!("id")) == Some(id) {
                return Some(node_id);
            }
            stack.extend(node.children.iter().rev().copied());
        }
        None
    }

    pub(crate) fn svg_clip(
        &self,
        url: &ComputedUrl,
        lookup: impl Fn(&str) -> Option<NodeId>,
    ) -> Option<ResolvedClipPath> {
        let fragment = match url {
            ComputedUrl::Valid(url) => url.fragment().map(|fragment| {
                percent_encoding::percent_decode_str(fragment)
                    .decode_utf8_lossy()
                    .into_owned()
            }),
            ComputedUrl::Invalid(url) => url.strip_prefix('#').map(str::to_owned),
        }?;
        let clip = self.with(lookup(&fragment)?);
        let clip_element = clip.element_data()?;
        if clip_element.name.ns != ns!(svg) || &*clip_element.name.local != "clipPath" {
            return None;
        }

        let size = self.final_layout().size;
        let mut base = transform_of(clip_element);
        if clip_element.attr(local_name!("clipPathUnits")) == Some("objectBoundingBox") {
            base = Affine::scale_non_uniform(f64::from(size.width), f64::from(size.height)) * base;
        }

        let mut parts = Vec::new();
        for &child_id in &clip.children {
            let child = self.with(child_id);
            let Some(element) = child.element_data() else {
                continue;
            };
            if element.name.ns != ns!(svg) || is_hidden(child, element) {
                continue;
            }
            let Some((mut path, has_fill_rule)) = shape_of(element) else {
                continue;
            };
            path.apply_affine(base * transform_of(element));
            let rule = element
                .attr(local_name!("clip-rule"))
                .or_else(|| clip_element.attr(local_name!("clip-rule")));
            parts.push(ClipPart {
                path,
                evenodd: has_fill_rule && rule == Some("evenodd"),
            });
        }
        Some(merge(parts))
    }
}

struct ClipPart {
    path: BezPath,
    evenodd: bool,
}

impl ClipPart {
    fn wound(&self) -> BezPath {
        if self.evenodd {
            return nested_to_nonzero(&self.path);
        }
        if self.path.area() < 0.0 {
            self.path.reverse_subpaths()
        } else {
            self.path.clone()
        }
    }

    fn is_compound(&self) -> bool {
        self.evenodd || subpaths(&self.path).len() > 1
    }
}

fn merge(mut parts: Vec<ClipPart>) -> ResolvedClipPath {
    if parts.len() == 1
        && let Some(part) = parts.pop()
    {
        return ResolvedClipPath {
            path: part.path,
            evenodd: part.evenodd,
        };
    }
    let convex: Vec<bool> = parts.iter().map(|part| is_convex(&part.path)).collect();
    let bounds: Vec<Rect> = parts.iter().map(|part| part.path.bounding_box()).collect();
    let unites = (1..parts.len()).any(|index| {
        !convex[index]
            && bounds[..index]
                .iter()
                .any(|earlier| overlap(*earlier, bounds[index]))
    });
    let mut path = BezPath::new();
    for (part, is_convex) in parts.iter().zip(convex) {
        if !unites && !is_convex && part.is_compound() {
            path.extend(part.wound().reverse_subpaths());
        } else {
            path.extend(part.wound());
        }
    }
    ResolvedClipPath {
        path,
        evenodd: false,
    }
}

fn overlap(first: Rect, second: Rect) -> bool {
    first.x0 < second.x1 && second.x0 < first.x1 && first.y0 < second.y1 && second.y0 < first.y1
}

fn subpaths(path: &BezPath) -> Vec<BezPath> {
    let mut list: Vec<BezPath> = Vec::new();
    for element in path.elements() {
        if matches!(element, PathEl::MoveTo(_)) {
            list.push(BezPath::new());
        }
        if let Some(last) = list.last_mut() {
            last.push(*element);
        }
    }
    list
}

fn nested_to_nonzero(path: &BezPath) -> BezPath {
    let list = subpaths(path);
    let mut wound = BezPath::new();
    for (index, subpath) in list.iter().enumerate() {
        let Some(PathEl::MoveTo(start)) = subpath.elements().first() else {
            continue;
        };
        let depth = list
            .iter()
            .enumerate()
            .filter(|(other, outer)| *other != index && outer.winding(*start) != 0)
            .count();
        if (subpath.area() < 0.0) == depth.is_multiple_of(2) {
            wound.extend(subpath.reverse_subpaths());
        } else {
            wound.extend(subpath.clone());
        }
    }
    wound
}

fn is_convex(path: &BezPath) -> bool {
    if subpaths(path).len() != 1 {
        return false;
    }
    let mut points: Vec<Point> = Vec::new();
    kurbo::flatten(path, 0.05, |element| {
        if let PathEl::MoveTo(point) | PathEl::LineTo(point) = element
            && points.last() != Some(&point)
        {
            points.push(point);
        }
    });
    if points.len() > 1 && points.first() == points.last() {
        points.pop();
    }
    let count = points.len();
    if count < 3 {
        return false;
    }
    let mut turning = 0.0;
    let mut sign = 0.0;
    for index in 0..count {
        let before = points[(index + 1) % count] - points[index];
        let after = points[(index + 2) % count] - points[(index + 1) % count];
        let cross = before.cross(after);
        if cross.abs() > 1e-9 {
            if sign != 0.0 && cross.signum() != sign {
                return false;
            }
            sign = cross.signum();
        }
        turning += cross.atan2(before.dot(after));
    }
    (turning.abs() - std::f64::consts::TAU).abs() < 0.01
}

fn is_hidden(node: &Node, element: &ElementData) -> bool {
    if element.attr(local_name!("display")) == Some("none")
        || matches!(
            element.attr(local_name!("visibility")),
            Some("hidden" | "collapse")
        )
    {
        return true;
    }
    node.primary_styles().is_some_and(|style| {
        style.get_box().display.is_none()
            || style.get_inherited_box().visibility != Visibility::Visible
    })
}

fn shape_of(element: &ElementData) -> Option<(BezPath, bool)> {
    let length = |name| {
        element
            .attr(name)
            .and_then(parse_length)
            .filter(|value: &f64| value.is_finite())
    };
    match &*element.name.local {
        "rect" => {
            let x = length(local_name!("x")).unwrap_or(0.0);
            let y = length(local_name!("y")).unwrap_or(0.0);
            let width = length(local_name!("width")).filter(|value| *value > 0.0)?;
            let height = length(local_name!("height")).filter(|value| *value > 0.0)?;
            let rx = length(local_name!("rx")).filter(|value| *value >= 0.0);
            let ry = length(local_name!("ry")).filter(|value| *value >= 0.0);
            let radius = Vec2::new(
                rx.or(ry).unwrap_or(0.0).min(width / 2.0),
                ry.or(rx).unwrap_or(0.0).min(height / 2.0),
            );
            let rect = Rect::new(x, y, x + width, y + height);
            Some((rounded_rect(rect, [radius; 4]), false))
        }
        "circle" => {
            let radius = length(local_name!("r")).filter(|value| *value > 0.0)?;
            Some((ellipse(element, Vec2::new(radius, radius)), false))
        }
        "ellipse" => {
            let rx = length(local_name!("rx")).filter(|value| *value > 0.0)?;
            let ry = length(local_name!("ry")).filter(|value| *value > 0.0)?;
            Some((ellipse(element, Vec2::new(rx, ry)), false))
        }
        "polygon" | "polyline" => {
            let numbers = parse_numbers(element.attr(local_name!("points"))?)?;
            let mut points = numbers.as_chunks::<2>().0.iter().map(|&[x, y]| (x, y));
            let mut path = BezPath::new();
            path.move_to(points.next()?);
            for point in points {
                path.line_to(point);
            }
            path.close_path();
            Some((path, true))
        }
        "path" => {
            let path = BezPath::from_svg(element.attr(local_name!("d"))?).ok()?;
            Some((path, true))
        }
        _ => None,
    }
}

fn ellipse(element: &ElementData, radius: Vec2) -> BezPath {
    let center = |name| {
        element
            .attr(name)
            .and_then(parse_length)
            .filter(|value: &f64| value.is_finite())
            .unwrap_or(0.0)
    };
    let cx = center(local_name!("cx"));
    let cy = center(local_name!("cy"));
    let rect = Rect::new(cx - radius.x, cy - radius.y, cx + radius.x, cy + radius.y);
    rounded_rect(rect, [radius; 4])
}

fn parse_length(value: &str) -> Option<f64> {
    let value = value.trim();
    value.strip_suffix("px").unwrap_or(value).parse().ok()
}

fn parse_numbers(list: &str) -> Option<Vec<f64>> {
    list.split(|character: char| character == ',' || character.is_whitespace())
        .filter(|number| !number.is_empty())
        .map(|number| number.parse().ok())
        .collect()
}

fn transform_of(element: &ElementData) -> Affine {
    element
        .attr(local_name!("transform"))
        .and_then(parse_transform)
        .unwrap_or(Affine::IDENTITY)
}

fn parse_transform(list: &str) -> Option<Affine> {
    let mut transform = Affine::IDENTITY;
    let mut rest = list.trim_start_matches(|character: char| character.is_whitespace());
    while !rest.is_empty() {
        let (name, tail) = rest.split_once('(')?;
        let (arguments, tail) = tail.split_once(')')?;
        let numbers = parse_numbers(arguments)?;
        let step = match (name.trim(), numbers.as_slice()) {
            ("matrix", &[a, b, c, d, e, f]) => Affine::new([a, b, c, d, e, f]),
            ("translate", &[x]) => Affine::translate((x, 0.0)),
            ("translate", &[x, y]) => Affine::translate((x, y)),
            ("scale", &[factor]) => Affine::scale(factor),
            ("scale", &[x, y]) => Affine::scale_non_uniform(x, y),
            ("rotate", &[angle]) => Affine::rotate(angle.to_radians()),
            ("rotate", &[angle, x, y]) => Affine::rotate_about(angle.to_radians(), (x, y)),
            ("skewX", &[angle]) => Affine::skew(angle.to_radians().tan(), 0.0),
            ("skewY", &[angle]) => Affine::skew(0.0, angle.to_radians().tan()),
            _ => return None,
        };
        transform *= step;
        rest = tail
            .trim_start_matches(|character: char| character == ',' || character.is_whitespace());
    }
    Some(transform)
}
