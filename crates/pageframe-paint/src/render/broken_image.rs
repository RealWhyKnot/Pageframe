use crate::color::Color;
use anyrender::PaintScene;
use kurbo::{Affine, BezPath, Line, Point, Rect, Stroke};
use pageframe_dom::node::TextBrush;
use pageframe_dom::{BaseDocument, Node, NodeId};
use parley::{Layout, PositionedLayoutItem};
use peniko::{Fill, Gradient};
use style::values::computed::Display;

const ICON_SIZE: f64 = 16.0;
const FOLD_Y: f64 = 4.0;
const CORNER: f64 = 11.25;
const HILL_TOP: f64 = 9.9;
const TEAR_LEFT: [(f64, f64); 5] = [
    (6.5, ICON_SIZE),
    (7.7, 14.8),
    (8.6, 14.3),
    (10.4, 12.5),
    (ICON_SIZE, 7.5),
];
const TEAR_RIGHT: [(f64, f64); 6] = [
    (ICON_SIZE, 11.0),
    (14.0, 12.3),
    (12.2, 14.25),
    (11.4, 14.75),
    (10.8, 15.25),
    (10.3, ICON_SIZE),
];
const WHITE: Color = Color::from_rgba8(255, 255, 255, 255);
const GRASS: Color = Color::from_rgba8(88, 174, 57, 255);

fn rgb(red: u8, green: u8, blue: u8) -> Color {
    Color::from_rgba8(red, green, blue, 255)
}

fn grey(level: f64) -> Color {
    let value = (level / 255.0).clamp(0.0, 1.0) as f32;
    Color::new([value, value, value, 1.0])
}

fn vertical(stops: &[(f32, Color)]) -> Gradient {
    Gradient::new_linear(Point::new(0.0, 0.0), Point::new(0.0, ICON_SIZE)).with_stops(stops)
}

fn polygon(points: impl IntoIterator<Item = (f64, f64)>) -> BezPath {
    let mut path = BezPath::new();
    for (index, point) in points.into_iter().enumerate() {
        if index == 0 {
            path.move_to(point);
        } else {
            path.line_to(point);
        }
    }
    path.close_path();
    path
}

fn hill() -> BezPath {
    let mut path = BezPath::new();
    path.move_to((0.09, ICON_SIZE));
    path.curve_to((4.09, 10.5), (5.64, HILL_TOP), (7.8, HILL_TOP));
    path.curve_to((9.96, HILL_TOP), (11.51, 10.5), (15.51, ICON_SIZE));
    path.close_path();
    path
}

fn cloud(px: f64) -> BezPath {
    let bottom = (6.5 / px).floor() * px;
    let mut path = BezPath::new();
    path.move_to((1.9, bottom));
    path.curve_to((1.9, 5.3), (2.4, 4.7), (3.5, 4.4));
    path.curve_to((3.6, 2.6), (7.2, 2.5), (7.6, 4.4));
    path.line_to((7.9, 5.0));
    path.line_to((7.9, bottom));
    path.close_path();
    path
}

fn fill_rects(scene: &mut impl PaintScene, transform: Affine, rects: &[(Rect, Color)]) {
    for &(rect, color) in rects {
        scene.fill(Fill::NonZero, transform, color, None, &rect);
    }
}

fn sky() -> Gradient {
    vertical(&[
        (0.0, rgb(232, 240, 253)),
        (0.09, rgb(207, 223, 250)),
        (0.28, rgb(203, 219, 247)),
        (0.56, rgb(195, 212, 243)),
        (0.81, rgb(187, 205, 238)),
    ])
}

fn draw_landscape(scene: &mut impl PaintScene, transform: Affine, px: f64, fold_x: f64) {
    let end = ICON_SIZE;
    let fine = (2.0 - 2.0 * px).clamp(0.0, 1.0) as f32;
    let shade = rgb(205, 214, 229).with_alpha(fine);
    let page = polygon([
        (0.0, 0.0),
        (CORNER, 0.0),
        (end, end - CORNER),
        (end, end),
        (0.0, end),
    ]);
    let sky = sky();
    scene.fill(Fill::NonZero, transform, &sky, None, &page);
    let rim = vertical(&[(0.12, rgb(202, 212, 228)), (0.94, rgb(193, 204, 223))]);
    scene.fill(
        Fill::NonZero,
        transform,
        &rim,
        None,
        &Rect::new(px, 2.0 * px, 2.0 * px, end),
    );
    scene.fill(Fill::NonZero, transform, WHITE, None, &cloud(px));
    scene.fill(Fill::NonZero, transform, GRASS, None, &hill());
    let broken = polygon(TEAR_RIGHT.into_iter().chain([(end, end)]));
    scene.fill(Fill::NonZero, transform, &sky, None, &broken);
    let mut piece = BezPath::new();
    piece.move_to((11.5, 13.0));
    piece.quad_to((14.2, 13.9), (end, end));
    piece.line_to(TEAR_RIGHT[5]);
    piece.close_path();
    scene.fill(Fill::NonZero, transform, GRASS, None, &piece);
    fill_rects(
        scene,
        transform,
        &[
            (Rect::new(px, px, fold_x, 2.0 * px), rgb(236, 243, 253)),
            (
                Rect::new(px, 2.0 * px, fold_x, 3.0 * px),
                rgb(217, 229, 251),
            ),
            (
                Rect::new(fold_x + px, FOLD_Y + px, end - 2.0 * px, FOLD_Y + 2.0 * px),
                rgb(226, 234, 246),
            ),
            (
                Rect::new(
                    fold_x + px,
                    FOLD_Y + 2.0 * px,
                    end - 2.0 * px,
                    FOLD_Y + 3.0 * px,
                ),
                rgb(212, 225, 247),
            ),
            (
                Rect::new(end - 2.0 * px, FOLD_Y + px, end - px, end),
                rgb(210, 220, 239),
            ),
            (Rect::new(fold_x - px, px, fold_x, FOLD_Y + px), shade),
        ],
    );
}

fn draw_frame(scene: &mut impl PaintScene, transform: Affine, px: f64, fold_x: f64) {
    let end = ICON_SIZE;
    let coarse = ((px - 0.5) * 2.0).clamp(0.0, 1.0);
    let fold = polygon([
        (fold_x, 0.0),
        (CORNER, 0.0),
        (CORNER + FOLD_Y + px, FOLD_Y + px),
        (fold_x, FOLD_Y + px),
    ]);
    scene.fill(Fill::NonZero, transform, rgb(250, 250, 250), None, &fold);
    let frame = vertical(&[
        (0.0, grey(156.3 + 7.4 * coarse)),
        (1.0, grey(133.9 + 6.9 * coarse)),
    ]);
    for rect in [
        Rect::new(0.0, 0.0, CORNER, px),
        Rect::new(0.0, 0.0, px, end),
        Rect::new(end - px, FOLD_Y, end, end),
    ] {
        scene.fill(Fill::NonZero, transform, &frame, None, &rect);
    }
    fill_rects(
        scene,
        transform,
        &[
            (
                Rect::new(0.0, end - px, end, end),
                grey(116.0 + 15.0 * coarse),
            ),
            (
                Rect::new(fold_x, 0.0, fold_x + px, FOLD_Y + px),
                rgb(173, 173, 174),
            ),
            (
                Rect::new(fold_x, FOLD_Y, end, FOLD_Y + px),
                grey(151.0 - 6.0 * coarse),
            ),
        ],
    );
    let crease = Line::new((CORNER, 0.0), (end, end - CORNER));
    scene.stroke(&Stroke::new(px), transform, grey(162.0), None, &crease);
}

fn draw_icon(scene: &mut impl PaintScene, transform: Affine) {
    let scale = transform.determinant().abs().sqrt();
    let px = if scale.is_finite() && scale > 0.0 {
        scale.recip()
    } else {
        1.0
    };
    let fold_x = 12.0 - 2.0 * px.clamp(0.5, 1.0);
    draw_landscape(scene, transform, px, fold_x);
    draw_frame(scene, transform, px, fold_x);
    let tear = polygon(TEAR_LEFT.into_iter().chain(TEAR_RIGHT));
    scene.fill(Fill::NonZero, transform, WHITE, None, &tear);
}

pub(super) fn draw_broken_image_icon(
    scene: &mut impl PaintScene,
    layout: &Layout<TextBrush>,
    transform: Affine,
) {
    let icon = layout
        .lines()
        .flat_map(|line| line.items())
        .find_map(|item| match item {
            PositionedLayoutItem::InlineBox(inline_box) => Some(inline_box),
            PositionedLayoutItem::GlyphRun(_) => None,
        });
    let Some(icon) = icon else {
        return;
    };
    let origin = (f64::from(icon.x), f64::from(icon.y));
    let transform =
        transform * Affine::translate(origin) * Affine::scale(f64::from(icon.width) / ICON_SIZE);
    draw_icon(scene, transform);
}

pub(super) fn decoration_root(doc: &BaseDocument, node: &Node, framed: bool) -> NodeId {
    let flows = node.primary_styles().is_some_and(|styles| {
        let style = styles.get_box();
        style.display == Display::Inline
            || (style.display == Display::Block
                && !style.position.is_absolutely_positioned()
                && !style.float.is_floating())
    });
    if framed || !flows {
        return node.id;
    }
    let mut root = node.id;
    let mut parent = node.parent;
    while let Some(ancestor) = parent.and_then(|id| doc.get_node(id)) {
        root = ancestor.id;
        let passes = ancestor.primary_styles().is_some_and(|styles| {
            let display = styles.get_box().display;
            display == Display::Inline || display.is_contents()
        });
        if !passes {
            break;
        }
        parent = ancestor.parent;
    }
    root
}
