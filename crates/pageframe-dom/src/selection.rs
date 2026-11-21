//! Text selection state and logic for non-input elements.
//!
//! This module handles text selection across inline roots in the document,
//! including support for anonymous blocks which have unstable IDs across
//! layout reconstruction.

use pageframe_traits::node_id::NodeId;
use std::collections::HashSet;

/// Represents one endpoint (anchor or focus) of a text selection.
///
/// For regular nodes, `node_or_parent` contains the node ID directly.
/// For anonymous blocks, `node_or_parent` contains the parent ID and
/// `sibling_index` contains the index among anonymous siblings.
#[derive(Clone, Debug, Default)]
pub struct SelectionEndpoint {
    /// For regular nodes: the node ID directly.
    /// For anonymous blocks: the parent ID (requires lookup via sibling_index).
    pub(crate) node_or_parent: Option<NodeId>,
    /// Byte offset within the inline root's text
    pub offset: usize,
    /// For anonymous blocks only: index among anonymous siblings.
    /// When Some, node_or_parent is a parent ID requiring lookup.
    pub(crate) sibling_index: Option<usize>,
}

impl SelectionEndpoint {
    /// Create a new endpoint at the given node and offset (for non-anonymous nodes)
    fn new(node: NodeId, offset: usize) -> Self {
        Self {
            node_or_parent: Some(node),
            offset,
            sibling_index: None,
        }
    }

    /// Check if this endpoint is set
    pub fn is_some(&self) -> bool {
        self.node_or_parent.is_some()
    }

    /// Clear this endpoint
    pub fn clear(&mut self) {
        self.node_or_parent = None;
        self.offset = 0;
        self.sibling_index = None;
    }

    /// Resolve the node ID, using a lookup function for anonymous blocks.
    pub fn resolve_node_id(
        &self,
        lookup_fn: impl FnOnce(NodeId, usize) -> Option<NodeId>,
    ) -> Option<NodeId> {
        match (self.node_or_parent, self.sibling_index) {
            (Some(node), None) => Some(node),
            (Some(parent), Some(idx)) => lookup_fn(parent, idx),
            _ => None,
        }
    }

    /// Set as a direct node reference (for regular nodes)
    pub fn set_node(&mut self, node: NodeId, offset: usize) {
        self.node_or_parent = Some(node);
        self.offset = offset;
        self.sibling_index = None;
    }

    /// Set as an anonymous block reference
    pub fn set_anonymous(&mut self, parent: NodeId, sibling_index: usize, offset: usize) {
        self.node_or_parent = Some(parent);
        self.offset = offset;
        self.sibling_index = Some(sibling_index);
    }
}

/// Text selection state for non-input elements.
///
/// Tracks both the anchor (where selection started) and focus (where it currently ends).
/// For anonymous blocks, we store stable parent references since anonymous block IDs
/// can change during layout reconstruction.
#[derive(Clone, Debug, Default)]
pub struct TextSelection {
    /// The anchor point (where selection started via mousedown)
    pub anchor: SelectionEndpoint,
    /// The focus point (where selection currently ends, updated during drag)
    pub focus: SelectionEndpoint,
}

impl TextSelection {
    /// Create a selection spanning from anchor to focus
    pub fn new(
        anchor_node: NodeId,
        anchor_offset: usize,
        focus_node: NodeId,
        focus_offset: usize,
    ) -> Self {
        Self {
            anchor: SelectionEndpoint::new(anchor_node, anchor_offset),
            focus: SelectionEndpoint::new(focus_node, focus_offset),
        }
    }

    /// Check if there is an active (non-empty) selection.
    /// Note: For anonymous blocks this compares parent+sibling_index, not resolved node IDs.
    pub fn is_active(&self) -> bool {
        self.anchor.is_some()
            && self.focus.is_some()
            && (self.anchor.node_or_parent != self.focus.node_or_parent
                || self.anchor.sibling_index != self.focus.sibling_index
                || self.anchor.offset != self.focus.offset)
    }

    /// Clear the selection
    pub fn clear(&mut self) {
        self.anchor.clear();
        self.focus.clear();
    }

    /// Update the focus endpoint (for non-anonymous nodes)
    pub fn set_focus(&mut self, node: NodeId, offset: usize) {
        self.focus.set_node(node, offset);
    }
}

struct EndpointPath {
    node: NodeId,
    offset: usize,
    chain: HashSet<NodeId>,
    boxes: Vec<(NodeId, usize, NodeId)>,
    key: Vec<usize>,
}

impl crate::BaseDocument {
    fn inline_text_len(&self, node: NodeId) -> Option<usize> {
        self.get_node(node)
            .and_then(|node| node.element_data())
            .and_then(|data| data.inline_layout_data.as_ref())
            .map(|inline_layout| inline_layout.text.len())
    }

    fn endpoint_path(&self, point: &SelectionEndpoint) -> Option<EndpointPath> {
        let lookup = |parent_id, idx| self.find_anonymous_block_by_index(parent_id, idx);
        let node = point.resolve_node_id(lookup)?;
        let clamped = point.offset.min(self.inline_text_len(node).unwrap_or(0));
        let mut chain = HashSet::from([node]);
        let mut boxes = Vec::new();
        let mut key = vec![clamped * 2];
        let mut child = node;
        while let Some(parent) = self
            .get_node(child)
            .and_then(|node| node.layout_parent.get().or(node.parent))
            .filter(|parent| !chain.contains(parent))
        {
            let parent_node = self.get_node(parent)?;
            let inline_box = parent_node
                .element_data()
                .and_then(|data| data.inline_layout_data.as_ref())
                .and_then(|inline_layout| {
                    inline_layout.layout.inline_boxes().find(|inline_box| {
                        inline_box.kind == parley::InlineBoxKind::InFlow
                            && chain.contains(&NodeId::from_u64(inline_box.id))
                    })
                })
                .map(|inline_box| (inline_box.index, NodeId::from_u64(inline_box.id)));
            if let Some((index, box_id)) = inline_box {
                boxes.push((parent, index, box_id));
                key.push(index * 2 + 1);
            } else {
                let position = parent_node
                    .layout_children
                    .borrow()
                    .as_ref()
                    .and_then(|children| children.iter().position(|&id| id == child))
                    .or_else(|| parent_node.children.iter().position(|&id| id == child))
                    .unwrap_or(0);
                key.push(position);
            }
            chain.insert(parent);
            child = parent;
        }
        key.reverse();
        Some(EndpointPath {
            node,
            offset: point.offset,
            chain,
            boxes,
            key,
        })
    }

    fn clip_nested_root_ranges(
        &self,
        ranges: &mut Vec<(NodeId, usize, usize)>,
        first: &EndpointPath,
        last: &EndpointPath,
    ) -> Vec<NodeId> {
        let roots = [first.node, last.node]
            .into_iter()
            .chain(first.boxes.iter().chain(&last.boxes).map(|entry| entry.0));
        for root in roots {
            if ranges.iter().all(|&(id, ..)| id != root)
                && let Some(text_len) = self.inline_text_len(root)
            {
                ranges.push((root, 0, text_len));
            }
        }
        let mut holding = Vec::new();
        for (root, start, end) in ranges.iter_mut() {
            let first_box = first.boxes.iter().find(|entry| entry.0 == *root);
            let last_box = last.boxes.iter().find(|entry| entry.0 == *root);
            if *root == first.node {
                *start = first.offset;
            } else if let Some(&(_, index, box_id)) = first_box {
                *start = index;
                holding.push(box_id);
            } else if *root == last.node || last_box.is_some() {
                *start = 0;
            }
            if *root == last.node {
                *end = last.offset;
            } else if let Some(&(_, index, box_id)) = last_box {
                *end = index;
                holding.push(box_id);
            } else if *root == first.node || first_box.is_some() {
                *end = self.inline_text_len(*root).unwrap_or(*end);
            }
        }
        holding.dedup();
        holding
    }

    fn atomic_children(
        &self,
        id: NodeId,
        first: &EndpointPath,
        last: &EndpointPath,
    ) -> Vec<NodeId> {
        let Some(node) = self.get_node(id) else {
            return Vec::new();
        };
        let children = node.layout_children.borrow();
        let Some(children) = children.as_ref() else {
            return Vec::new();
        };
        let from = first
            .chain
            .contains(&id)
            .then(|| {
                children
                    .iter()
                    .position(|child| first.chain.contains(child))
            })
            .flatten()
            .unwrap_or(0);
        let to = last
            .chain
            .contains(&id)
            .then(|| children.iter().position(|child| last.chain.contains(child)))
            .flatten()
            .map_or(children.len(), |to| to + 1);
        children.iter().take(to).skip(from).rev().copied().collect()
    }

    pub(crate) fn with_atomic_contents(
        &self,
        mut ranges: Vec<(NodeId, usize, usize)>,
    ) -> Vec<(NodeId, usize, usize)> {
        if ranges.is_empty() {
            return ranges;
        }
        let (Some(anchor), Some(focus)) = (
            self.endpoint_path(&self.text_selection.anchor),
            self.endpoint_path(&self.text_selection.focus),
        ) else {
            return ranges;
        };
        let (first, last) = if anchor.key <= focus.key {
            (anchor, focus)
        } else {
            (focus, anchor)
        };
        let mut pending = self.clip_nested_root_ranges(&mut ranges, &first, &last);
        let mut seen: HashSet<NodeId> = ranges.iter().map(|&(root, ..)| root).collect();
        let mut at = 0;
        loop {
            while let Some(id) = pending.pop() {
                let holds = first.chain.contains(&id) || last.chain.contains(&id);
                match self.inline_text_len(id) {
                    Some(text_len) => {
                        if text_len > 0 && !holds && seen.insert(id) {
                            ranges.push((id, 0, text_len));
                        }
                    }
                    None => pending.extend(self.atomic_children(id, &first, &last)),
                }
            }
            let Some(&(root, start, end)) = ranges.get(at) else {
                break;
            };
            at += 1;
            let Some(inline_layout) = self
                .get_node(root)
                .and_then(|node| node.element_data())
                .and_then(|data| data.inline_layout_data.as_ref())
            else {
                continue;
            };
            let mut boxes: Vec<NodeId> = inline_layout
                .layout
                .inline_boxes()
                .filter(|inline_box| {
                    inline_box.kind == parley::InlineBoxKind::InFlow
                        && (start..end).contains(&inline_box.index)
                })
                .map(|inline_box| NodeId::from_u64(inline_box.id))
                .collect();
            boxes.reverse();
            pending.extend(boxes);
        }
        ranges.retain(|&(_, start, end)| start < end);
        ranges
    }
}
