use anyrender::PaintScene;
use blitz_dom::CopyStyleGetters;
use blitz_dom::node::SpecialElementData;
use kurbo::{BezPath, Cap, Circle, Insets, Join, PathEl, Point, Rect, Shape as _, Stroke, Vec2};
use peniko::{Color, Fill};
use smallvec::SmallVec;
use style::{
    computed_values::border_collapse::T as BorderCollapse,
    properties::ComputedValues,
    values::computed::{BorderSideWidth, BorderStyle, OutlineStyle},
    values::specified::box_::DisplayInside,
};

use crate::{
    color::ToColorColor as _,
    kurbo_css::{CssBox, Edge},
    render::{
        ElementCx,
        miter::{Side, SideShape, paint_order},
    },
};

const CHANNEL_SCALE: f32 = 255.999_98;
const DARK_THRESHOLD: f32 = 3.0 * 32.0 * 32.0;
const LIGHT_THRESHOLD: f32 = 3.0 * 20.0 * 20.0;
const LIGHTENED_BLACK: f32 = 84.0 / 255.0;

fn byte_channels(color: Color) -> [f32; 4] {
    let [r, g, b, a] = color.components;
    [
        (r * 255.0).round() / 255.0,
        (g * 255.0).round() / 255.0,
        (b * 255.0).round() / 255.0,
        a,
    ]
}

fn scaled(r: f32, g: f32, b: f32, a: f32, multiplier: f32) -> Color {
    let channel = |value: f32| (value * multiplier * CHANNEL_SCALE).floor() / 255.0;
    Color::new([channel(r), channel(g), channel(b), a])
}

fn darken(color: Color) -> Color {
    let [r, g, b, a] = byte_channels(color);
    let v = r.max(g).max(b);
    if v == 0.0 {
        return color;
    }
    scaled(r, g, b, a, ((v - 0.33) / v).max(0.0))
}

fn lighten(color: Color) -> Color {
    let [r, g, b, a] = byte_channels(color);
    let v = r.max(g).max(b);
    if v == 0.0 {
        return Color::new([LIGHTENED_BLACK, LIGHTENED_BLACK, LIGHTENED_BLACK, a]);
    }
    scaled(r, g, b, a, (v + 0.33).min(1.0) / v)
}

fn squared_distance(color: Color, target: f32) -> f32 {
    let [r, g, b, _] = byte_channels(color);
    [r, g, b]
        .iter()
        .map(|channel| ((channel - target) * 255.0).round().powi(2))
        .sum()
}

fn beveled_edge_color(color: Color, edge: Edge, inset: bool) -> Color {
    let top_or_left = matches!(edge, Edge::Top | Edge::Left);
    let (base, dark) = if squared_distance(color, 0.0) <= DARK_THRESHOLD {
        let lightened = lighten(color);
        (lightened, lightened)
    } else {
        (color, darken(color))
    };
    if top_or_left == inset {
        return dark;
    }
    if squared_distance(base, 1.0) >= LIGHT_THRESHOLD {
        lighten(base)
    } else {
        base
    }
}

/// The (outer half, inner half) colours of a single edge of a `groove`/`ridge`
/// border.
///
/// A `groove` looks carved into the page: its outer half is shaded like `inset`
/// and its inner half like `outset`. `ridge` is the reverse (raised).
fn grooved_edge_colors(color: Color, edge: Edge, ridge: bool) -> (Color, Color) {
    let outer = beveled_edge_color(color, edge, !ridge);
    let inner = beveled_edge_color(color, edge, ridge);
    (outer, inner)
}

/// The `(dash, gap)` lengths of a dashed border, as multiples of the border
/// thickness.
///
/// Matches Chrome: borders that are at least 3px use a 2:1 dash-to-gap ratio,
/// while thinner borders use longer dashes and gaps (3:2) so they still read as
/// dashes rather than dots or a solid line. `thickness` and `scale` are in device
/// pixels; the 3px threshold is applied in CSS pixels.
fn dashed_ratios(thickness: f64, scale: f64) -> (f64, f64) {
    if thickness >= 3.0 * scale {
        (2.0, 1.0)
    } else {
        (3.0, 2.0)
    }
}

/// Return `count` points spaced `spacing` apart (by arc length) along `path`,
/// starting at its beginning. Used to place dots along a rounded border.
fn sample_points_along_path(path: &BezPath, spacing: f64, count: usize) -> Vec<Point> {
    // Flatten to a polyline so we can walk it by arc length.
    let mut poly: Vec<Point> = Vec::new();
    kurbo::flatten(path.iter(), 0.1, |el| match el {
        PathEl::MoveTo(p) | PathEl::LineTo(p) => poly.push(p),
        PathEl::ClosePath => {
            if let Some(&first) = poly.first() {
                poly.push(first);
            }
        }
        _ => {}
    });

    let mut points = Vec::with_capacity(count);
    if poly.len() < 2 {
        return points;
    }

    let mut seg = 0;
    let mut seg_start = 0.0;
    let mut seg_len = (poly[1] - poly[0]).hypot();
    for i in 0..count {
        let target = i as f64 * spacing;
        while seg + 2 < poly.len() && target > seg_start + seg_len {
            seg_start += seg_len;
            seg += 1;
            seg_len = (poly[seg + 1] - poly[seg]).hypot();
        }
        let t = if seg_len > 0.0 {
            ((target - seg_start) / seg_len).clamp(0.0, 1.0)
        } else {
            0.0
        };
        points.push(poly[seg].lerp(poly[seg + 1], t));
    }
    points
}

const EDGE_SLACK: f64 = 0.5;

struct CollapsedLine {
    rect: Rect,
    color: Color,
    width: f64,
    edge: Edge,
}

struct SeamLine {
    line: CollapsedLine,
    boundary: usize,
    y: f64,
    span: std::ops::Range<f64>,
}

impl SeamLine {
    fn key(&self) -> f64 {
        (self.y * 64.0).round()
    }

    fn crosses(&self, other: &Self) -> bool {
        self.span.start < other.span.end && other.span.start < self.span.end
    }
}

fn seam_rects(seams: &mut [SeamLine]) -> Vec<(Rect, Color)> {
    seams.sort_by(|first, second| {
        first
            .key()
            .total_cmp(&second.key())
            .then(first.boundary.cmp(&second.boundary))
            .then(first.line.width.total_cmp(&second.line.width))
    });
    let mut rects = Vec::with_capacity(seams.len());
    for run in seams.chunk_by(|first, second| first.key() == second.key()) {
        let (Some(first), Some(last)) = (run.first(), run.last()) else {
            continue;
        };
        let above: Vec<&SeamLine> = run
            .iter()
            .filter(|seam| seam.boundary == first.boundary)
            .collect();
        let below: Vec<&SeamLine> = run
            .iter()
            .filter(|seam| seam.boundary == last.boundary)
            .collect();
        let widest = |group: &[&SeamLine], seam: &SeamLine| {
            group
                .iter()
                .filter(|other| other.crosses(seam))
                .map(|other| other.line.width)
                .fold(0.0, f64::max)
        };
        for seam in run {
            let half = seam.line.width / 2.0;
            let top = seam.y - half.min(widest(&above, seam) / 2.0);
            let bottom = seam.y + half.min(widest(&below, seam) / 2.0);
            if bottom > top {
                let rect = Rect::new(seam.line.rect.x0, top, seam.line.rect.x1, bottom);
                rects.push((rect, seam.line.color));
            }
        }
    }
    rects
}

fn inset(rect: Rect, by: Insets) -> Rect {
    Rect::new(
        rect.x0 + by.x0,
        rect.y0 + by.y0,
        rect.x1 - by.x1,
        rect.y1 - by.y1,
    )
}

fn layout_border(layout: &taffy::Layout) -> Insets {
    Insets::new(
        f64::from(layout.border.left),
        f64::from(layout.border.top),
        f64::from(layout.border.right),
        f64::from(layout.border.bottom),
    )
}

fn collapsed_widths(style: &ComputedValues) -> Insets {
    let border = style.get_border();
    let side = |width: &BorderSideWidth, style: BorderStyle| {
        if style.none_or_hidden() {
            0.0
        } else {
            f64::from(width.0.to_f32_px())
        }
    };
    Insets::new(
        side(&border.border_left_width, border.border_left_style),
        side(&border.border_top_width, border.border_top_style),
        side(&border.border_right_width, border.border_right_style),
        side(&border.border_bottom_width, border.border_bottom_style),
    )
}

fn push_collapsed_lines(
    lines: &mut Vec<CollapsedLine>,
    inner: Rect,
    widths: Insets,
    style: &ComputedValues,
) {
    let border = style.get_border();
    let current_color = style.clone_color();
    let outer = Rect::new(
        inner.x0 - widths.x0,
        inner.y0 - widths.y0,
        inner.x1 + widths.x1,
        inner.y1 + widths.y1,
    );
    let sides = [
        (
            widths.x0,
            &border.border_left_color,
            Rect::new(outer.x0, outer.y0, inner.x0, outer.y1),
            Edge::Left,
        ),
        (
            widths.y0,
            &border.border_top_color,
            Rect::new(outer.x0, outer.y0, outer.x1, inner.y0),
            Edge::Top,
        ),
        (
            widths.x1,
            &border.border_right_color,
            Rect::new(inner.x1, outer.y0, outer.x1, outer.y1),
            Edge::Right,
        ),
        (
            widths.y1,
            &border.border_bottom_color,
            Rect::new(outer.x0, inner.y1, outer.x1, outer.y1),
            Edge::Bottom,
        ),
    ];
    for (width, color, rect, edge) in sides {
        let color = color.resolve_to_absolute(&current_color).as_srgb_color();
        if width > 0.0 && color != Color::TRANSPARENT {
            lines.push(CollapsedLine {
                rect,
                color,
                width,
                edge,
            });
        }
    }
}

impl ElementCx<'_, '_> {
    /// Draw all borders for a node
    pub(crate) fn draw_border(&self, scene: &mut impl PaintScene) {
        let style = &*self.style;
        let border = style.get_border();
        let current_color = style.clone_color();

        // Fast path: a uniform-width, single-color solid border whose corners are
        // all circular arcs (equal x and y radius — sharp rectangles, ordinary
        // rounded rectangles and circles all qualify) can be drawn as a single
        // stroke along its center-line instead of a fill per edge. Besides being
        // faster, this works around Vello seam artifacts at the quarter-arc joins
        // when a thin circular/elliptical border is drawn as the
        // border-box/padding-box annulus.
        //
        // A full ellipse (`border-radius: 50%` on a non-square box) has the same
        // seam problem but no rounded-rect representation, so it is stroked as an
        // ellipse.
        let all_solid = border.border_top_style == BorderStyle::Solid
            && border.border_right_style == BorderStyle::Solid
            && border.border_bottom_style == BorderStyle::Solid
            && border.border_left_style == BorderStyle::Solid;
        if all_solid {
            let color = border
                .border_top_color
                .resolve_to_absolute(&current_color)
                .as_srgb_color();
            let same_color = [
                &border.border_right_color,
                &border.border_bottom_color,
                &border.border_left_color,
            ]
            .iter()
            .all(|c| c.resolve_to_absolute(&current_color).as_srgb_color() == color);
            let bw = self.frame.border_width;
            let uniform_width = bw.x0 == bw.y0 && bw.x0 == bw.x1 && bw.x0 == bw.y1 && bw.x0 > 0.0;
            if same_color && uniform_width && color.components[3] > 0.0 {
                let bb = self.frame.border_box;
                let pb = self.frame.padding_box;
                let width = (bb.width() - pb.width()) / 2.0;
                // Miter join keeps sharp corners square. It has no effect on rounded
                // corners, whose arcs meet the straight edges tangentially.
                let stroke = Stroke::new(width).with_join(Join::Miter);

                if self.frame.is_uniform_corner_border() {
                    let radii = self.frame.border_radii;
                    // The stroke matches the CSS border shape only when each corner is
                    // sharp or its radius is at least the border width (outer radius
                    // `r`, inner radius `r - width`, concentric). For 0 < r < width CSS
                    // draws a rounded outer edge against a sharp inner corner, which a
                    // stroke can't represent — leave those to the per-edge fill.
                    let is_exact = |r: Vec2| r.x == 0.0 || r.x >= width;
                    let stroke_is_exact = is_exact(radii.top_left)
                        && is_exact(radii.top_right)
                        && is_exact(radii.bottom_right)
                        && is_exact(radii.bottom_left);
                    if stroke_is_exact {
                        let half_width = width / 2.0;
                        // Center-line rect, and corner radii shrunk to match.
                        let centerline_box = bb - Insets::uniform(half_width);
                        let centerline_radius = |r: Vec2| (r.x - half_width).max(0.0);
                        let radii = kurbo::RoundedRectRadii::new(
                            centerline_radius(radii.top_left),
                            centerline_radius(radii.top_right),
                            centerline_radius(radii.bottom_right),
                            centerline_radius(radii.bottom_left),
                        );
                        let rr = kurbo::RoundedRect::from_rect(centerline_box, radii);
                        scene.stroke(&stroke, self.transform, color, None, &rr);
                        return;
                    }
                } else if self.frame.is_elliptical_border() {
                    // Center-line radii, straight from the boxes.
                    let rx = (bb.width() + pb.width()) / 4.0;
                    let ry = (bb.height() + pb.height()) / 4.0;
                    let e = kurbo::Ellipse::new(bb.center(), (rx, ry), 0.0);
                    scene.stroke(&stroke, self.transform, color, None, &e);
                    return;
                }
            }
        }

        let mut sides: SmallVec<[Side; 4]> = SmallVec::new();
        for edge in [Edge::Top, Edge::Right, Edge::Bottom, Edge::Left] {
            let (color, edge_style) = match edge {
                Edge::Top => (&border.border_top_color, border.border_top_style),
                Edge::Right => (&border.border_right_color, border.border_right_style),
                Edge::Bottom => (&border.border_bottom_color, border.border_bottom_style),
                Edge::Left => (&border.border_left_color, border.border_left_style),
            };
            let color = color.resolve_to_absolute(&current_color).as_srgb_color();
            let hidden = matches!(edge_style, BorderStyle::None | BorderStyle::Hidden);
            if color.components[3] > 0.0 && !hidden && self.edge_width(edge) > 0.0 {
                sides.push((edge, edge_style, color));
            }
        }
        sides.sort_by(|a, b| paint_order(a).total_cmp(&paint_order(b)));

        let mut borders: SmallVec<[(Color, BezPath); 8]> = SmallVec::new();
        for (index, &(edge, edge_style, color)) in sides.iter().enumerate() {
            let side_shape = SideShape::new(&self.frame, self.transform, &sides, index);
            let shape = |frame: &CssBox| side_shape.path(frame, edge);

            match edge_style {
                BorderStyle::None | BorderStyle::Hidden => {}
                BorderStyle::Dotted => self.draw_dotted_border_edge(scene, edge, color),
                BorderStyle::Dashed => self.draw_dashed_border_edge(scene, edge, color),
                BorderStyle::Double => {
                    let path = if self.edge_width(edge) < 3.0 * self.scale {
                        shape(&self.frame)
                    } else {
                        let mut path = shape(&self.frame.border_slice(0.0, 1.0 / 3.0));
                        path.extend(&shape(&self.frame.border_slice(2.0 / 3.0, 1.0)));
                        path
                    };
                    borders.push((color, path));
                }
                BorderStyle::Inset | BorderStyle::Outset => {
                    let inset = edge_style == BorderStyle::Inset;
                    let shade = beveled_edge_color(color, edge, inset);
                    borders.push((shade, shape(&self.frame)));
                }
                BorderStyle::Groove | BorderStyle::Ridge => {
                    let ridge = edge_style == BorderStyle::Ridge;
                    let (outer, inner) = grooved_edge_colors(color, edge, ridge);
                    borders.push((outer, shape(&self.frame.border_slice(0.0, 0.5))));
                    borders.push((inner, shape(&self.frame.border_slice(0.5, 1.0))));
                }
                BorderStyle::Solid => borders.push((color, shape(&self.frame))),
            }
        }

        let mut start = 0;
        while start < borders.len() {
            let color = borders[start].0;
            let mut path = std::mem::take(&mut borders[start].1);
            let mut next = start + 1;
            while next < borders.len() && borders[next].0 == color {
                path.extend(&borders[next].1);
                next += 1;
            }
            scene.fill(Fill::NonZero, self.transform, color, None, &path);
            start = next;
        }
    }

    /// The border width (in device pixels) of a single edge.
    fn edge_width(&self, edge: Edge) -> f64 {
        match edge {
            Edge::Top => self.frame.border_width.y0,
            Edge::Bottom => self.frame.border_width.y1,
            Edge::Left => self.frame.border_width.x0,
            Edge::Right => self.frame.border_width.x1,
        }
    }

    /// Draw a single `dashed` border edge.
    ///
    /// The dashes are produced by stroking the centre line of the border with a
    /// dash pattern (butt caps give them flat, square ends). Everything is clipped
    /// to the edge's region (the same trapezoid used by the solid path) so corners
    /// are mitred, adjacent edges of different colors don't overlap, and any
    /// border-radius is respected.
    fn draw_dashed_border_edge(&self, scene: &mut impl PaintScene, edge: Edge, color: Color) {
        let thickness = self.edge_width(edge);
        if thickness <= 0.0 {
            return;
        }

        let (dash_ratio, gap_ratio) = dashed_ratios(thickness, self.scale);

        // Work out the centre line to stroke and the dash/gap lengths along it.
        let (centerline, dash, gap) = if self.frame.has_border_radius() {
            // Rounded corners: stroke the rounded centre line running through the
            // whole perimeter. Every edge uses the same centre line and pattern, so
            // dashes stay continuous and aligned as they wrap around each corner.
            // Dash and gap keep their ratio, sized so a whole number of periods fit
            // exactly around the perimeter (kurbo merges the dash across the seam).
            let mut centerline = self.frame.border_slice(0.0, 0.5).padding_box_path();
            centerline.close_path();
            let perimeter = centerline.perimeter(0.1);
            if perimeter <= 0.0 {
                return;
            }
            let period0 = (dash_ratio + gap_ratio) * thickness;
            let count = (perimeter / period0).round().max(1.0);
            let period = perimeter / count;
            let dash = period * dash_ratio / (dash_ratio + gap_ratio);
            (centerline, dash, period - dash)
        } else {
            // Square corners: stroke a straight line through the middle of the edge,
            // corner to corner. Dash and gap keep their ratio but are scaled so the
            // edge both starts and ends with a dash (covering the corners).
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
            let dash0 = dash_ratio * thickness;
            let gap0 = gap_ratio * thickness;
            // `count` dashes with `count - 1` gaps between them.
            let count = ((length + gap0) / (dash0 + gap0)).round().max(1.0);
            let r = dash_ratio / gap_ratio;
            let gap = length / (count * r + count - 1.0);

            let mut line = BezPath::new();
            line.move_to(start);
            line.line_to(end);
            (line, r * gap, gap)
        };

        let stroke = Stroke::new(thickness)
            .with_caps(Cap::Butt)
            .with_dashes(0.0, [dash, gap]);
        let clip = self.frame.border_edge_shape(edge);
        scene.push_clip_layer(Fill::NonZero, self.transform, &clip);
        scene.stroke(&stroke, self.transform, color, None, &centerline);
        scene.pop_layer();
    }

    /// Draw a single `dotted` border edge.
    ///
    /// Dots are filled circles (diameter == border thickness). Unlike dashes they
    /// can't be produced by stroking, because kurbo doesn't emit zero-length dashes
    /// (so a round-capped dash pattern would render nothing); drawing them
    /// explicitly also lets us anchor a dot in each square corner. Everything is
    /// clipped to the edge's region, as for the other styles.
    fn draw_dotted_border_edge(&self, scene: &mut impl PaintScene, edge: Edge, color: Color) {
        let thickness = self.edge_width(edge);
        if thickness <= 0.0 {
            return;
        }
        let radius = thickness / 2.0;

        let mut path = BezPath::new();
        if self.frame.has_border_radius() {
            // Rounded corners: dots spaced evenly around the rounded centre line so
            // the ring wraps seamlessly around the corners.
            let mut centerline = self.frame.border_slice(0.0, 0.5).padding_box_path();
            centerline.close_path();
            let perimeter = centerline.perimeter(0.1);
            if perimeter <= 0.0 {
                return;
            }
            let count = (perimeter / (2.0 * thickness)).round().max(1.0);
            let spacing = perimeter / count;
            for center in sample_points_along_path(&centerline, spacing, count as usize) {
                path.extend(Circle::new(center, radius).path_elements(0.1));
            }
        } else {
            // Square corners: a dot is anchored in each corner (both ends of the
            // edge, inset by the radius so it sits snugly in the corner) and the
            // rest are spread evenly between them.
            let bb = self.frame.border_box;
            let length = match edge {
                Edge::Top | Edge::Bottom => bb.width(),
                Edge::Left | Edge::Right => bb.height(),
            };
            if length <= 0.0 {
                return;
            }
            let dot_center = |along: f64| -> Point {
                match edge {
                    Edge::Top => Point::new(bb.x0 + along, bb.y0 + radius),
                    Edge::Bottom => Point::new(bb.x0 + along, bb.y1 - radius),
                    Edge::Left => Point::new(bb.x0 + radius, bb.y0 + along),
                    Edge::Right => Point::new(bb.x1 - radius, bb.y0 + along),
                }
            };

            let span = length - thickness;
            if span <= 0.0 {
                // Edge too short to fit two dots; place a single centred dot.
                let center = dot_center(length / 2.0);
                path.extend(Circle::new(center, radius).path_elements(0.1));
            } else {
                // Aim for a centre-to-centre spacing of about two dot diameters,
                // then adjust it so the first and last dots land in the corners.
                let gaps = (span / (2.0 * thickness)).round().max(1.0) as usize;
                let spacing = span / gaps as f64;
                for i in 0..=gaps {
                    let center = dot_center(radius + i as f64 * spacing);
                    path.extend(Circle::new(center, radius).path_elements(0.1));
                }
            }
        }

        let clip = self.frame.border_edge_shape(edge);
        scene.push_clip_layer(Fill::NonZero, self.transform, &clip);
        scene.fill(Fill::NonZero, self.transform, color, None, &path);
        scene.pop_layer();
    }

    pub(crate) fn caption_space(&self) -> (f32, f32) {
        let SpecialElementData::TableRoot(table) = &self.element.special_data else {
            return (0.0, 0.0);
        };
        let height = self.node.final_layout().size.height;
        let mut above = 0.0_f32;
        let mut below = 0.0_f32;
        for caption in &table.captions {
            let Some(node) = self.context.dom.get_node(caption.node_id) else {
                continue;
            };
            let layout = node.final_layout();
            let top = layout.location.y;
            if caption.is_below {
                below = below.max(height - top + layout.margin.top);
            } else {
                above = above.max(top + layout.size.height + layout.margin.bottom);
            }
        }
        (above, below)
    }

    pub(crate) fn has_collapsed_borders(&self) -> bool {
        if self.style.clone_border_collapse() != BorderCollapse::Collapse {
            return false;
        }
        match self.style.clone_display().inside() {
            DisplayInside::Table => true,
            DisplayInside::TableCell => {
                let removed = self
                    .node
                    .layout_parent
                    .get()
                    .and_then(|id| self.context.dom.get_node(id))
                    .and_then(|node| node.element_data())
                    .is_some_and(|element| match &element.special_data {
                        SpecialElementData::TableRoot(table) => {
                            table.cell_is_collapsed(self.node.id)
                        }
                        _ => false,
                    });
                self.element.has_collapsed_border() && !removed
            }
            _ => false,
        }
    }

    pub(crate) fn draw_table_borders(&self, scene: &mut impl PaintScene) {
        let SpecialElementData::TableRoot(table) = &self.element.special_data else {
            return;
        };
        if table.has_collapsed_columns() {
            self.context.deferred_paints.borrow_mut().push(
                crate::render::DeferredPaint::TableBorders {
                    node_id: self.node.id,
                    transform: self.transform,
                },
            );
        } else {
            self.paint_table_borders(scene);
        }
    }

    pub(crate) fn paint_table_borders(&self, scene: &mut impl PaintScene) {
        let SpecialElementData::TableRoot(table) = &self.element.special_data else {
            return;
        };
        if table.border_collapse != BorderCollapse::Collapse {
            return;
        }

        let layout = self.node.final_layout();
        let (above, below) = self.caption_space();
        let table_box = Rect::new(
            0.0,
            f64::from(above),
            f64::from(layout.size.width),
            f64::from(layout.size.height - below),
        );
        let cells_box = inset(table_box, layout_border(layout));
        let own = self.style.get_border();
        let hidden = |style: BorderStyle| style == BorderStyle::Hidden;

        let mut lines = Vec::new();
        let mut seams = Vec::new();
        let has_seams = table.has_collapsed_rows();
        let own_widths = collapsed_widths(&self.style);
        push_collapsed_lines(
            &mut lines,
            inset(table_box, own_widths),
            own_widths,
            &self.style,
        );
        for cell in table.cells.iter().rev() {
            let Some(node) = self.context.dom.get_node(cell.node_id) else {
                continue;
            };
            let Some(style) = node.primary_styles() else {
                continue;
            };
            let layout = node.final_layout();
            let left = f64::from(layout.location.x);
            let top = f64::from(layout.location.y);
            let cell_box = Rect::new(
                left,
                top,
                left + f64::from(layout.size.width),
                top + f64::from(
                    table
                        .collapsed_cell_span_height(cell.node_id)
                        .unwrap_or(layout.size.height),
                ),
            );
            let mut widths = collapsed_widths(&style);
            if hidden(own.border_left_style) && cell_box.x0 <= cells_box.x0 + EDGE_SLACK {
                widths.x0 = 0.0;
            }
            if hidden(own.border_top_style) && cell_box.y0 <= cells_box.y0 + EDGE_SLACK {
                widths.y0 = 0.0;
            }
            if hidden(own.border_right_style) && cell_box.x1 >= cells_box.x1 - EDGE_SLACK {
                widths.x1 = 0.0;
            }
            if hidden(own.border_bottom_style) && cell_box.y1 >= cells_box.y1 - EDGE_SLACK {
                widths.y1 = 0.0;
            }
            let first = lines.len();
            push_collapsed_lines(
                &mut lines,
                inset(cell_box, layout_border(layout)),
                widths,
                &style,
            );
            if has_seams {
                let [top, bottom] = table.seam_edges(cell);
                let mut index = first;
                while index < lines.len() {
                    let seam = match lines[index].edge {
                        Edge::Top => top.map(|boundary| (boundary, cell_box.y0)),
                        Edge::Bottom => bottom.map(|boundary| (boundary, cell_box.y1)),
                        Edge::Left | Edge::Right => None,
                    };
                    match seam {
                        Some((boundary, y)) => seams.push(SeamLine {
                            line: lines.remove(index),
                            boundary,
                            y,
                            span: cell_box.x0..cell_box.x1,
                        }),
                        None => index += 1,
                    }
                }
            }
        }

        lines.sort_by(|first, second| first.width.total_cmp(&second.width));
        for run in lines.chunk_by(|first, second| first.color == second.color) {
            let Some(line) = run.first() else {
                continue;
            };
            let mut shape = BezPath::new();
            for line in run {
                shape.extend(line.rect.scale_from_origin(self.scale).path_elements(0.1));
            }
            scene.fill(Fill::NonZero, self.transform, line.color, None, &shape);
        }
        for (rect, color) in seam_rects(&mut seams) {
            let rect = rect.scale_from_origin(self.scale);
            scene.fill(Fill::NonZero, self.transform, color, None, &rect);
        }
    }

    /// ❌ dotted - Defines a dotted border
    /// ❌ dashed - Defines a dashed border
    /// ✅ solid - Defines a solid border
    /// ❌ double - Defines a double border
    /// ❌ groove - Defines a 3D grooved border. The effect depends on the border-color value
    /// ❌ ridge - Defines a 3D ridged border. The effect depends on the border-color value
    /// ❌ inset - Defines a 3D inset border. The effect depends on the border-color value
    /// ❌ outset - Defines a 3D outset border. The effect depends on the border-color value
    /// ✅ none - Defines no border
    /// ✅ hidden - Defines a hidden border
    pub(crate) fn draw_outline(&self, scene: &mut impl PaintScene) {
        let outline = self.style.get_outline();

        let current_color = self.style.clone_color();
        let color = outline
            .outline_color
            .resolve_to_absolute(&current_color)
            .as_srgb_color();

        let style = match outline.outline_style {
            OutlineStyle::Auto => return,
            OutlineStyle::BorderStyle(style) => style,
        };

        let path = match style {
            BorderStyle::None | BorderStyle::Hidden => return,
            BorderStyle::Solid => self.frame.outline(),

            // TODO: Implement other border styles
            BorderStyle::Inset
            | BorderStyle::Groove
            | BorderStyle::Outset
            | BorderStyle::Ridge
            | BorderStyle::Dotted
            | BorderStyle::Dashed
            | BorderStyle::Double => self.frame.outline(),
        };

        scene.fill(Fill::NonZero, self.transform, color, None, &path);
    }
}
