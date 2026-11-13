use anyrender::PaintScene;
use kurbo::{BezPath, Insets, Join, Stroke, Vec2};
use pageframe_dom::CopyStyleGetters;
use pageframe_dom::node::SpecialElementData;
use peniko::{Color, Fill};
use smallvec::SmallVec;
use style::{
    computed_values::border_collapse::T as BorderCollapse,
    values::computed::BorderStyle,
    values::specified::box_::{Display, DisplayInside},
};

mod bleed;
mod contour;
mod dashed;
mod dotted;
mod groove;

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

pub(super) fn beveled_edge_color(color: Color, edge: Edge, inset: bool) -> Color {
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

fn open_dash_gap(length: f64, dash: f64, gap: f64) -> f64 {
    let fewer = ((length + gap) / (dash + gap)).floor();
    let more = fewer + 1.0;
    let fewer_gap = (length - fewer * dash) / (fewer - 1.0);
    let more_gap = (length - more * dash) / (more - 1.0);
    if more_gap <= 0.0 || (fewer_gap - gap).abs() < (more_gap - gap).abs() {
        fewer_gap
    } else {
        more_gap
    }
}

impl ElementCx<'_, '_> {
    pub(super) fn covered_border_share(&self) -> Option<f64> {
        if self.style.clone_display() == Display::Inline || !self.frame.has_border_radius() {
            return None;
        }
        let border = self.style.get_border();
        let current_color = self.style.clone_color();
        let width = self.frame.border_width;
        let sides = [
            (border.border_top_style, &border.border_top_color, width.y0),
            (
                border.border_right_style,
                &border.border_right_color,
                width.x1,
            ),
            (
                border.border_bottom_style,
                &border.border_bottom_color,
                width.y1,
            ),
            (
                border.border_left_style,
                &border.border_left_color,
                width.x0,
            ),
        ];
        let mut share: f64 = 0.5;
        for (line, color, width) in sides {
            let is_broken = matches!(
                line,
                BorderStyle::None | BorderStyle::Hidden | BorderStyle::Dotted | BorderStyle::Dashed
            );
            let color = color.resolve_to_absolute(&current_color).as_srgb_color();
            if is_broken || width <= 0.0 || color.components[3] < 1.0 {
                return None;
            }
            if line == BorderStyle::Double {
                share = share.min(1.0 / 6.0);
            }
        }
        Some(share)
    }

    /// Draw all borders for a node
    pub(crate) fn draw_border(&self, scene: &mut impl PaintScene, outer_clipped: bool) {
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
        let widths = self.frame.border_width;
        let styles = [
            (border.border_top_style, widths.y0),
            (border.border_right_style, widths.x1),
            (border.border_bottom_style, widths.y1),
            (border.border_left_style, widths.x0),
        ]
        .map(|(style, width)| groove::effective_style(style, width));
        let all_solid = styles.iter().all(|style| *style == BorderStyle::Solid);
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
                if self.frame.pixel_phase.is_some() && self.frame.has_border_radius() {
                    let mut ring = self.frame.border_box_path();
                    ring.extend(self.frame.padding_box_path());
                    scene.fill(Fill::EvenOdd, self.transform, color, None, &ring);
                    return;
                }
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

        let colors = [
            &border.border_top_color,
            &border.border_right_color,
            &border.border_bottom_color,
            &border.border_left_color,
        ]
        .map(|color| color.resolve_to_absolute(&current_color).as_srgb_color());
        self.draw_sides(scene, styles, colors, outer_clipped);
    }

    pub(super) fn draw_sides(
        &self,
        scene: &mut impl PaintScene,
        styles: [BorderStyle; 4],
        colors: [Color; 4],
        outer_clipped: bool,
    ) {
        let widths = self.frame.border_width;
        let all_dashed = styles.iter().all(|style| *style == BorderStyle::Dashed);
        let one_color = colors.iter().all(|color| *color == colors[0]);
        let one_width = widths.x0 > 0.0
            && [widths.y0, widths.x1, widths.y1]
                .iter()
                .all(|width| *width == widths.x0);
        if all_dashed
            && one_color
            && one_width
            && colors[0].components[3] > 0.0
            && self.frame.has_border_radius()
        {
            self.draw_uniform_round_dashed_border(scene, colors[0], outer_clipped);
            return;
        }

        let mut sides: SmallVec<[Side; 4]> = SmallVec::new();
        let edges = [Edge::Top, Edge::Right, Edge::Bottom, Edge::Left];
        for ((edge, edge_style), color) in edges.into_iter().zip(styles).zip(colors) {
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
                BorderStyle::Dotted => {
                    self.draw_dotted_border_edge(scene, edge, color, &shape(&self.frame));
                }
                BorderStyle::Dashed => {
                    self.draw_dashed_border_edge(scene, edge, color, &shape(&self.frame));
                }
                BorderStyle::Double => {
                    let width = self.edge_width(edge);
                    let path = if width < 3.0 {
                        shape(&self.frame)
                    } else {
                        let [outer, inner] = self.frame.double_stripes();
                        let mut path = shape(&outer);
                        path.extend(&shape(&inner));
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
                    if self.edge_width(edge) <= 1.0 {
                        borders.push((color, shape(&self.frame)));
                        continue;
                    }
                    let ridge = edge_style == BorderStyle::Ridge;
                    let (outer, inner) = grooved_edge_colors(color, edge, ridge);
                    let [outer_half, inner_half] = self.frame.groove_halves(edge);
                    let width = self.edge_width(edge);
                    let neighbours = match edge {
                        Edge::Top | Edge::Bottom => [Edge::Left, Edge::Right],
                        Edge::Left | Edge::Right => [Edge::Top, Edge::Bottom],
                    };
                    let mitred_evenly = neighbours
                        .iter()
                        .all(|side| self.edge_width(*side) == width);
                    let opaque = outer.components[3] >= 1.0;
                    let outer_shape =
                        if opaque && (self.frame.has_border_radius() || !mitred_evenly) {
                            shape(&self.frame)
                        } else {
                            shape(&outer_half)
                        };
                    borders.push((outer, outer_shape));
                    borders.push((inner, shape(&inner_half)));
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

    pub(crate) fn caption_space(&self) -> (f32, f32) {
        let SpecialElementData::TableRoot(table) = &self.element.special_data else {
            return (0.0, 0.0);
        };
        let mut above = 0.0_f32;
        let mut below = 0.0_f32;
        for caption in &table.captions {
            let Some(node) = self.context.dom.get_node(caption.node_id) else {
                continue;
            };
            let layout = node.final_layout();
            let space = layout.margin.top + layout.size.height + layout.margin.bottom;
            if caption.is_below {
                below += space;
            } else {
                above += space;
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
}
