use markup5ever::local_name;
use pageframe_traits::node_id::NodeId;
use parley::FontFamily;
use style::computed_values::list_style_type::T as ListStyleType;
use style::{
    computed_values::list_style_position::T as ListStylePosition, counter_style::CounterStyle,
    selector_parser::PseudoElement, shared_lock::StylesheetGuards, stylist::RuleInclusion,
    values::computed::Content,
};
use stylo_taffy::CopyStyleGetters;

use crate::{
    BaseDocument,
    layout::{construct::pe_content_text, counter_style},
    node::{ListItemLayout, ListItemLayoutPosition, Marker, MarkerShape, MarkerSymbol},
    stylo_to_parley,
};

pub(super) const BULLET_FONT_FAMILY: &str = "Bullet, monospace, sans-serif";

const NO_BREAK_SPACE: char = '\u{a0}';

const LIST_ITEM_COUNTER: &str = "list-item";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pass {
    Local,
    List,
    Refresh,
    Tail,
}

impl Pass {
    fn goes_through_wrappers(self) -> bool {
        self != Pass::Local
    }
}

pub(super) fn number_list_items(doc: &mut BaseDocument, node_id: NodeId) {
    if is_list_element(doc, node_id) {
        number_from(doc, node_id, Pass::List);
        return;
    }
    let items: Vec<NodeId> = doc.nodes[node_id]
        .children
        .iter()
        .copied()
        .filter(|child_id| is_list_item(doc, *child_id))
        .collect();
    let owner = (!items.is_empty())
        .then(|| owning_list(doc, node_id))
        .flatten();
    let Some(owner) = owner else {
        number_from(doc, node_id, Pass::Local);
        return;
    };
    for item in items {
        if let Some(element_data) = doc.nodes[item].element_data_mut() {
            element_data.list_item_data = None;
        }
    }
    if !renumber_from_wrapper(doc, owner, node_id) {
        number_from(doc, owner, Pass::Refresh);
    }
}

fn renumber_from_wrapper(doc: &mut BaseDocument, owner: NodeId, wrapper: NodeId) -> bool {
    let (start, reversed) = list_start(doc, owner);
    if reversed {
        return false;
    }
    let step = 1;
    let before = if is_list_item(doc, wrapper) {
        Some(wrapper)
    } else {
        previous_item(doc, owner, wrapper)
    };
    let mut ordinal = match before {
        Some(item) => {
            let stored = doc.nodes[item]
                .element_data()
                .and_then(|element_data| element_data.list_item_data.as_ref())
                .map(|data| data.ordinal);
            match stored {
                Some(stored) => stored,
                None => return false,
            }
        }
        None => start.unwrap_or(1).saturating_sub(step),
    };
    collect_list_item_children(doc, &mut ordinal, step, wrapper, Pass::List);

    let mut current = wrapper;
    while current != owner {
        let Some(parent) = doc.nodes[current].parent else {
            break;
        };
        let position = doc.nodes[parent]
            .children
            .iter()
            .position(|child_id| *child_id == current);
        let mut index = position.map_or(usize::MAX, |position| position + 1);
        while let Some(sibling) = doc.nodes[parent].children.get(index).copied() {
            if !number_node(doc, &mut ordinal, step, sibling, Pass::Tail) {
                return true;
            }
            index += 1;
        }
        current = parent;
    }
    true
}

fn previous_item(doc: &BaseDocument, owner: NodeId, wrapper: NodeId) -> Option<NodeId> {
    let mut current = wrapper;
    while current != owner {
        let parent = doc.nodes[current].parent?;
        let children = &doc.nodes[parent].children;
        let position = children.iter().position(|child_id| *child_id == current)?;
        let before = children[..position]
            .iter()
            .rev()
            .find_map(|sibling| last_item(doc, *sibling));
        if before.is_some() {
            return before;
        }
        if is_list_item(doc, parent) {
            return Some(parent);
        }
        current = parent;
    }
    None
}

fn last_item(doc: &BaseDocument, node_id: NodeId) -> Option<NodeId> {
    let is_item = is_list_item(doc, node_id);
    if !is_item && !is_wrapper(doc, node_id) {
        return None;
    }
    doc.nodes[node_id]
        .children
        .iter()
        .rev()
        .find_map(|child_id| last_item(doc, *child_id))
        .or(is_item.then_some(node_id))
}

fn list_start(doc: &BaseDocument, node_id: NodeId) -> (Option<i32>, bool) {
    let element = doc.nodes[node_id].element_data();
    let is_ordered = element.is_some_and(|element| element.name.local == local_name!("ol"));
    let start: Option<i32> = element
        .filter(|_| is_ordered)
        .and_then(|element| element.attr_parsed(local_name!("start")));
    let reversed = is_ordered
        && element.is_some_and(|element| element.attr(local_name!("reversed")).is_some());
    (start, reversed)
}

fn number_from(doc: &mut BaseDocument, node_id: NodeId, pass: Pass) {
    let through_wrappers = pass.goes_through_wrappers();
    let (start, reversed) = list_start(doc, node_id);
    let step = if reversed { -1 } else { 1 };
    let first = start.unwrap_or_else(|| {
        if reversed {
            i32::try_from(count_list_items(doc, node_id, through_wrappers)).unwrap_or(i32::MAX)
        } else {
            1
        }
    });
    let mut ordinal = first.saturating_sub(step);
    collect_list_item_children(doc, &mut ordinal, step, node_id, pass);
}

fn owning_list(doc: &BaseDocument, node_id: NodeId) -> Option<NodeId> {
    let mut ancestor = doc.nodes[node_id].parent;
    while let Some(ancestor_id) = ancestor {
        if is_list_element(doc, ancestor_id) {
            return Some(ancestor_id);
        }
        ancestor = doc.nodes[ancestor_id].parent;
    }
    None
}

fn is_wrapper(doc: &BaseDocument, node_id: NodeId) -> bool {
    !is_list_element(doc, node_id)
        && doc.nodes[node_id]
            .primary_styles()
            .is_some_and(|style| !style.get_box().display.is_none())
}

fn count_list_items(doc: &BaseDocument, node_id: NodeId, through_wrappers: bool) -> usize {
    doc.nodes[node_id]
        .children
        .iter()
        .map(|child_id| {
            if is_list_item(doc, *child_id) {
                1 + count_list_items(doc, *child_id, through_wrappers)
            } else if through_wrappers && is_wrapper(doc, *child_id) {
                count_list_items(doc, *child_id, through_wrappers)
            } else {
                0
            }
        })
        .sum()
}

fn collect_list_item_children(
    doc: &mut BaseDocument,
    ordinal: &mut i32,
    step: i32,
    node_id: NodeId,
    pass: Pass,
) -> bool {
    let children = doc.nodes[node_id].children.clone();
    children
        .into_iter()
        .all(|child| number_node(doc, ordinal, step, child, pass))
}

fn number_node(
    doc: &mut BaseDocument,
    ordinal: &mut i32,
    step: i32,
    child: NodeId,
    pass: Pass,
) -> bool {
    if !is_list_item(doc, child) {
        if let Some(element_data) = doc.nodes[child].element_data_mut() {
            element_data.list_item_data = None;
        }
        return !(pass.goes_through_wrappers() && is_wrapper(doc, child))
            || collect_list_item_children(doc, ordinal, step, child, pass);
    }
    let (increment, set) = list_item_counters(doc, child);
    *ordinal = ordinal.saturating_add(increment.unwrap_or(step));
    if let Some(value) = set.or_else(|| list_item_value(doc, child)) {
        *ordinal = value;
    }
    let is_current = matches!(pass, Pass::Refresh | Pass::Tail)
        && doc.nodes[child]
            .element_data()
            .and_then(|element_data| element_data.list_item_data.as_ref())
            .is_some_and(|data| data.ordinal == *ordinal);
    if is_current && pass == Pass::Tail {
        return false;
    }
    if !is_current {
        let layout = list_item_layout(doc, child, *ordinal).map(Box::new);
        if let Some(element_data) = doc.nodes[child].element_data_mut() {
            element_data.list_item_data = layout;
        }
    }
    collect_list_item_children(doc, ordinal, step, child, pass)
}

fn is_list_element(doc: &BaseDocument, node_id: NodeId) -> bool {
    doc.nodes[node_id]
        .element_data()
        .is_some_and(|element_data| {
            matches!(
                element_data.name.local,
                local_name!("ol") | local_name!("ul"),
            )
        })
}

fn is_list_item(doc: &BaseDocument, node_id: NodeId) -> bool {
    !is_list_element(doc, node_id)
        && doc.nodes[node_id]
            .primary_styles()
            .is_some_and(|style| style.get_box().display.is_list_item())
}

fn list_item_counters(doc: &BaseDocument, node_id: NodeId) -> (Option<i32>, Option<i32>) {
    let Some(styles) = doc.nodes[node_id].primary_styles() else {
        return (None, None);
    };
    let counters = styles.get_counters();
    let increment = counters
        .counter_increment
        .iter()
        .rev()
        .find(|pair| &*pair.name.0 == LIST_ITEM_COUNTER)
        .map(|pair| pair.value);
    let set = counters
        .counter_set
        .iter()
        .rev()
        .find(|pair| &*pair.name.0 == LIST_ITEM_COUNTER)
        .map(|pair| pair.value);
    (increment, set)
}

fn list_item_value(doc: &BaseDocument, node_id: NodeId) -> Option<i32> {
    let element = doc.nodes[node_id].element_data()?;
    if element.name.local != local_name!("li") {
        return None;
    }
    element.attr_parsed(local_name!("value"))
}

fn list_item_layout(
    doc: &mut BaseDocument,
    child_id: NodeId,
    ordinal: i32,
) -> Option<ListItemLayout> {
    let node = &doc.nodes[child_id];
    let styles = node.primary_styles()?;
    let list_style_type = styles.get_list_style_type();
    let list_style_position = styles.clone_list_style_position();
    let marker_styles = {
        let read_guard = doc.guard.read();
        doc.stylist.lazily_compute_pseudo_element_style(
            &StylesheetGuards::same(&read_guard),
            node,
            &PseudoElement::Marker,
            RuleInclusion::All,
            &styles,
            true,
            None,
        )
    };
    let content = marker_styles
        .as_ref()
        .map(|style| &style.get_counters().content);
    let mut marker = match content {
        Some(Content::None) => return None,
        Some(Content::Items(_)) => Marker::String(
            marker_styles
                .as_deref()
                .and_then(pe_content_text)
                .unwrap_or_default(),
        ),
        _ => marker_for_style(list_style_type, ordinal)?,
    };
    let is_outside = matches!(list_style_position, ListStylePosition::Outside);
    let text_styles = marker_styles.as_ref().unwrap_or(&*styles);

    let layout = if is_outside || matches!(marker, Marker::Symbol(_)) {
        let mut parley_style = stylo_to_parley::style(child_id, text_styles);
        match marker {
            Marker::Char(_) => parley_style.font_family = FontFamily::from(BULLET_FONT_FAMILY),
            Marker::Symbol(_) => parley_style.upright_glyphs = parley::UprightGlyphs::None,
            Marker::String(_) => {}
        }

        let mut font_ctx = doc
            .font_ctx
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut builder =
            doc.layout_ctx
                .tree_builder(&mut font_ctx, doc.viewport.scale(), true, &parley_style);
        builder.set_line_break_override(Some(crate::util::LINE_BREAK_OVERRIDE));
        builder.set_base_direction(stylo_to_parley::base_direction(
            styles.clone_direction(),
            styles.clone_unicode_bidi(),
        ));

        match &marker {
            Marker::Symbol(_) => builder.push_text(&marker.inside_text()),
            Marker::Char(char) => builder.push_text(char.encode_utf8(&mut [0; 4])),
            Marker::String(text) => builder.push_text(&outside_marker_text(text)),
        };

        let mut layout = builder.build().0;
        let width = layout.calculate_content_widths().max;
        layout.break_all_lines(Some(width));
        Some(layout)
    } else {
        None
    };

    if let (Marker::Symbol(symbol), Some(layout)) = (&mut marker, &layout) {
        if let Some(line) = layout.lines().next() {
            let metrics = line.metrics();
            symbol.ascent = metrics.baseline - metrics.content_block_min_coord;
            symbol.descent = metrics.content_block_max_coord - metrics.baseline;
        }
        symbol.space_advance = layout.full_width();
    }

    let position = match layout {
        Some(layout) if is_outside => ListItemLayoutPosition::Outside(Box::new(layout)),
        _ => ListItemLayoutPosition::Inside,
    };

    Some(ListItemLayout {
        ordinal,
        marker,
        marker_color: marker_styles.as_ref().map(|style| style.clone_color()),
        position,
        content_box_subpixel_offset: taffy::Point::ZERO,
    })
}

fn outside_marker_text(text: &str) -> String {
    match text.strip_suffix(' ') {
        Some(head) => format!("{head}{NO_BREAK_SPACE}"),
        None => text.to_string(),
    }
}

fn marker_for_style(list_style_type: &ListStyleType, ordinal: i32) -> Option<Marker> {
    Some(match &list_style_type.0 {
        CounterStyle::None => return None,
        CounterStyle::Name(name) => match &*name.0 {
            "disc" => symbol(MarkerShape::Disc),
            "circle" => symbol(MarkerShape::Circle),
            "square" => symbol(MarkerShape::Square),
            "disclosure-open" => Marker::Char('▾'),
            "disclosure-closed" => Marker::Char('▸'),
            name => Marker::String(format!(
                "{}{}",
                counter_style::text(name, ordinal),
                counter_style::suffix(name)
            )),
        },
        CounterStyle::String(atom_string) => Marker::String(atom_string.as_ref().to_string()),
        CounterStyle::Symbols { symbols, .. } => {
            let symbols = &symbols.0;
            if symbols.is_empty() {
                symbol(MarkerShape::Disc)
            } else {
                let index = usize::try_from(ordinal.saturating_sub(1)).unwrap_or(0);
                let marker = match &symbols[index % symbols.len()] {
                    style::counter_style::Symbol::String(text) => text.to_string(),
                    style::counter_style::Symbol::Ident(ident) => ident.0.to_string(),
                };
                Marker::String(marker)
            }
        }
    })
}

fn symbol(shape: MarkerShape) -> Marker {
    Marker::Symbol(MarkerSymbol {
        shape,
        ascent: 0.0,
        descent: 0.0,
        space_advance: 0.0,
    })
}

#[cfg(test)]
mod tests {
    use crate::node::{Marker, MarkerShape, MarkerSymbol};
    use style::{
        Atom,
        counter_style::CounterStyle,
        values::{CustomIdent, computed::ListStyleType},
    };

    use super::{marker_for_style, outside_marker_text, symbol};

    fn list_style(s: &str) -> ListStyleType {
        ListStyleType(CounterStyle::Name(CustomIdent(Atom::from(s))))
    }

    fn text_marker(style: &str, ordinal: i32) -> Option<Marker> {
        marker_for_style(&list_style(style), ordinal)
    }

    fn string(text: &str) -> Option<Marker> {
        Some(Marker::String(text.to_string()))
    }

    #[test]
    fn test_marker_for_disc() {
        let result = marker_for_style(&ListStyleType::disc(), 1);
        assert_eq!(result, Some(symbol(MarkerShape::Disc)));
    }

    #[test]
    fn symbol_geometry_follows_chrome() {
        let geometry = |ascent: f32| {
            let symbol = MarkerSymbol {
                shape: MarkerShape::Disc,
                ascent,
                descent: 0.0,
                space_advance: 0.0,
            };
            (symbol.offset(), symbol.size(), symbol.top_from_baseline())
        };
        assert_eq!(geometry(12.67), (8.0, 4.0, -6.0));
        assert_eq!(geometry(25.34), (16.0, 8.0, -12.0));
        assert_eq!(geometry(21.38), (14.0, 7.0, -11.0));
    }

    #[test]
    fn test_marker_for_decimal() {
        assert_eq!(text_marker("decimal", 1), string("1. "));
        assert_eq!(text_marker("decimal", 2), string("2. "));
        assert_eq!(text_marker("decimal", -3), string("-3. "));
    }

    #[test]
    fn test_marker_for_lower_alpha() {
        assert_eq!(text_marker("lower-alpha", 1), string("a. "));
        assert_eq!(text_marker("lower-alpha", 2), string("b. "));
        assert_eq!(text_marker("lower-alpha", 27), string("aa. "));
        assert_eq!(text_marker("lower-alpha", 28), string("ab. "));
    }

    #[test]
    fn test_marker_for_upper_alpha() {
        assert_eq!(text_marker("upper-alpha", 1), string("A. "));
        assert_eq!(text_marker("upper-alpha", 2), string("B. "));
        assert_eq!(text_marker("upper-alpha", 27), string("AA. "));
        assert_eq!(text_marker("upper-alpha", 28), string("AB. "));
    }

    #[test]
    fn test_marker_for_other_counter_styles() {
        assert_eq!(text_marker("upper-roman", 9), string("IX. "));
        assert_eq!(text_marker("decimal-leading-zero", 2), string("02. "));
        assert_eq!(text_marker("cjk-decimal", 1), string("一、"));
        assert_eq!(text_marker("no-such-style", 4), string("4. "));
    }

    #[test]
    fn outside_marker_keeps_its_trailing_space() {
        assert_eq!(outside_marker_text("1. "), "1.\u{a0}");
        assert_eq!(outside_marker_text("一、"), "一、");
    }
}
