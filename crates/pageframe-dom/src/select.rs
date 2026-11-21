use markup5ever::local_name;
use pageframe_traits::node_id::NodeId;
use style_dom::ElementState;

use crate::BaseDocument;
use crate::traversal::TreeTraverser;

impl BaseDocument {
    fn has_tag(&self, node_id: NodeId, name: &markup5ever::LocalName) -> bool {
        self.get_node(node_id)
            .is_some_and(|node| node.data.is_element_with_tag_name(name))
    }

    pub fn is_select(&self, node_id: NodeId) -> bool {
        self.has_tag(node_id, &local_name!("select"))
    }

    pub fn is_option(&self, node_id: NodeId) -> bool {
        self.has_tag(node_id, &local_name!("option"))
    }

    pub fn selected_index(&self, select: NodeId) -> Option<usize> {
        self.select_options(select)
            .into_iter()
            .position(|option| self.option_is_selected(option))
    }

    pub fn set_selected_index(&mut self, select: NodeId, index: Option<usize>) {
        let options = self.select_options(select);
        for (position, option) in options.into_iter().enumerate() {
            self.set_option_selected(option, Some(position) == index);
        }
    }

    pub fn select_options(&self, select: NodeId) -> Vec<NodeId> {
        let mut options = Vec::new();
        let Some(node) = self.get_node(select) else {
            return options;
        };
        for &child in &node.children {
            if self.has_tag(child, &local_name!("option")) {
                options.push(child);
            } else if self.has_tag(child, &local_name!("optgroup")) {
                let grouped = self.nodes[child].children.iter().copied();
                options.extend(grouped.filter(|&id| self.has_tag(id, &local_name!("option"))));
            }
        }
        options
    }

    pub(crate) fn option_select(&self, option: NodeId) -> Option<NodeId> {
        if !self.has_tag(option, &local_name!("option")) {
            return None;
        }
        let parent = self.get_node(option)?.parent?;
        if self.is_select(parent) {
            return Some(parent);
        }
        let outer = self.get_node(parent)?.parent?;
        (self.has_tag(parent, &local_name!("optgroup")) && self.is_select(outer)).then_some(outer)
    }

    pub(crate) fn is_menulist_item(&self, node_id: NodeId) -> bool {
        let select = if self.is_option(node_id) {
            self.option_select(node_id)
        } else {
            let parent = self.get_node(node_id).and_then(|node| node.parent);
            match parent {
                Some(parent)
                    if self.is_select(parent) && self.nodes[parent].after() == Some(node_id) =>
                {
                    Some(parent)
                }
                _ => return false,
            }
        };
        !select.is_some_and(|select| self.select_is_list(select))
    }

    fn option_is_selected(&self, option: NodeId) -> bool {
        self.nodes[option]
            .element_data()
            .is_some_and(|element| element.element_state.contains(ElementState::CHECKED))
    }

    pub(crate) fn option_is_disabled(&self, option: NodeId) -> bool {
        let disabled = |id: NodeId| self.nodes[id].attr(local_name!("disabled")).is_some();
        disabled(option)
            || self.nodes[option].parent.is_some_and(|parent| {
                self.has_tag(parent, &local_name!("optgroup")) && disabled(parent)
            })
    }

    pub fn option_value(&self, option: NodeId) -> String {
        let node = &self.nodes[option];
        match node.attr(local_name!("value")) {
            Some(value) => value.to_owned(),
            None => node
                .text_content()
                .split_ascii_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        }
    }

    pub(crate) fn selected_options(&self, select: NodeId) -> Vec<NodeId> {
        let mut options = self.select_options(select);
        options.retain(|&option| self.option_is_selected(option));
        options
    }

    pub(crate) fn selected_option(&self, select: NodeId) -> Option<NodeId> {
        self.selected_options(select).first().copied()
    }

    pub fn select_value(&self, select: NodeId) -> Option<String> {
        self.selected_option(select)
            .map(|option| self.option_value(option))
    }

    pub(crate) fn set_option_selected(&mut self, option: NodeId, selected: bool) {
        if self.option_is_selected(option) == selected {
            return;
        }
        self.snapshot_node_and(option, ElementState::CHECKED, |node| {
            if let Some(element) = node.element_data_mut() {
                element.element_state.set(ElementState::CHECKED, selected);
            }
            node.mark_ancestors_dirty();
        });
    }

    pub(crate) fn set_selected_option(&mut self, select: NodeId, option: NodeId) {
        let single = self.nodes[select].attr(local_name!("multiple")).is_none();
        self.pick_option(select, option, single);
    }

    pub(crate) fn pick_option(&mut self, select: NodeId, option: NodeId, single: bool) {
        let options = self.select_options(select);
        if !options.contains(&option) {
            return;
        }
        for other in options {
            if other == option {
                self.set_option_selected(other, true);
            } else if single {
                self.set_option_selected(other, false);
            }
        }
    }

    pub fn set_select_value(&mut self, select: NodeId, value: &str) -> bool {
        let options = self.select_options(select);
        let found = options
            .iter()
            .copied()
            .find(|&option| self.option_value(option) == value);
        for other in options {
            self.set_option_selected(other, Some(other) == found);
        }
        found.is_some()
    }

    pub(crate) fn reset_select(&mut self, select: NodeId) {
        let list = self.select_is_list(select);
        if self.nodes[select].attr(local_name!("multiple")).is_some() {
            return;
        }
        let options = self.select_options(select);
        let keep = options
            .iter()
            .rev()
            .copied()
            .find(|&option| self.option_is_selected(option))
            .or_else(|| {
                options
                    .iter()
                    .copied()
                    .find(|&option| !list && !self.option_is_disabled(option))
            });
        for option in options {
            self.set_option_selected(option, Some(option) == keep);
        }
    }

    pub(crate) fn reset_selects(&mut self) {
        let selects: Vec<NodeId> = TreeTraverser::new(self)
            .filter(|&id| self.is_select(id))
            .collect();
        for select in selects {
            self.reset_select(select);
        }
    }
}
