use icu_properties::CodePointMapData;
use icu_properties::props::BidiClass;
use markup5ever::local_name;
use style::computed_values::direction::T as Direction;

use crate::node::{ElementData, Node, NodeData};

pub(crate) fn has_auto_direction(element: &ElementData) -> bool {
    match element
        .attr(local_name!("dir"))
        .filter(|dir| is_valid_dir(dir))
    {
        Some(dir) => dir.eq_ignore_ascii_case("auto"),
        None => element.name.local == local_name!("bdi"),
    }
}

fn is_valid_dir(dir: &str) -> bool {
    ["ltr", "rtl", "auto"]
        .iter()
        .any(|valid| dir.eq_ignore_ascii_case(valid))
}

pub(crate) fn auto_direction(node: &Node) -> Direction {
    let control_value = node
        .element_data()
        .and_then(ElementData::text_input_data)
        .map(|input| input.value());
    match control_value {
        Some(value) => text_direction(value),
        None => contained_direction(node),
    }
    .unwrap_or(Direction::Ltr)
}

pub(crate) fn blocks_auto_direction(element: &ElementData) -> bool {
    matches!(
        element.name.local,
        local_name!("bdi") | local_name!("script") | local_name!("style") | local_name!("textarea")
    ) || element.attr(local_name!("dir")).is_some_and(is_valid_dir)
}

fn contained_direction(node: &Node) -> Option<Direction> {
    let mut pending: Vec<_> = node.children.iter().rev().copied().collect();
    while let Some(id) = pending.pop() {
        let child = node.with(id);
        match &child.data {
            NodeData::Text(text) => {
                if let Some(direction) = text_direction(&text.content) {
                    return Some(direction);
                }
            }
            NodeData::Element(element) if !blocks_auto_direction(element) => {
                pending.extend(child.children.iter().rev().copied());
            }
            _ => {}
        }
    }
    None
}

fn text_direction(text: &str) -> Option<Direction> {
    let classes = CodePointMapData::<BidiClass>::new();
    text.chars().find_map(|ch| match classes.get(ch) {
        BidiClass::LeftToRight => Some(Direction::Ltr),
        BidiClass::RightToLeft | BidiClass::ArabicLetter => Some(Direction::Rtl),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::text_direction;
    use style::computed_values::direction::T as Direction;

    #[test]
    fn first_strong_character_picks_the_direction() {
        assert_eq!(text_direction("12 \u{5d0}b"), Some(Direction::Rtl));
        assert_eq!(text_direction("1 b\u{5d0}"), Some(Direction::Ltr));
        assert_eq!(text_direction("\u{200f}bc"), Some(Direction::Rtl));
        assert_eq!(text_direction("\u{627}"), Some(Direction::Rtl));
        assert_eq!(text_direction("12 ."), None);
    }
}
