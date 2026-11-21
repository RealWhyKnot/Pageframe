use pageframe_dom::{BaseDocument, NodeId, local_name, node::Node};
use smallvec::SmallVec;
use style::values::specified::box_::{DisplayInside, DisplayOutside};

pub(super) type Ancestors = SmallVec<[NodeId; 8]>;

pub(super) fn decorating_ancestors(doc: &BaseDocument, root: NodeId) -> Ancestors {
    let mut ancestors = Ancestors::new();
    let mut current = doc.get_node(root);
    while let Some(node) = current.filter(|node| passes_decorations(node)) {
        current = node.parent.and_then(|id| doc.get_node(id));
        if let Some(parent) = current {
            ancestors.push(parent.id);
        }
    }
    ancestors
}

fn passes_decorations(node: &Node) -> bool {
    let replaced = node.element_data().is_some_and(|element| {
        matches!(
            element.name.local,
            local_name!("img")
                | local_name!("svg")
                | local_name!("canvas")
                | local_name!("video")
                | local_name!("embed")
                | local_name!("iframe")
                | local_name!("object")
                | local_name!("input")
                | local_name!("textarea")
                | local_name!("select")
                | local_name!("button")
        )
    });
    !replaced
        && node.primary_styles().is_none_or(|styles| {
            let style = styles.get_box();
            let display = style.display;
            let atomic = display.outside() == DisplayOutside::Inline
                && matches!(
                    display.inside(),
                    DisplayInside::FlowRoot
                        | DisplayInside::Flex
                        | DisplayInside::Grid
                        | DisplayInside::GridLanes
                        | DisplayInside::Table
                );
            !atomic && !style.position.is_absolutely_positioned() && !style.float.is_floating()
        })
}
