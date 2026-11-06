use blitz_dom::{BaseDocument, LocalName, QualName, ns};
use blitz_traits::NodeId;
use style_dom::ElementState;

pub struct Ui<'d> {
    doc: &'d mut BaseDocument,
}

impl<'d> Ui<'d> {
    pub(crate) fn new(doc: &'d mut BaseDocument) -> Self {
        Self { doc }
    }

    pub fn find(&self, selector: &str) -> Option<NodeId> {
        match self.doc.query_selector(selector) {
            Ok(found) => found,
            Err(error) => {
                tracing::error!(selector, ?error, "invalid selector");
                None
            }
        }
    }

    pub fn find_all(&self, selector: &str) -> Vec<NodeId> {
        match self.doc.query_selector_all(selector) {
            Ok(found) => found.into_vec(),
            Err(error) => {
                tracing::error!(selector, ?error, "invalid selector");
                Vec::new()
            }
        }
    }

    pub fn find_in(&self, scope: NodeId, selector: &str) -> Option<NodeId> {
        self.doc.get_node(scope)?;
        match self.doc.query_selector_in(scope, selector) {
            Ok(found) => found,
            Err(error) => {
                tracing::error!(selector, ?error, "invalid selector");
                None
            }
        }
    }

    pub fn closest(&self, node: NodeId, selector: &str) -> Option<NodeId> {
        self.doc.get_node(node)?;
        match self.doc.closest(node, selector) {
            Ok(found) => found,
            Err(error) => {
                tracing::error!(selector, ?error, "invalid selector");
                None
            }
        }
    }

    pub fn parent(&self, node: NodeId) -> Option<NodeId> {
        self.doc.get_node(node)?.parent
    }

    pub fn children(&self, node: NodeId) -> Vec<NodeId> {
        self.doc
            .get_node(node)
            .map(|n| {
                n.children
                    .iter()
                    .copied()
                    .filter(|&child| self.doc.get_node(child).is_some_and(|c| c.is_element()))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn select(&mut self, selector: &str) -> Selection<'_> {
        let nodes = self.find_all(selector);
        Selection {
            doc: self.doc,
            nodes,
        }
    }

    pub fn node(&mut self, node: NodeId) -> Selection<'_> {
        let nodes = if self.doc.get_node(node).is_some() {
            vec![node]
        } else {
            Vec::new()
        };
        Selection {
            doc: self.doc,
            nodes,
        }
    }

    pub fn instantiate(&mut self, template_selector: &str) -> Vec<NodeId> {
        let Some(template) = self.find(template_selector) else {
            return Vec::new();
        };
        let mut mutator = self.doc.mutate();
        let Some(content) = mutator.try_template_contents(template) else {
            tracing::error!(template_selector, "not a <template> element");
            return Vec::new();
        };
        let children = mutator.child_ids(content);
        children
            .iter()
            .map(|&child| mutator.deep_clone_node(child))
            .collect()
    }
}

pub struct Selection<'d> {
    doc: &'d mut BaseDocument,
    nodes: Vec<NodeId>,
}

fn qual(name: &str) -> QualName {
    QualName::new(None, ns!(), LocalName::from(name))
}

impl Selection<'_> {
    pub fn ids(&self) -> &[NodeId] {
        &self.nodes
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    fn first(&self) -> Option<&blitz_dom::Node> {
        self.doc.get_node(*self.nodes.first()?)
    }

    pub fn text(&self) -> Option<String> {
        self.first().map(blitz_dom::Node::text_content)
    }

    pub fn attr(&self, name: &str) -> Option<String> {
        self.first()?.attr(LocalName::from(name)).map(str::to_owned)
    }

    pub fn value(&self) -> Option<String> {
        let element = self.first()?.element_data()?;
        match element.text_input_data() {
            Some(input) => Some(input.value().to_owned()),
            None => element.attr(LocalName::from("value")).map(str::to_owned),
        }
    }

    pub fn checked(&self) -> Option<bool> {
        self.first()?.element_data()?.checkbox_input_checked()
    }

    pub fn has_class(&self, class: &str) -> bool {
        self.attr("class")
            .is_some_and(|classes| classes.split_ascii_whitespace().any(|c| c == class))
    }

    pub fn set_text(&mut self, text: &str) -> &mut Self {
        let targets: Vec<(NodeId, Option<NodeId>)> = self
            .nodes
            .iter()
            .map(|&node| (node, self.only_text_child(node)))
            .collect();
        let mut mutator = self.doc.mutate();
        for (node, text_child) in targets {
            if let Some(child) = text_child {
                mutator.set_node_text(child, text);
            } else {
                mutator.remove_and_drop_all_children(node);
                let child = mutator.create_text_node(text);
                mutator.append_children(node, &[child]);
            }
        }
        drop(mutator);
        self
    }

    fn only_text_child(&self, node: NodeId) -> Option<NodeId> {
        match self.doc.get_node(node)?.children.as_slice() {
            [child] if self.doc.get_node(*child)?.is_text_node() => Some(*child),
            _ => None,
        }
    }

    pub fn set_html(&mut self, html: &str) -> &mut Self {
        let mut mutator = self.doc.mutate();
        for &node in &self.nodes {
            mutator.set_inner_html(node, html);
        }
        drop(mutator);
        self
    }

    pub fn set_attr(&mut self, name: &str, value: &str) -> &mut Self {
        let mut mutator = self.doc.mutate();
        for &node in &self.nodes {
            mutator.set_attribute(node, qual(name), value);
        }
        drop(mutator);
        self
    }

    pub fn remove_attr(&mut self, name: &str) -> &mut Self {
        let mut mutator = self.doc.mutate();
        for &node in &self.nodes {
            mutator.clear_attribute(node, qual(name));
        }
        drop(mutator);
        self
    }

    pub fn set_value(&mut self, value: &str) -> &mut Self {
        self.set_attr("value", value)
    }

    pub fn set_checked(&mut self, checked: bool) -> &mut Self {
        for &node in &self.nodes {
            self.doc
                .snapshot_node_and(node, ElementState::CHECKED, |n| {
                    if let Some(element) = n.element_data_mut() {
                        element.set_checkbox_input_checked(checked);
                    }
                    n.mark_ancestors_dirty();
                });
        }
        self.doc.shell_provider.request_redraw();
        self
    }

    pub fn set_style(&mut self, property: &str, value: &str) -> &mut Self {
        let mut mutator = self.doc.mutate();
        for &node in &self.nodes {
            mutator.set_style_property(node, property, value);
        }
        drop(mutator);
        self
    }

    pub fn remove_style(&mut self, property: &str) -> &mut Self {
        let mut mutator = self.doc.mutate();
        for &node in &self.nodes {
            mutator.remove_style_property(node, property);
        }
        drop(mutator);
        self
    }

    pub fn add_class(&mut self, class: &str) -> &mut Self {
        self.edit_classes(|classes| {
            if !classes.iter().any(|c| c == class) {
                classes.push(class.to_owned());
            }
        })
    }

    pub fn remove_class(&mut self, class: &str) -> &mut Self {
        self.edit_classes(|classes| classes.retain(|c| c != class))
    }

    pub fn set_class(&mut self, class: &str, enabled: bool) -> &mut Self {
        if enabled {
            self.add_class(class)
        } else {
            self.remove_class(class)
        }
    }

    fn edit_classes(&mut self, edit: impl Fn(&mut Vec<String>)) -> &mut Self {
        let updates: Vec<(NodeId, String)> = self
            .nodes
            .iter()
            .filter_map(|&node| {
                let current = self.doc.get_node(node)?.attr(LocalName::from("class"));
                let mut classes: Vec<String> = current
                    .unwrap_or_default()
                    .split_ascii_whitespace()
                    .map(str::to_owned)
                    .collect();
                edit(&mut classes);
                let joined = classes.join(" ");
                (current != Some(joined.as_str())).then_some((node, joined))
            })
            .collect();
        let mut mutator = self.doc.mutate();
        for (node, classes) in &updates {
            mutator.set_attribute(*node, qual("class"), classes);
        }
        drop(mutator);
        self
    }

    pub fn append(&mut self, children: &[NodeId]) -> &mut Self {
        if let Some(&parent) = self.nodes.first() {
            self.doc.mutate().append_children(parent, children);
        }
        self
    }

    pub fn clear(&mut self) -> &mut Self {
        let mut mutator = self.doc.mutate();
        for &node in &self.nodes {
            mutator.remove_and_drop_all_children(node);
        }
        drop(mutator);
        self
    }

    pub fn remove(&mut self) {
        let mut mutator = self.doc.mutate();
        for &node in &self.nodes {
            mutator.remove_and_drop_node(node);
        }
        drop(mutator);
        self.nodes.clear();
    }
}
