use crate::node::Node;
use markup5ever::{LocalName, local_name};
use style::properties::PropertyDeclaration;
use style::values::specified::BorderSideWidth;

fn border_px(value: &str) -> u32 {
    style::servo::attr::parse_unsigned_integer(value.chars()).unwrap_or(1)
}

fn widths(px: u32) -> Vec<PropertyDeclaration> {
    let width = BorderSideWidth::from_px(px as f32);
    vec![
        PropertyDeclaration::BorderTopWidth(width.clone()),
        PropertyDeclaration::BorderRightWidth(width.clone()),
        PropertyDeclaration::BorderBottomWidth(width.clone()),
        PropertyDeclaration::BorderLeftWidth(width),
    ]
}

fn cell_table(cell: &Node) -> Option<&Node> {
    let tree = cell.tree();
    let mut node = cell;
    for _ in 0..3 {
        node = tree.get(node.parent?)?;
        if node.data.is_element_with_tag_name(&local_name!("table")) {
            return Some(node);
        }
    }
    None
}

pub(super) fn declarations(node: &Node, tag: &LocalName) -> Vec<PropertyDeclaration> {
    if *tag == local_name!("table") {
        return node
            .attr(local_name!("border"))
            .map_or_else(Vec::new, |value| widths(border_px(value)));
    }
    if *tag != local_name!("td") && *tag != local_name!("th") {
        return Vec::new();
    }
    let unbordered = cell_table(node).is_some_and(|table| {
        table.attr(local_name!("rules")).is_none()
            && table
                .attr(local_name!("border"))
                .is_some_and(|value| border_px(value) == 0)
    });
    if unbordered { widths(0) } else { Vec::new() }
}
