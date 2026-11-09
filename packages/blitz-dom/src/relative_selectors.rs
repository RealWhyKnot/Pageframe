use crate::document::BaseDocument;
use crate::stylo::BlitzNode;
use blitz_traits::node_id::NodeId;
use markup5ever::local_name;
use selectors::matching::ElementSelectorFlags;
use style::dom::TElement;
use style::invalidation::element::element_wrapper::{ElementSnapshot, ElementWrapper, Snapshots};
use style::invalidation::element::invalidation_map::TSStateForInvalidation;
use style::invalidation::element::invalidator::{InvalidationResult, SiblingTraversalMap};
use style::invalidation::element::relative_selector::{
    DomMutationOperation, RelativeSelectorInvalidator,
};
use style::invalidation::element::restyle_hints::RestyleHint;
use style::selector_parser::Snapshot;
use style::values::{AtomIdent, GenericAtomIdent};
use style::{Atom, LocalName};
use style_dom::ElementState;

fn mark_invalidated(element: BlitzNode<'_>, _result: &InvalidationResult) {
    element.mark_ancestors_dirty();
}

struct ChangedNames {
    ids: Vec<Atom>,
    classes: Vec<AtomIdent>,
    attrs: Vec<LocalName>,
}

impl ChangedNames {
    fn between(element: BlitzNode<'_>, snapshot: &Snapshot) -> Self {
        let mut ids = Vec::new();
        if snapshot.id_changed() {
            if let Some(id) = snapshot.id_attr() {
                ids.push(id.clone());
            }
            if let Some(id) = element.id() {
                ids.push(Atom::from(id));
            }
        }
        let mut classes = Vec::new();
        if snapshot.class_changed() {
            snapshot.each_class(|class| classes.push(class.clone()));
            element.each_class(|class| classes.push(class.clone()));
        }
        let mut attrs = Vec::new();
        if snapshot.other_attr_changed() {
            snapshot.each_attr_changed(|name| attrs.push(name.clone()));
        }
        Self {
            ids,
            classes,
            attrs,
        }
    }
}

impl BaseDocument {
    pub(crate) fn invalidate_relative_selectors_for_snapshots(&self) {
        if self.snapshots.is_empty() {
            return;
        }
        let snapshots = Snapshots::new(&self.snapshots);
        for opaque in self.snapshots.keys() {
            let Some(node) = u64::try_from(opaque.id())
                .ok()
                .and_then(|id| self.nodes.get(NodeId::from_u64(id)))
            else {
                continue;
            };
            if !node.is_element() || !node.flags.is_in_document() {
                continue;
            }
            let element: BlitzNode<'_> = node;
            let wrapper = ElementWrapper::new(element, &snapshots);
            let Some(snapshot) = wrapper.snapshot() else {
                continue;
            };
            let state_changes = wrapper.state_changes();
            let names = ChangedNames::between(element, snapshot);
            let invalidator = RelativeSelectorInvalidator {
                element,
                quirks_mode: self.stylist.quirks_mode(),
                snapshots: Some(&snapshots),
                invalidated: mark_invalidated,
                sibling_traversal_map: SiblingTraversalMap::default(),
            };
            invalidator.invalidate_relative_selectors_for_this(
                &self.stylist,
                |element, scope, data, quirks_mode, collector| {
                    let map = data.relative_selector_invalidation_map();
                    for name in &names.ids {
                        if let Some(deps) = map.id_to_selector.get(name, quirks_mode) {
                            for dep in deps {
                                collector.add_dependency(dep, *element, scope);
                            }
                        }
                    }
                    for name in &names.classes {
                        if let Some(deps) = map.class_to_selector.get(name, quirks_mode) {
                            for dep in deps {
                                collector.add_dependency(dep, *element, scope);
                            }
                        }
                    }
                    for name in &names.attrs {
                        if let Some(deps) = map.other_attribute_affecting_selectors.get(name) {
                            for dep in deps {
                                collector.add_dependency(dep, *element, scope);
                            }
                        }
                    }
                    if !state_changes.is_empty() {
                        map.state_affecting_selectors.lookup_with_additional(
                            *element,
                            quirks_mode,
                            None,
                            &[],
                            ElementState::empty(),
                            |dependency| {
                                if dependency.state.intersects(state_changes) {
                                    collector.add_dependency(&dependency.dep, *element, scope);
                                }
                                true
                            },
                        );
                    }
                },
            );
        }
    }

    pub(crate) fn invalidate_relative_selectors_for_mutation(
        &self,
        node_id: NodeId,
        operation: DomMutationOperation,
    ) {
        let Some(node) = self.nodes.get(node_id) else {
            return;
        };
        if !node.flags.is_in_document() {
            return;
        }
        if !node.is_element() {
            if let Some(parent_id) = node.parent {
                self.invalidate_relative_selectors_for_emptiness(parent_id);
            }
            return;
        }
        let element: BlitzNode<'_> = node;
        let invalidator = RelativeSelectorInvalidator {
            element,
            quirks_mode: self.stylist.quirks_mode(),
            snapshots: None,
            invalidated: mark_invalidated,
            sibling_traversal_map: SiblingTraversalMap::default(),
        };
        invalidator.invalidate_relative_selectors_for_dom_mutation(
            true,
            &self.stylist,
            ElementSelectorFlags::empty(),
            operation,
        );
    }

    fn invalidate_relative_selectors_for_emptiness(&self, parent_id: NodeId) {
        let Some(parent) = self.nodes.get(parent_id) else {
            return;
        };
        if !parent.is_element() || !parent.flags.is_in_document() {
            return;
        }
        let element: BlitzNode<'_> = parent;
        let invalidator = RelativeSelectorInvalidator {
            element,
            quirks_mode: self.stylist.quirks_mode(),
            snapshots: None,
            invalidated: mark_invalidated,
            sibling_traversal_map: SiblingTraversalMap::default(),
        };
        invalidator.invalidate_relative_selectors_for_this(
            &self.stylist,
            |element, scope, data, quirks_mode, collector| {
                data.relative_invalidation_map_attributes()
                    .ts_state_to_selector
                    .lookup_with_additional(
                        *element,
                        quirks_mode,
                        None,
                        &[],
                        ElementState::empty(),
                        |dependency| {
                            if dependency.state.contains(TSStateForInvalidation::EMPTY) {
                                collector.add_dependency(&dependency.dep, *element, scope);
                            }
                            true
                        },
                    );
            },
        );
    }

    pub(crate) fn restyle_siblings_for_nth_of_attr(
        &mut self,
        node_id: NodeId,
        attr: &markup5ever::LocalName,
        new_value: Option<&str>,
    ) {
        let old_value = self.nodes[node_id].attr(attr.clone());
        let attr_ident = GenericAtomIdent(attr.clone());
        let depends = |value: Option<&str>| {
            if *attr == local_name!("class") {
                value
                    .unwrap_or_default()
                    .split_ascii_whitespace()
                    .any(|class| {
                        let class = Atom::from(class);
                        self.stylist
                            .iter_origins()
                            .any(|(data, _)| data.might_have_nth_of_class_dependency(&class))
                    })
            } else if *attr == local_name!("id") {
                value.is_some_and(|id| {
                    let id = Atom::from(id);
                    self.stylist
                        .iter_origins()
                        .any(|(data, _)| data.might_have_nth_of_id_dependency(&id))
                })
            } else {
                self.stylist
                    .iter_origins()
                    .any(|(data, _)| data.might_have_nth_of_attribute_dependency(&attr_ident))
            }
        };
        if depends(old_value) || depends(new_value) {
            self.restyle_siblings_for_nth_of(node_id);
        }
    }

    pub(crate) fn restyle_siblings_for_nth_of_state(
        &mut self,
        node_id: NodeId,
        state: ElementState,
    ) {
        if self
            .stylist
            .iter_origins()
            .any(|(data, _)| data.has_nth_of_state_dependency(state))
        {
            self.restyle_siblings_for_nth_of(node_id);
        }
    }

    fn restyle_siblings_for_nth_of(&mut self, node_id: NodeId) {
        let Some(parent_id) = self.nodes[node_id].parent else {
            return;
        };
        let parent = &self.nodes[parent_id];
        if !parent
            .selector_flags()
            .get()
            .contains(ElementSelectorFlags::HAS_SLOW_SELECTOR_NTH_OF)
        {
            return;
        }
        let siblings = parent.children.clone();
        for sibling in siblings {
            if sibling != node_id && self.nodes[sibling].is_element() {
                self.nodes[sibling].set_restyle_hint(RestyleHint::RESTYLE_SELF);
            }
        }
    }
}
