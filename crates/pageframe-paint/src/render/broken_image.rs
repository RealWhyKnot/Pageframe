use crate::color::Color;
use anyrender::PaintScene;
use kurbo::{Affine, BezPath, Stroke};
use pageframe_dom::node::TextBrush;
use pageframe_dom::{BaseDocument, Node, NodeId};
use parley::{Layout, PositionedLayoutItem};
use peniko::Fill;
use style::values::computed::Display;

const ICON_SIZE: f64 = 16.0;
const PAPER: Color = Color::from_rgba8(236, 241, 250, 255);
const FOLD: Color = Color::from_rgba8(255, 255, 255, 255);
const OUTLINE: Color = Color::from_rgba8(150, 150, 150, 255);
const HILL: Color = Color::from_rgba8(86, 170, 60, 255);

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

    let mut page = BezPath::new();
    page.move_to((0.5, 0.5));
    page.line_to((10.5, 0.5));
    page.line_to((15.5, 5.5));
    page.line_to((15.5, 15.5));
    page.line_to((0.5, 15.5));
    page.close_path();

    let mut hill = BezPath::new();
    hill.move_to((1.0, 15.0));
    hill.line_to((1.0, 13.5));
    hill.quad_to((5.0, 8.5), (9.0, 11.0));
    hill.quad_to((11.5, 12.5), (12.5, 15.0));
    hill.close_path();

    let mut fold = BezPath::new();
    fold.move_to((10.5, 0.5));
    fold.line_to((10.5, 5.5));
    fold.line_to((15.5, 5.5));
    fold.close_path();

    let outline = Stroke::new(1.0);
    scene.fill(Fill::NonZero, transform, PAPER, None, &page);
    scene.fill(Fill::NonZero, transform, HILL, None, &hill);
    scene.fill(Fill::NonZero, transform, FOLD, None, &fold);
    scene.stroke(&outline, transform, OUTLINE, None, &page);
    scene.stroke(&outline, transform, OUTLINE, None, &fold);
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
        let inline = ancestor
            .primary_styles()
            .is_some_and(|styles| styles.get_box().display == Display::Inline);
        if !inline {
            break;
        }
        parent = ancestor.parent;
    }
    root
}
