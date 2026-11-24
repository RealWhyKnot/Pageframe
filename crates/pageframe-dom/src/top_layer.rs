use num_traits::ToPrimitive;
use pageframe_traits::events::HitResult;
use pageframe_traits::node_id::NodeId;
use std::collections::HashMap;
use style::properties::ComputedValues;
use style::selector_parser::PseudoElement;
use style::servo_arc::Arc as ServoArc;
use style::shared_lock::StylesheetGuards;
use style::stylist::RuleInclusion;
use style_dom::ElementState;
use taffy::{
    AvailableSpace, AxisStaticEdge, AxisStaticPosition, ContainingBlockClaims, CoreStyle,
    LayoutPartialTree, OofCandidate, OofPositioningArea, RoundTree,
};

use crate::BaseDocument;
use crate::layout::LayoutPassState;
use crate::layout::damage::ALL_DAMAGE;
use crate::node::{Node, NodeFlags, OverflowControl};
use crate::{dom_node_id, taffy_node_id};

impl BaseDocument {
    pub fn top_layer(&self) -> &[NodeId] {
        &self.top_layer
    }

    pub fn active_modal(&self) -> Option<NodeId> {
        self.top_layer.last().copied()
    }

    pub fn is_in_top_layer(&self, node_id: NodeId) -> bool {
        self.nodes
            .get(node_id)
            .is_some_and(|node| node.flags.is_in_top_layer())
    }

    pub fn set_dialog_modal(&mut self, node_id: NodeId, modal: bool) -> bool {
        if !modal {
            return self.remove_from_top_layer(node_id);
        }
        let Some(node) = self.nodes.get(node_id) else {
            return false;
        };
        if !node.is_element() || !node.flags.is_in_document() {
            return false;
        }
        self.top_layer.retain(|&id| id != node_id);
        self.top_layer.push(node_id);
        match self.focus_node_id {
            Some(focus) if focus != node_id => {
                self.modal_openers.insert(node_id, focus);
            }
            _ => {
                self.modal_openers.remove(&node_id);
            }
        }
        self.nodes[node_id].flags.insert(NodeFlags::IN_TOP_LAYER);
        self.set_modal_state(node_id, true);
        true
    }

    pub(crate) fn remove_from_top_layer(&mut self, node_id: NodeId) -> bool {
        let Some(index) = self.top_layer.iter().position(|&id| id == node_id) else {
            return false;
        };
        self.top_layer.remove(index);
        self.backdrop_styles.remove(&node_id);
        self.modal_openers.remove(&node_id);
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.flags.remove(NodeFlags::IN_TOP_LAYER);
        }
        self.unhoist(node_id);
        self.set_modal_state(node_id, false);
        true
    }

    fn unhoist(&mut self, node_id: NodeId) {
        let Some(containing_block) = self
            .nodes
            .get(node_id)
            .and_then(|node| node.oof_containing_block.take())
        else {
            return;
        };
        if let Some(containing_block) = self.nodes.get_mut(containing_block) {
            containing_block
                .hoisted_children
                .borrow_mut()
                .retain(|&child| child != node_id);
            containing_block.insert_damage(ALL_DAMAGE);
            containing_block.mark_ancestors_dirty();
        }
    }

    pub(crate) fn remove_detached_modal(&mut self, node_id: NodeId) -> Option<NodeId> {
        let opener = self.modal_openers.get(&node_id).copied();
        self.remove_from_top_layer(node_id)
            .then_some(opener)
            .flatten()
    }

    pub(crate) fn restore_modal_opener(&mut self, opener: NodeId) {
        let focus_lost = self.focus_node_id.is_none_or(|id| {
            self.nodes
                .get(id)
                .is_none_or(|node| !node.flags.is_in_document())
        });
        let usable = self.nodes.get(opener).is_some_and(|node| {
            node.flags.is_in_document() && node.is_element() && node.is_focussable()
        });
        if focus_lost && usable {
            self.set_focus_to(opener);
        }
    }

    fn set_modal_state(&mut self, node_id: NodeId, modal: bool) {
        let Some(node) = self.nodes.get(node_id) else {
            return;
        };
        let in_document = node.flags.is_in_document();
        let parent = node.parent;
        let apply = |node: &mut Node| {
            if let Some(element) = node.element_data_mut() {
                element.element_state.set(ElementState::MODAL, modal);
            }
            node.insert_damage(ALL_DAMAGE);
            node.mark_ancestors_dirty();
        };
        if in_document {
            self.snapshot_node_and(node_id, ElementState::MODAL, apply);
            if let Some(parent) = parent.and_then(|id| self.nodes.get_mut(id)) {
                parent.insert_damage(ALL_DAMAGE);
            }
        } else {
            apply(&mut self.nodes[node_id]);
        }
        self.shell_provider.request_redraw();
    }

    pub fn is_inert(&self, node_id: NodeId) -> bool {
        self.active_modal()
            .is_some_and(|modal| !self.is_inclusive_ancestor(modal, node_id))
    }

    pub(crate) fn is_sequentially_focusable(&self, node: &Node) -> bool {
        node.is_focussable()
            && !self.is_inert(node.id)
            && node
                .primary_styles()
                .is_some_and(|style| !style.get_box().display.is_none())
    }

    pub(crate) fn has_focus_in_modal(&self) -> bool {
        self.active_modal().is_some() && self.focus_node_id.is_some_and(|id| !self.is_inert(id))
    }

    fn is_inclusive_ancestor(&self, ancestor: NodeId, node_id: NodeId) -> bool {
        let mut current = Some(node_id);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.nodes.get(id).and_then(|node| node.parent);
        }
        false
    }

    pub fn backdrop_style(&self, node_id: NodeId) -> Option<&ServoArc<ComputedValues>> {
        self.backdrop_styles.get(&node_id)
    }

    pub fn top_layer_origin(&self, node_id: NodeId) -> taffy::Point<f32> {
        let Some(node) = self.nodes.get(node_id) else {
            return taffy::Point::ZERO;
        };
        let Some(containing_block) = node.containing_block().and_then(|id| self.nodes.get(id))
        else {
            return taffy::Point::ZERO;
        };
        let origin = containing_block.absolute_position(0.0, 0.0);
        let is_viewport_fixed = node.taffy_position() == taffy::Position::Fixed
            && containing_block.containing_block().is_none();
        let shift = if is_viewport_fixed {
            *containing_block.scroll_offset()
        } else {
            let scroll = self.viewport_scroll();
            crate::Point {
                x: -scroll.x,
                y: -scroll.y,
            }
        };
        taffy::Point {
            x: origin.x + shift.x.to_f32().unwrap_or(0.0),
            y: origin.y + shift.y.to_f32().unwrap_or(0.0),
        }
    }

    pub(crate) fn hit_active_modal(
        &self,
        x: f32,
        y: f32,
        control: &mut Option<OverflowControl>,
    ) -> Option<HitResult> {
        let modal = self.active_modal()?;
        let node = self.nodes.get(modal)?;
        let scroll = self.viewport_scroll();
        let origin = self.top_layer_origin(modal);
        let local_x = x - scroll.x.to_f32().unwrap_or(0.0) - origin.x;
        let local_y = y - scroll.y.to_f32().unwrap_or(0.0) - origin.y;
        node.hit_inner(local_x, local_y, self.viewport().scale_f64(), control)
            .or_else(|| {
                let location = node.final_layout().location;
                Some(HitResult {
                    node_id: modal,
                    x: local_x - location.x,
                    y: local_y - location.y,
                    is_text: false,
                })
            })
    }

    pub(crate) fn refresh_top_layer(&mut self) {
        let stale: Vec<NodeId> = self
            .top_layer
            .iter()
            .copied()
            .filter(|&id| {
                self.nodes
                    .get(id)
                    .is_none_or(|node| !node.flags.is_in_document())
            })
            .collect();
        for id in stale {
            self.remove_from_top_layer(id);
        }
        let mut styles = HashMap::new();
        {
            let guard = self.guard.read();
            for &id in &self.top_layer {
                let node = &self.nodes[id];
                let Some(primary) = node.primary_styles() else {
                    continue;
                };
                let backdrop = self.stylist.lazily_compute_pseudo_element_style(
                    &StylesheetGuards::same(&guard),
                    node,
                    &PseudoElement::Backdrop,
                    RuleInclusion::All,
                    &primary,
                    true,
                    None,
                );
                if let Some(backdrop) = backdrop {
                    styles.insert(id, backdrop);
                }
            }
        }
        self.backdrop_styles = styles;
    }
}

pub(crate) fn place_top_layer_in_viewport(
    state: &mut LayoutPassState<'_>,
    root: taffy::NodeId,
    available_space: taffy::Size<AvailableSpace>,
) {
    let root_id = dom_node_id(root);
    let pending: Vec<NodeId> = state
        .top_layer
        .iter()
        .copied()
        .filter(|&id| {
            state.nodes.get(id).is_some_and(|node| {
                node.taffy_position() == taffy::Position::Fixed
                    && node
                        .oof_containing_block
                        .get()
                        .is_some_and(|containing_block| containing_block != root_id)
            })
        })
        .collect();
    if pending.is_empty() {
        return;
    }

    let layout = state.get_unrounded_layout(root);
    let direction = state.get_core_container_style(root).direction();
    let icb = available_space.into_options();
    let inset_width = layout.border.left + layout.border.right + layout.scrollbar_size.width;
    let inset_height = layout.border.top + layout.border.bottom + layout.scrollbar_size.height;
    let (width, x) = match icb.width {
        Some(width) => (
            (width - layout.scrollbar_size.width).max(0.0),
            -layout.location.x,
        ),
        None => (layout.size.width - inset_width, layout.border.left),
    };
    let (height, y) = match icb.height {
        Some(height) => (
            (height - layout.scrollbar_size.height).max(0.0),
            -layout.location.y,
        ),
        None => (layout.size.height - inset_height, layout.border.top),
    };
    let area = OofPositioningArea {
        size: taffy::Size { width, height },
        offset: taffy::Point { x, y },
    };

    for id in pending {
        let node = taffy_node_id(id);
        let candidate = OofCandidate {
            node,
            order: state.get_unrounded_layout(node).order,
            position: taffy::Position::Fixed,
            static_position: taffy::Point {
                x: AxisStaticPosition::from_edge(0.0, AxisStaticEdge::Start),
                y: AxisStaticPosition::from_edge(0.0, AxisStaticEdge::Start),
            },
        };
        taffy::compute_oof_layout_for_area(
            &mut *state,
            root,
            &[candidate],
            area,
            direction,
            ContainingBlockClaims::ALL,
        );
        if let Some(previous) = state.nodes[id].oof_containing_block.get()
            && let Some(previous) = state.nodes.get(previous)
        {
            previous
                .hoisted_children
                .borrow_mut()
                .retain(|&child| child != id);
        }
        let root_node = &state.nodes[root_id];
        let mut hoisted = root_node.hoisted_children.borrow_mut();
        if !hoisted.contains(&id) {
            hoisted.push(id);
        }
        drop(hoisted);
        state.nodes[id].oof_containing_block.set(Some(root_id));
    }
}
