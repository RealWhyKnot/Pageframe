use markup5ever::local_name;
use pageframe_traits::node_id::NodeId;
use style_dom::ElementState;

use crate::{BaseDocument, Node};

impl BaseDocument {
    pub(crate) fn refresh_disabled(&mut self, root: NodeId) {
        let mut root_blocked = None;
        let mut pending = vec![(root, None)];
        while let Some((node_id, inherited)) = pending.pop() {
            let node = &self.nodes[node_id];
            let children = node.children.clone();
            let Some(element) = node.element_data() else {
                pending.extend(children.into_iter().map(|child| (child, inherited)));
                continue;
            };
            let has_attribute = element.has_attr(local_name!("disabled"));
            let blocks_children = has_attribute && element.name.local == local_name!("fieldset");
            let can_be_disabled = element.can_be_disabled();
            let blocked = match inherited {
                None if can_be_disabled => {
                    Some(*root_blocked.get_or_insert_with(|| self.disabled_by_fieldset(root)))
                }
                known => known,
            };
            if can_be_disabled {
                self.set_disabled_state(node_id, has_attribute || blocked == Some(true));
            }
            let legend = blocks_children
                .then(|| self.first_legend(node_id))
                .flatten();
            pending.extend(children.into_iter().map(|child| {
                match blocks_children && Some(child) != legend {
                    true => (child, Some(true)),
                    false => (child, blocked),
                }
            }));
        }
    }

    fn disabled_by_fieldset(&self, node_id: NodeId) -> bool {
        let mut child = node_id;
        while let Some(parent) = self.nodes[child].parent {
            let blocks = self.nodes[parent].element_data().is_some_and(|element| {
                element.name.local == local_name!("fieldset")
                    && element.has_attr(local_name!("disabled"))
            });
            if blocks && self.first_legend(parent) != Some(child) {
                return true;
            }
            child = parent;
        }
        false
    }

    fn first_legend(&self, fieldset: NodeId) -> Option<NodeId> {
        self.nodes[fieldset]
            .children
            .iter()
            .copied()
            .find(|&child| {
                self.nodes[child]
                    .data
                    .is_element_with_tag_name(&local_name!("legend"))
            })
    }

    fn set_disabled_state(&mut self, node_id: NodeId, disabled: bool) {
        let node = &self.nodes[node_id];
        let unchanged = node.element_data().is_none_or(|element| {
            element.element_state.contains(ElementState::DISABLED) == disabled
        });
        if unchanged {
            return;
        }
        let apply = |node: &mut Node| {
            if let Some(element) = node.element_data_mut() {
                element.element_state.set(ElementState::DISABLED, disabled);
                element.element_state.set(ElementState::ENABLED, !disabled);
                element.flush_is_focussable();
            }
            node.mark_ancestors_dirty();
        };
        match node.flags.is_in_document() {
            true => self.snapshot_node_and(
                node_id,
                ElementState::DISABLED | ElementState::ENABLED,
                apply,
            ),
            false => apply(&mut self.nodes[node_id]),
        }
    }
}
