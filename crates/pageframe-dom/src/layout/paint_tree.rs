//! Construction of the paint tree (`Node::paint_children` and
//! `Node::stacking_context`) from the laid-out box tree.
//!
//! The pass runs once per frame after layout and builds *topology* only: which
//! node ids each node paints itself and which z-indexed descendants are hoisted
//! to the nearest stacking-context root. Geometry (a hoisted box's offset from
//! its stacking-context root) is derived at use time from `final_layout()` and
//! memoised per geometry generation, so replaying a cached list for a clean
//! subtree can never paint or hit-test a box where layout no longer puts it.

use pageframe_traits::node_id::NodeId;
use std::cell::{Cell, RefCell};
use std::ops::Range;
use stylo_taffy::CopyStyleGetters;
use thin_vec::ThinVec;

use crate::layout::damage::ALL_DAMAGE;
use crate::node::{NodeFlags, SpecialElementData};
use crate::tree::NodeTree;
use crate::{BaseDocument, Node};
use style::properties::generated::longhands::position::computed_value::T as Position;
use style::selector_parser::RestyleDamage;
use style::values::computed::Float;
use style::values::specified::box_::{DisplayInside, DisplayOutside};
use taffy::{Point, Rect};

/// Offset of the hoisted child `child_id` from the border box of its
/// stacking-context root `sc_root_id`, excluding the child's own
/// `final_layout().location`: the sum of `location - scroll_offset` of every
/// box strictly between the two on the child's positioning chain.
///
/// Walks `Node::containing_block` (the out-of-flow containing block for boxes
/// hoisted by Taffy, otherwise `layout_parent`, which is set for every layout
/// child by `resolve_layout_children`, anonymous blocks included).
pub fn hoisted_child_position(tree: &NodeTree, sc_root_id: NodeId, child_id: NodeId) -> Point<f32> {
    hoisted_child_geometry(tree, sc_root_id, child_id).position
}

enum AncestorGroup {
    Kept,
    Left(f32),
}

#[derive(Default)]
struct HoistedGeometry {
    position: Point<f32>,
    clip: Option<Rect<f32>>,
    clippers: ThinVec<(NodeId, Point<f32>)>,
}

fn hoisted_child_geometry(
    tree: &NodeTree,
    sc_root_id: NodeId,
    child_id: NodeId,
) -> HoistedGeometry {
    let child = &tree[child_id];
    let mut position = Point::ZERO;
    let mut clip: Option<Rect<f32>> = None;
    let mut clippers = ThinVec::new();
    let mut current = child.containing_block();

    // A fixed-position box whose containing block is the root element is
    // positioned against the viewport and does not scroll with it: cancel the
    // scroll the walk below (or the stacking-context root itself) applies.
    // A fixed box contained by a transformed ancestor scrolls with it.
    if child.taffy_position() == taffy::Position::Fixed
        && let Some(cb) = current.and_then(|id| tree.get(id))
        && cb.containing_block().is_none()
    {
        let scroll_offset = *cb.scroll_offset();
        let viewport_scroll = tree.viewport_scroll();
        position.x += scroll_offset.x as f32 + viewport_scroll.x as f32;
        position.y += scroll_offset.y as f32 + viewport_scroll.y as f32;
    }

    while let Some(id) = current {
        if id == sc_root_id {
            return HoistedGeometry {
                position,
                clip,
                clippers,
            };
        }
        let Some(node) = tree.get(id) else {
            break;
        };
        let location = node.final_layout().location;
        let scroll_offset = *node.scroll_offset();
        position.x += location.x - scroll_offset.x as f32;
        position.y += location.y - scroll_offset.y as f32;
        if let Some(bounds) = node.overflow_clip_bounds() {
            let x = position.x - location.x;
            let y = position.y - location.y;
            clippers.push((id, Point { x: -x, y: -y }));
            let inner = clip.unwrap_or(Rect {
                left: f32::NEG_INFINITY,
                right: f32::INFINITY,
                top: f32::NEG_INFINITY,
                bottom: f32::INFINITY,
            });
            let left = (bounds.left - x).max(inner.left);
            let top = (bounds.top - y).max(inner.top);
            clip = Some(Rect {
                left,
                right: (bounds.right - x).min(inner.right).max(left),
                top,
                bottom: (bounds.bottom - y).min(inner.bottom).max(top),
            });
        }
        current = node.containing_block();
    }
    let origin = tree[sc_root_id].absolute_position(0.0, 0.0);
    position.x -= origin.x;
    position.y -= origin.y;
    HoistedGeometry {
        position,
        clip,
        clippers,
    }
}

/// A child with a z_index that is hoisted up to it's containing Stacking Context for paint purposes
#[derive(Debug, Clone)]
pub struct HoistedPaintChild {
    pub node_id: NodeId,
    pub z_index: i32,
    /// `(geometry generation, offset from the stacking-context root)`, see
    /// [`Self::position`]. Generation `0` is never current.
    position: Cell<(u64, Point<f32>)>,
    clip: Cell<Option<Rect<f32>>>,
    clippers: RefCell<ThinVec<(NodeId, Point<f32>)>>,
}

impl HoistedPaintChild {
    fn new(node_id: NodeId, z_index: i32) -> Self {
        Self {
            node_id,
            z_index,
            position: Cell::new((0, Point::ZERO)),
            clip: Cell::new(None),
            clippers: RefCell::new(ThinVec::new()),
        }
    }

    /// The child's offset from the border box of its stacking-context root
    /// (`sc_root_id`), excluding the child's own `final_layout().location`.
    /// Computed at most once per geometry generation.
    pub fn position(&self, tree: &NodeTree, sc_root_id: NodeId) -> Point<f32> {
        let generation = tree.geometry_generation();
        let (stamp, cached) = self.position.get();
        if stamp == generation {
            return cached;
        }
        let geometry = hoisted_child_geometry(tree, sc_root_id, self.node_id);
        self.position.set((generation, geometry.position));
        self.clip.set(geometry.clip);
        *self.clippers.borrow_mut() = geometry.clippers;
        geometry.position
    }

    pub fn clip(&self, tree: &NodeTree, sc_root_id: NodeId) -> Option<Rect<f32>> {
        self.position(tree, sc_root_id);
        self.clip.get()
    }

    pub fn with_clippers<R>(
        &self,
        tree: &NodeTree,
        sc_root_id: NodeId,
        visit: impl FnOnce(&[(NodeId, Point<f32>)]) -> R,
    ) -> R {
        self.position(tree, sc_root_id);
        visit(&self.clippers.borrow())
    }

    pub fn is_clipped_at(&self, tree: &NodeTree, sc_root_id: NodeId, span: Rect<f32>) -> bool {
        self.clip(tree, sc_root_id).is_some_and(|clip| {
            span.right <= clip.left
                || span.left >= clip.right
                || span.bottom <= clip.top
                || span.top >= clip.bottom
        })
    }

    /// Whether [`Self::position`] is memoised for the current geometry generation.
    pub fn position_is_cached(&self, tree: &NodeTree) -> bool {
        self.position.get().0 == tree.geometry_generation()
    }
}

#[derive(Debug)]
pub struct StackingContext {
    pub children: ThinVec<HoistedPaintChild>,
    /// The number of hoisted point children with negative z_index
    pub negative_z_count: u32,
    /// `(geometry generation, bounding box of the hoisted children)`, see
    /// [`Self::hoisted_content_bbox`].
    hoisted_content_bbox: Cell<(u64, Rect<f32>)>,
}

impl StackingContext {
    /// Build a sorted stacking context from the hoisted entries collected for it.
    fn from_children(children: ThinVec<HoistedPaintChild>) -> Self {
        let mut sc = Self {
            children,
            negative_z_count: 0,
            hoisted_content_bbox: Cell::new((0, Rect::ZERO)),
        };
        sc.sort();
        sc
    }

    /// Bounding box (in CSS pixels relative to the stacking-context root
    /// `sc_root_id`, unscrolled) of the scrollable overflow of all hoisted
    /// children, including their transforms. `scale` is the device pixel
    /// ratio `scrollable_overflow` is stored in. Computed at most once per
    /// geometry generation.
    pub fn hoisted_content_bbox(
        &self,
        tree: &NodeTree,
        sc_root_id: NodeId,
        scale: f64,
    ) -> Rect<f32> {
        let generation = tree.geometry_generation();
        let (stamp, cached) = self.hoisted_content_bbox.get();
        if stamp == generation {
            return cached;
        }

        let child_rect = |child: &HoistedPaintChild| -> Option<Rect<f32>> {
            let node = tree.get(child.node_id)?;
            let position = child.position(tree, sc_root_id);
            let location = node.final_layout().location;
            let mut overflow = *node.scrollable_overflow();
            if let Some(transform) = node.transform().as_deref() {
                overflow = transform.transform_rect_bbox(overflow);
            }
            let left = position.x + location.x;
            let top = position.y + location.y;
            Some(Rect {
                left: left + (overflow.x0 / scale) as f32,
                top: top + (overflow.y0 / scale) as f32,
                right: left + (overflow.x1 / scale) as f32,
                bottom: top + (overflow.y1 / scale) as f32,
            })
        };

        // Children dropped by a DOM mutation since the last `resolve` yield
        // `None` and are left out, like hit testing skips them.
        let mut rects = self.children.iter().filter_map(child_rect);
        let mut area = rects.next().unwrap_or(Rect::ZERO);
        for pos in rects {
            area.left = area.left.min(pos.left);
            area.top = area.top.min(pos.top);
            area.right = area.right.max(pos.right);
            area.bottom = area.bottom.max(pos.bottom);
        }
        self.hoisted_content_bbox.set((generation, area));
        area
    }

    pub fn sort(&mut self) {
        self.children.sort_by_key(|c| c.z_index);
        self.negative_z_count = self.children.iter().take_while(|c| c.z_index < 0).count() as u32;
    }

    pub fn neg_z_range(&self) -> Range<usize> {
        0..(self.negative_z_count as usize)
    }

    pub fn pos_z_range(&self) -> Range<usize> {
        (self.negative_z_count as usize)..self.children.len()
    }

    pub fn neg_z_hoisted_children(
        &self,
    ) -> impl ExactSizeIterator<Item = &HoistedPaintChild> + DoubleEndedIterator {
        self.children[self.neg_z_range()].iter()
    }

    pub fn pos_z_hoisted_children(
        &self,
    ) -> impl ExactSizeIterator<Item = &HoistedPaintChild> + DoubleEndedIterator {
        self.children[self.pos_z_range()].iter()
    }
}

impl BaseDocument {
    /// Rebuild `paint_children` and `stacking_context` for the tree rooted at
    /// `root_id` from the laid-out box tree.
    ///
    /// Each node's layout children are placed either in its `paint_children`
    /// (sorted by paint order) or, when they have a non-zero `z-index` and are
    /// positioned or flex/grid items, in the nearest ancestor stacking context.
    ///
    /// Subtrees whose stored damage lacks `REBUILD_STACKING_CONTEXT` are skipped: their
    /// own lists are still valid and the entries they contributed to the
    /// enclosing stacking context are replayed from `Node::sc_contribution_cache`.
    pub(crate) fn build_paint_tree(&mut self, root_id: NodeId) {
        self.build_paint_tree_impl(root_id, None);
    }

    /// `stacking_context` is the entry list of the nearest ancestor stacking
    /// context, or `None` when `node_id` is itself a stacking-context root.
    fn build_paint_tree_impl(
        &mut self,
        node_id: NodeId,
        stacking_context: Option<&mut ThinVec<HoistedPaintChild>>,
    ) {
        {
            let node = &self.nodes[node_id];
            // `propagate_damage_flags` stores the union of a node's own and
            // its descendants' damage on the node, so the absence of
            // `REBUILD_STACKING_CONTEXT` (which every heavier level implies)
            // means nothing in the subtree can have changed a paint list.
            // Anonymous boxes are never skipped: damage marking walks the DOM
            // parent chain, which bypasses them.
            //
            // Whether a node is a stacking-context root also depends on its
            // parent (z-indexed flex/grid items), which does not damage the
            // node itself, so the role the caller resolved must match the one
            // this node was last built with.
            let role_unchanged = node.stacking_context.is_some() == stacking_context.is_none();
            let clean = role_unchanged
                && !node
                    .damage()
                    .unwrap_or(ALL_DAMAGE)
                    .contains(RestyleDamage::REBUILD_STACKING_CONTEXT)
                && !node.is_anonymous();
            if clean {
                if let Some(stacking_context) = stacking_context {
                    stacking_context.extend(node.sc_contribution_cache.borrow().iter().cloned());
                }
                return;
            }
        }

        self.nodes[node_id]
            .flags
            .remove(NodeFlags::HOISTS_INLINE_LAYERS);
        let Some(display) = self.nodes[node_id].display_style() else {
            return;
        };

        // Entries hoisted out of this subtree are collected directly into the
        // storage that will hold them afterwards: the node's own stacking
        // context for a root, otherwise its contribution cache (from which
        // they are copied into the enclosing stacking context below).
        let mut entries = {
            let node = &mut self.nodes[node_id];
            let mut entries = match stacking_context {
                None => node
                    .stacking_context
                    .take()
                    .map(|sc| sc.children)
                    .unwrap_or_default(),
                Some(_) => std::mem::take(&mut *node.sc_contribution_cache.borrow_mut()),
            };
            entries.clear();
            entries
        };

        let children = self.nodes[node_id].layout_children.borrow_mut().take();
        if let Some(children) = children {
            let is_flex_or_grid =
                matches!(display.inside(), DisplayInside::Flex | DisplayInside::Grid);

            // Out-of-flow boxes are laid out (and painted) relative to their
            // containing block, as recorded by Taffy's out-of-flow pass in the
            // containing block's `hoisted_children`. A box whose containing
            // block is not its layout parent is owned by the containing block;
            // its layout parent neither visits nor lists it.
            let owned_here = |doc: &Self, child_id: NodeId| -> bool {
                doc.nodes[child_id]
                    .oof_containing_block
                    .get()
                    .is_none_or(|cb| cb == node_id)
                    || doc.nodes[child_id].stays_in_ancestor_group()
            };
            // Boxes hoisted to this node from further down the tree, keyed by
            // the index of the layout child they descend from: they are visited
            // and listed in tree order, directly after that child (CSS 2.1
            // Appendix E). Boxes whose layout-parent chain does not lead through
            // a layout child of this node come after all of them.
            let mut hoisted_past_parent: Vec<(usize, NodeId)> = self.nodes[node_id]
                .hoisted_children
                .borrow()
                .iter()
                .copied()
                .filter(|&id| {
                    self.nodes.get(id).is_some_and(|n| {
                        n.layout_parent.get() != Some(node_id) && !n.stays_in_ancestor_group()
                    })
                })
                .map(|id| {
                    let anchor = self
                        .layout_child_containing(node_id, id)
                        .and_then(|anchor| children.iter().position(|&c| c == anchor))
                        .unwrap_or(children.len());
                    (anchor, id)
                })
                .collect();
            hoisted_past_parent.sort_by_key(|&(anchor, _)| anchor);
            let mut next_hoisted = 0;

            let in_removed_column = self.nodes[node_id]
                .layout_parent
                .get()
                .and_then(|parent_id| self.nodes[parent_id].element_data())
                .is_some_and(|element| match &element.special_data {
                    SpecialElementData::TableRoot(table) => table.column_cell_is_collapsed(node_id),
                    _ => false,
                });
            let is_layer_root = stacking_context.is_none()
                || (self.nodes[node_id].is_paint_layer_root() && !in_removed_column);
            let keyed_base = self.scratch.keyed_children.len();
            let layers = match is_layer_root || self.nodes[node_id].columns().is_some() {
                true => Vec::new(),
                false => self.inline_layers(node_id).unwrap_or_default(),
            };
            let mut next_layer = 0;

            for (index, &child_id) in children.iter().enumerate() {
                while let Some(&(anchor, layer)) = layers.get(next_layer)
                    && anchor == Some(child_id)
                {
                    entries.push(HoistedPaintChild::new(layer, 0));
                    next_layer += 1;
                }
                if owned_here(self, child_id) {
                    let removed_column =
                        self.nodes[node_id].element_data().is_some_and(|element| {
                            match &element.special_data {
                                SpecialElementData::TableRoot(table) => {
                                    table.column_cell_is_collapsed(child_id)
                                }
                                _ => false,
                            }
                        });
                    if removed_column {
                        self.build_paint_tree_impl(child_id, Some(&mut entries));
                        continue;
                    }
                    let is_sc_root = self.nodes[child_id].is_stacking_context_root(is_flex_or_grid);
                    let first_entry = entries.len();
                    self.build_paint_tree_impl(
                        child_id,
                        match is_sc_root {
                            true => None,
                            false => Some(&mut entries),
                        },
                    );

                    // Only stacking contexts with a non-zero z-index are lifted
                    // out of tree order into the enclosing stacking context's
                    // z-sorted list (CSS 2.1 Appendix E).
                    let child = &self.nodes[child_id];
                    let z_index = child.z_index();
                    let paint_order = node_to_paint_order(child, is_flex_or_grid);
                    let paint_order = if paint_order.0 < 2 && child.is_in_lifted_row() {
                        (2, paint_order.1)
                    } else {
                        paint_order
                    };
                    if is_sc_root && z_index != 0 {
                        entries.push(HoistedPaintChild::new(child_id, z_index))
                    } else if !is_layer_root
                        && (child.is_in_flow_positioned()
                            || (is_sc_root && !is_flex_or_grid && child.is_static()))
                    {
                        // Positioned boxes paint above all in-flow content of
                        // the nearest positioned (or clipping) ancestor, not just
                        // of their parent: lift them out of this static box.
                        entries.push(HoistedPaintChild::new(child_id, 0))
                    } else {
                        let collapsed_cell = !is_sc_root
                            && !child.is_in_flow_positioned()
                            && self.nodes[node_id].element_data().is_some_and(|element| {
                                match &element.special_data {
                                    SpecialElementData::TableRoot(table) => {
                                        table.cell_is_collapsed(child_id)
                                    }
                                    _ => false,
                                }
                            });
                        if !collapsed_cell {
                            let is_caption = paint_order == (0, 0)
                                && self.nodes[node_id].element_data().is_some_and(|element| {
                                    match &element.special_data {
                                        SpecialElementData::TableRoot(table) => table
                                            .captions
                                            .iter()
                                            .any(|caption| caption.node_id == child_id),
                                        _ => false,
                                    }
                                });
                            let paint_order = if is_caption { (0, -1) } else { paint_order };
                            self.scratch.keyed_children.push((paint_order, child_id));
                        }
                    }

                    // Positioned descendants lifted out of the static boxes of
                    // this child's subtree join this node's positioned paint
                    // level, in tree order after the child.
                    if is_layer_root && entries.len() > first_entry {
                        let subtree_entries: ThinVec<_> = entries.drain(first_entry..).collect();
                        for entry in subtree_entries {
                            if entry.z_index == 0 {
                                self.scratch
                                    .keyed_children
                                    .push(((2, paint_order.1), entry.node_id));
                            } else {
                                entries.push(entry);
                            }
                        }
                    }
                }

                while let Some(&(anchor, hoisted_id)) = hoisted_past_parent.get(next_hoisted)
                    && anchor == index
                {
                    self.attach_hoisted_past_parent(hoisted_id, is_flex_or_grid, &mut entries);
                    next_hoisted += 1;
                }
            }
            for &(_, hoisted_id) in &hoisted_past_parent[next_hoisted..] {
                self.attach_hoisted_past_parent(hoisted_id, is_flex_or_grid, &mut entries);
            }
            for &(_, layer) in layers.get(next_layer..).unwrap_or_default() {
                entries.push(HoistedPaintChild::new(layer, 0));
            }
            self.nodes[node_id]
                .flags
                .set(NodeFlags::HOISTS_INLINE_LAYERS, !layers.is_empty());

            self.scratch.keyed_children[keyed_base..].sort_by_key(|&(paint_order, _)| paint_order);

            let mut paint_children = self.nodes[node_id]
                .paint_children
                .borrow_mut()
                .take()
                .unwrap_or_default();
            paint_children.clear();
            paint_children.extend(
                self.scratch
                    .keyed_children
                    .drain(keyed_base..)
                    .map(|(_, id)| id),
            );

            let node = &self.nodes[node_id];
            *node.paint_children.borrow_mut() = Some(paint_children);
            *node.layout_children.borrow_mut() = Some(children);
        }

        let node = &mut self.nodes[node_id];
        if let Some(stacking_context) = stacking_context {
            node.stacking_context = None;
            stacking_context.extend(entries.iter().cloned());
            *node.sc_contribution_cache.borrow_mut() = entries;
        } else {
            node.sc_contribution_cache.borrow_mut().clear();
            node.stacking_context = Some(Box::new(StackingContext::from_children(entries)));
        }
    }

    fn inline_layers(&self, root_id: NodeId) -> Option<Vec<(Option<NodeId>, NodeId)>> {
        let root = &self.nodes[root_id];
        if !root.flags.is_inline_root() {
            return None;
        }
        let mut layers: Vec<(Option<NodeId>, NodeId)> = Vec::new();
        let mut anchored = 0;
        let mut stack: Vec<(NodeId, bool)> = Vec::new();
        let push_children = |stack: &mut Vec<(NodeId, bool)>, node: &Node, in_layer: bool| {
            let children = node
                .before()
                .into_iter()
                .chain(node.children.iter().copied())
                .chain(node.after());
            let start = stack.len();
            stack.extend(children.map(|child| (child, in_layer)));
            stack[start..].reverse();
        };
        push_children(&mut stack, root, false);
        while let Some((id, in_layer)) = stack.pop() {
            let Some(node) = self.nodes.get(id) else {
                continue;
            };
            if node.element_data().is_none() {
                continue;
            }
            let display = node.display_style();
            let outside = display.map(|display| display.outside());
            let inside = display.map(|display| display.inside());
            if outside == Some(DisplayOutside::None) {
                if inside == Some(DisplayInside::Contents) {
                    push_children(&mut stack, node, in_layer);
                }
                continue;
            }
            if node.is_non_atomic_inline() {
                let starts = !in_layer && node.starts_inline_layer();
                if starts {
                    layers.push((None, id));
                }
                push_children(&mut stack, node, in_layer || starts);
                continue;
            }
            if in_layer {
                return None;
            }
            for layer in layers.get_mut(anchored..).unwrap_or_default() {
                layer.0 = Some(id);
            }
            anchored = layers.len();
        }
        (!layers.is_empty()).then_some(layers)
    }

    /// The layout child of `node_id` whose subtree contains `id`, following
    /// `layout_parent`.
    fn layout_child_containing(&self, node_id: NodeId, id: NodeId) -> Option<NodeId> {
        let mut current = id;
        loop {
            let parent = self.nodes.get(current)?.layout_parent.get()?;
            if parent == node_id {
                return Some(current);
            }
            current = parent;
        }
    }

    /// Visit an out-of-flow box hoisted to its containing block past its
    /// layout parent and place it in the paint lists: z-indexed boxes go to
    /// the enclosing stacking context like any other z-indexed positioned
    /// child, the rest join `paint_children` at the same paint level as other
    /// positioned boxes (the caller's stable sort keeps tree order).
    fn attach_hoisted_past_parent(
        &mut self,
        child_id: NodeId,
        is_flex_or_grid: bool,
        entries: &mut ThinVec<HoistedPaintChild>,
    ) {
        self.build_paint_tree_impl(
            child_id,
            match self.nodes[child_id].is_stacking_context_root(false) {
                true => None,
                false => Some(entries),
            },
        );

        let z_index = self.nodes[child_id].z_index();
        if z_index != 0 {
            entries.push(HoistedPaintChild::new(child_id, z_index));
        } else {
            let paint_order = node_to_paint_order(&self.nodes[child_id], is_flex_or_grid);
            self.scratch.keyed_children.push((paint_order, child_id));
        }
    }
}

impl Node {
    pub(crate) fn hit_clip(&self) -> Option<HitClip> {
        let layers = self.overflow_clip_bounds()?;
        let is_root_element = self
            .parent
            .and_then(|parent| self.tree().get(parent))
            .is_none_or(|parent| parent.parent.is_none());
        if is_root_element {
            return None;
        }
        if !self.is_hit_layer() {
            return Some(HitClip {
                layers,
                flow: layers,
            });
        }
        let size = self.final_layout().size;
        let edge = |clipped: f32, outer: f32| match clipped.is_finite() {
            true => outer,
            false => clipped,
        };
        let flow = Rect {
            left: edge(layers.left, 0.0),
            right: edge(layers.right, size.width),
            top: edge(layers.top, 0.0),
            bottom: edge(layers.bottom, size.height),
        };
        Some(HitClip { layers, flow })
    }

    pub fn used_overflow(
        &self,
    ) -> Option<(
        style::values::computed::Overflow,
        style::values::computed::Overflow,
    )> {
        use style::values::computed::Overflow;

        let style = self.primary_styles()?;
        match self.overflow_goes_to_viewport() {
            true => Some((Overflow::Visible, Overflow::Visible)),
            false => Some((style.clone_overflow_x(), style.clone_overflow_y())),
        }
    }

    pub(crate) fn overflow_goes_to_viewport(&self) -> bool {
        use style::values::computed::Overflow;

        let is_named = |node: &Node, name: &markup5ever::LocalName| {
            node.data
                .downcast_element()
                .is_some_and(|element| element.name.local == *name)
        };
        let body = markup5ever::local_name!("body");
        if !is_named(self, &body) {
            return false;
        }
        let tree = self.tree();
        let Some(root) = self.parent.and_then(|parent| tree.get(parent)) else {
            return false;
        };
        let is_root_element = root
            .parent
            .and_then(|parent| tree.get(parent))
            .is_some_and(|document| document.parent.is_none());
        let first_body = root
            .children
            .iter()
            .filter_map(|&child| tree.get(child))
            .find(|child| is_named(child, &body));
        let is_uncontained = |node: &Node| {
            node.primary_styles()
                .is_some_and(|style| style.clone_contain().is_empty())
        };
        is_root_element
            && is_named(root, &markup5ever::local_name!("html"))
            && first_body.is_some_and(|body| body.id == self.id)
            && is_uncontained(self)
            && root.primary_styles().is_some_and(|style| {
                style.clone_overflow_x() == Overflow::Visible
                    && style.clone_overflow_y() == Overflow::Visible
                    && style.clone_contain().is_empty()
            })
    }

    pub(crate) fn is_hit_layer(&self) -> bool {
        self.primary_styles()
            .is_some_and(|style| style.clone_position() != Position::Static)
            || self.is_stacking_context_root(false)
    }

    fn overflow_clip_bounds(&self) -> Option<Rect<f32>> {
        use style::values::computed::{Contain, Overflow};

        let style = self.primary_styles()?;
        let box_style = style.get_box();
        let display = box_style.display;
        let (overflow_x, overflow_y) = self.used_overflow()?;
        let overflows_x = overflow_x != Overflow::Visible;
        let overflows_y = overflow_y != Overflow::Visible;
        let contains_paint = (!overflows_x || !overflows_y)
            && box_style.slow_clone_contain().contains(Contain::PAINT)
            && !display.is_inline_flow()
            && !(display.outside() == DisplayOutside::InternalTable
                && display.inside() != DisplayInside::TableCell);
        let clips_x = contains_paint || overflows_x;
        let clips_y = contains_paint || overflows_y;
        if !clips_x && !clips_y {
            return None;
        }
        let layout = self.final_layout();
        let (left, right) = match clips_x {
            true => (layout.border.left, layout.size.width - layout.border.right),
            false => (f32::NEG_INFINITY, f32::INFINITY),
        };
        let (top, bottom) = match clips_y {
            true => (layout.border.top, layout.size.height - layout.border.bottom),
            false => (f32::NEG_INFINITY, f32::INFINITY),
        };
        Some(Rect {
            left,
            right,
            top,
            bottom,
        })
    }

    fn is_removed_column_cell(&self) -> bool {
        self.layout_parent
            .get()
            .and_then(|id| self.with(id).element_data())
            .is_some_and(|element| match &element.special_data {
                SpecialElementData::TableRoot(table) => table.column_cell_is_collapsed(self.id),
                _ => false,
            })
    }

    fn ancestor_group(&self) -> AncestorGroup {
        use style::values::computed::basic_shape::ClipPath;

        if self.flags.is_in_top_layer() {
            return AncestorGroup::Left(1.0);
        }
        let Some(containing_block) = self.oof_containing_block.get() else {
            return AncestorGroup::Left(1.0);
        };
        let mut is_in_context = false;
        let mut is_clipped = false;
        let mut opacity = 1.0;
        let mut current = self.layout_parent.get();
        while let Some(id) = current.filter(|&id| id != containing_block) {
            let ancestor = self.with(id);
            let style = ancestor.primary_styles();
            if style
                .as_ref()
                .is_some_and(|style| !matches!(style.get_clip_path(), ClipPath::None))
            {
                return AncestorGroup::Kept;
            }
            if !ancestor.is_removed_column_cell() {
                is_in_context |= ancestor.is_stacking_context_root(ancestor.is_flex_or_grid_item());
                opacity *= style.map_or(1.0, |style| style.clone_opacity());
            }
            is_clipped |= ancestor.overflow_clip_bounds().is_some();
            current = ancestor.layout_parent.get();
        }
        match is_in_context && !is_clipped {
            true => AncestorGroup::Kept,
            false => AncestorGroup::Left(opacity),
        }
    }

    fn stays_in_ancestor_group(&self) -> bool {
        matches!(self.ancestor_group(), AncestorGroup::Kept)
    }

    fn is_flex_or_grid_item(&self) -> bool {
        self.layout_parent.get().is_some_and(|id| {
            matches!(
                self.with(id).taffy_display(),
                taffy::Display::Flex | taffy::Display::Grid
            )
        })
    }

    pub fn skipped_ancestor_opacity(&self) -> f32 {
        match self.ancestor_group() {
            AncestorGroup::Kept => 1.0,
            AncestorGroup::Left(opacity) => opacity,
        }
    }

    /// Whether this node paints its positioned descendants itself (CSS 2.1
    /// Appendix E step 8) rather than leaving them to an ancestor: positioned
    /// boxes, and boxes that clip their contents. Stacking-context roots are
    /// handled by the caller.
    fn is_paint_layer_root(&self) -> bool {
        use style::values::computed::{Contain, Overflow};

        let Some(style) = self.primary_styles() else {
            return true;
        };
        let box_style = style.get_box();
        let (overflow_x, overflow_y) = self
            .used_overflow()
            .unwrap_or((Overflow::Visible, Overflow::Visible));
        style.clone_position() != Position::Static
            || !matches!(overflow_x, Overflow::Visible)
            || !matches!(overflow_y, Overflow::Visible)
            || box_style.slow_clone_contain().contains(Contain::PAINT)
            || self.element_data().is_some_and(|e| {
                e.raster_image_data().is_some()
                    || e.sub_doc_data().is_some()
                    || e.text_input_data().is_some()
            })
    }

    /// Whether this is a `position: relative` or `sticky` box.
    fn is_in_flow_positioned(&self) -> bool {
        self.primary_styles().is_some_and(|style| {
            matches!(
                style.clone_position(),
                Position::Relative | Position::Sticky
            )
        })
    }

    fn is_table_cell(&self) -> bool {
        self.primary_styles()
            .is_some_and(|style| style.clone_display().inside() == DisplayInside::TableCell)
    }

    pub fn is_in_lifted_row(&self) -> bool {
        let is_cell = self.is_table_cell();
        let mut part = self.parent.filter(|_| is_cell).map(|id| self.with(id));
        while let Some(node) = part.filter(|node| !node.flags.is_table_root()) {
            let is_faded = node
                .primary_styles()
                .is_some_and(|style| style.clone_opacity() != 1.0);
            if is_faded || node.is_in_flow_positioned() {
                return true;
            }
            part = node.parent.map(|id| node.with(id));
        }
        false
    }

    pub fn row_opacity(&self) -> f32 {
        let is_cell = self.is_table_cell();
        let mut opacity = 1.0;
        let mut part = self.parent.filter(|_| is_cell).map(|id| self.with(id));
        while let Some(node) = part.filter(|node| !node.flags.is_table_root()) {
            if let Some(style) = node.primary_styles() {
                opacity *= style.clone_opacity();
            }
            part = node.parent.map(|id| node.with(id));
        }
        opacity
    }

    pub fn inline_ancestor_opacity(&self) -> f32 {
        let layout_parent = self.layout_parent.get();
        let escaped_block = self
            .oof_containing_block
            .get()
            .filter(|block| Some(*block) != layout_parent && !self.stays_in_ancestor_group());
        let stop = escaped_block.or(layout_parent);
        let step = |id: Option<NodeId>| id.filter(|id| Some(*id) != stop).map(|id| self.with(id));
        let mut opacity = 1.0;
        let mut part = step(self.parent);
        while let Some(node) = part {
            let Some(style) = node.primary_styles() else {
                break;
            };
            let display = style.get_box().display;
            if display.is_contents() {
                part = step(node.parent);
                continue;
            }
            if display.is_inline_flow() {
                opacity *= style.clone_opacity();
            } else if escaped_block.is_none() {
                break;
            }
            part = step(node.parent);
        }
        opacity
    }

    pub fn removed_cell_opacity(&self) -> f32 {
        let is_item = Node::is_flex_or_grid_item;
        let is_context = self.is_stacking_context_root(is_item(self));
        let is_positioned = self
            .primary_styles()
            .is_some_and(|style| style.clone_position() != Position::Static);
        if !is_context && !is_positioned {
            return 1.0;
        }
        let passes_layers = is_context && self.z_index() != 0;
        let mut outer_block = match self.stays_in_ancestor_group() {
            true => None,
            false => self.oof_containing_block.get(),
        };
        let mut opacity = 1.0;
        let mut part = self.parent.map(|id| self.with(id));
        while let Some(ancestor) = part {
            let is_removed_cell = ancestor.is_removed_column_cell();
            if outer_block == Some(ancestor.id) {
                outer_block = None;
            }
            if is_removed_cell {
                opacity *= ancestor
                    .primary_styles()
                    .map_or(1.0, |style| style.clone_opacity())
                    * ancestor.row_opacity();
            } else if outer_block.is_none()
                && (ancestor.is_stacking_context_root(is_item(ancestor))
                    || (!passes_layers && ancestor.is_paint_layer_root()))
            {
                return opacity;
            }
            part = ancestor.parent.map(|id| self.with(id));
        }
        opacity
    }

    pub fn is_float_under_text(&self) -> bool {
        self.is_static()
            && !self.is_stacking_context_root(false)
            && self
                .primary_styles()
                .is_some_and(|style| style.clone_float() != Float::None)
    }

    pub(crate) fn hit_phase(&self) -> Option<HitPhase> {
        let style = self.primary_styles()?;
        let in_flex_or_grid = self
            .layout_parent
            .get()
            .and_then(|id| self.tree().get(id))
            .and_then(|parent| parent.primary_styles())
            .is_some_and(|parent| {
                matches!(
                    parent.clone_display().inside(),
                    DisplayInside::Flex | DisplayInside::Grid
                )
            });
        let is_svg_or_frame = self.element_data().is_some_and(|element| {
            &*element.name.local == "svg" || element.sub_doc_data().is_some()
        });
        if self.is_hit_layer() {
            return Some(HitPhase::Positioned);
        }
        if in_flex_or_grid
            || self.display_constructed_as().outside() == DisplayOutside::Inline
            || is_svg_or_frame
        {
            return Some(HitPhase::Foreground);
        }
        (style.clone_float() != Float::None).then_some(HitPhase::Float)
    }

    fn is_static(&self) -> bool {
        self.primary_styles()
            .is_some_and(|style| style.clone_position() == Position::Static)
    }

    /// The offset from the border box of `painter_id` at which this node is
    /// painted, excluding its own `final_layout().location`, when it is in the
    /// paint list of `painter_id` without being laid out relative to it (a
    /// positioned box lifted past static ancestors).
    pub fn lifted_paint_offset(&self, painter_id: NodeId) -> Option<Point<f32>> {
        match self.containing_block() {
            Some(id) if id == painter_id => None,
            None => None,
            Some(_) => Some(hoisted_child_position(self.tree(), painter_id, self.id)),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct HitClip {
    pub(crate) layers: Rect<f32>,
    pub(crate) flow: Rect<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum HitPhase {
    BlockBackground,
    Float,
    Foreground,
    Positioned,
}

#[inline(always)]
fn position_to_order(pos: Position) -> i32 {
    match pos {
        Position::Static => 0,
        // All positioned descendants with z-index: auto share one paint
        // level (CSS 2.1 Appendix E step 8); the stable sort keeps them in
        // tree order among themselves, above in-flow content and floats.
        Position::Relative | Position::Sticky | Position::Absolute | Position::Fixed => 2,
    }
}
#[inline(always)]
fn float_to_order(pos: Float) -> i32 {
    match pos {
        Float::None => 0,
        _ => 1,
    }
}

/// Paint sort key: (paint level, order-modified position). Positioned
/// (z-index: auto) descendants paint above in-flow content (CSS 2.1
/// Appendix E step 8); within a level the stable sort preserves
/// (order-modified) document order.
#[inline(always)]
fn node_to_paint_order(node: &Node, is_flex_or_grid: bool) -> (i32, i32) {
    let Some(style) = node.primary_styles() else {
        return (0, 0);
    };
    let position = style.clone_position();
    let stacks = node.is_stacking_context_root(is_flex_or_grid);
    if is_flex_or_grid {
        match position {
            Position::Static => (if stacks { 2 } else { 0 }, style.clone_order()),
            Position::Relative | Position::Sticky => (2, style.clone_order()),
            // Out-of-flow children are not flex/grid items: `order` does
            // not apply; tree order does.
            Position::Absolute | Position::Fixed => (2, 0),
        }
    } else if stacks && position == Position::Static {
        (2, 0)
    } else {
        (
            position_to_order(position) + float_to_order(style.clone_float()),
            0,
        )
    }
}
