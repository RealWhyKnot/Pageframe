//! Text selection state and logic for non-input elements.
//!
//! This module handles text selection across inline roots in the document,
//! including support for anonymous blocks which have unstable IDs across
//! layout reconstruction.

use pageframe_traits::node_id::NodeId;

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

impl crate::BaseDocument {
    pub(crate) fn with_atomic_contents(
        &self,
        mut ranges: Vec<(NodeId, usize, usize)>,
    ) -> Vec<(NodeId, usize, usize)> {
        let mut seen: std::collections::HashSet<NodeId> =
            ranges.iter().map(|&(root, ..)| root).collect();
        let lookup = |parent_id, idx| self.find_anonymous_block_by_index(parent_id, idx);
        let mut holds_endpoint = std::collections::HashSet::new();
        for point in [&self.text_selection.anchor, &self.text_selection.focus] {
            let mut cursor = point.resolve_node_id(lookup);
            while let Some(id) = cursor.filter(|&id| holds_endpoint.insert(id)) {
                cursor = self
                    .get_node(id)
                    .and_then(|node| node.layout_parent.get().or(node.parent));
            }
        }
        let mut at = 0;
        while let Some(&(root, start, end)) = ranges.get(at) {
            at += 1;
            let Some(inline_layout) = self
                .get_node(root)
                .and_then(|node| node.element_data())
                .and_then(|data| data.inline_layout_data.as_ref())
            else {
                continue;
            };
            let mut pending: Vec<NodeId> = inline_layout
                .layout
                .inline_boxes()
                .filter(|inline_box| {
                    inline_box.kind == parley::InlineBoxKind::InFlow
                        && (start..end).contains(&inline_box.index)
                })
                .map(|inline_box| NodeId::from_u64(inline_box.id))
                .collect();
            while let Some(id) = pending.pop() {
                let Some(node) = self.get_node(id) else {
                    continue;
                };
                let holds = holds_endpoint.contains(&id);
                let text_len = node
                    .element_data()
                    .and_then(|data| data.inline_layout_data.as_ref())
                    .map(|inline_layout| inline_layout.text.len());
                if let Some(text_len) = text_len {
                    if text_len > 0 && !holds && seen.insert(id) {
                        ranges.push((id, 0, text_len));
                    }
                    continue;
                }
                if let Some(children) = node.layout_children.borrow().as_ref() {
                    let before = children
                        .iter()
                        .position(|child| holds && holds_endpoint.contains(child))
                        .unwrap_or(children.len());
                    pending.extend(children.iter().take(before).rev());
                }
            }
        }
        ranges
    }
}
