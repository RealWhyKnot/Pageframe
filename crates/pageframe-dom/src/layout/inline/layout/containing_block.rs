use super::LayoutPassState;
use pageframe_traits::node_id::NodeId;
use style::values::computed::BorderSideWidth;
use style::values::specified::border::BorderStyle;
use style::values::specified::box_::{DisplayInside, DisplayOutside};
use stylo_taffy::{CopyStyleGetters, StyleFlags};
use taffy::{Direction, OofCandidates, OofPositioningArea, Point, Size};

struct SpanContainingBlock {
    id: NodeId,
    claims: taffy::ContainingBlockClaims,
    direction: Direction,
}

#[derive(Default)]
pub(super) struct SpanPlacement {
    pub(super) hoisted: Vec<taffy::NodeId>,
    pub(super) fallback_flags: StyleFlags,
    pub(super) overflow: Option<taffy::Rect<f32>>,
}

impl LayoutPassState<'_> {
    fn inline_span_containing_block(
        &self,
        root_id: NodeId,
        box_id: NodeId,
        position: taffy::Position,
    ) -> Option<SpanContainingBlock> {
        let root = &self.nodes[root_id];
        let root_parent = root.is_anonymous().then_some(root.parent).flatten();
        let mut current = self.nodes[box_id].parent;
        while let Some(id) = current.filter(|id| *id != root_id && Some(*id) != root_parent) {
            let ancestor = &self.nodes[id];
            let found = ancestor
                .primary_styles()
                .filter(|style| {
                    let display = style.clone_display();
                    display.outside() == DisplayOutside::Inline
                        && display.inside() == DisplayInside::Flow
                })
                .map(|style| SpanContainingBlock {
                    id,
                    claims: stylo_taffy::convert::inline_containing_block_claims(&style),
                    direction: if style.clone_direction()
                        == style::computed_values::direction::T::Rtl
                    {
                        Direction::Rtl
                    } else {
                        Direction::Ltr
                    },
                })
                .filter(|span| span.claims.for_position(position));
            if found.is_some() {
                return found;
            }
            current = ancestor.parent;
        }
        None
    }

    fn span_padding_box(&self, span: &SpanContainingBlock) -> Option<OofPositioningArea> {
        let node = &self.nodes[span.id];
        let style = node.primary_styles()?;
        let fragments: Vec<taffy::Rect<f32>> = node
            .inline_fragment_boxes(self.primary_font_extent(node))?
            .collect();
        let (first, last) = (fragments.first()?, fragments.last()?);
        let border = style.get_border();
        let side = |width: &BorderSideWidth, line: BorderStyle| {
            if line.none_or_hidden() {
                0.0
            } else {
                width.0.to_f32_px()
            }
        };
        let is_rtl = span.direction == Direction::Rtl;
        let left = if is_rtl { last.left } else { first.left }
            + side(&border.border_left_width, border.border_left_style);
        let right = if is_rtl { first.right } else { last.right }
            - side(&border.border_right_width, border.border_right_style);
        let top = first.top + side(&border.border_top_width, border.border_top_style);
        let bottom = last.bottom - side(&border.border_bottom_width, border.border_bottom_style);
        Some(OofPositioningArea {
            size: Size {
                width: (right - left).max(0.0),
                height: (bottom - top).max(0.0),
            },
            offset: Point { x: left, y: top },
        })
    }

    pub(super) fn place_span_contained_boxes(
        &mut self,
        root_id: NodeId,
        candidates: &mut OofCandidates,
        final_size: Size<f32>,
        padding: taffy::Rect<f32>,
        border: taffy::Rect<f32>,
    ) -> SpanPlacement {
        let mut by_span: Vec<(SpanContainingBlock, OofCandidates)> = Vec::new();
        let mut rest = OofCandidates::new();
        for candidate in candidates.iter() {
            let span = self.inline_span_containing_block(
                root_id,
                crate::dom_node_id(candidate.node),
                candidate.position,
            );
            match span {
                Some(span) => match by_span.iter_mut().find(|(known, _)| known.id == span.id) {
                    Some((_, list)) => list.push(*candidate),
                    None => {
                        let mut list = OofCandidates::new();
                        list.push(*candidate);
                        by_span.push((span, list));
                    }
                },
                None => rest.push(*candidate),
            }
        }
        let mut placement = SpanPlacement::default();
        let is_horizontal = self.nodes[root_id]
            .primary_styles()
            .is_some_and(|style| style.writing_mode.is_horizontal());
        if !is_horizontal {
            for (span, _) in &by_span {
                if span.claims.absolute {
                    placement.fallback_flags |= StyleFlags::SPAN_ABSOLUTE_CB;
                }
                if span.claims.fixed {
                    placement.fallback_flags |= StyleFlags::SPAN_FIXED_CB;
                }
            }
            return placement;
        }
        if by_span.is_empty() {
            return placement;
        }
        *candidates = rest;
        let root_layout = self.nodes[root_id].unrounded_layout_mut();
        root_layout.size = final_size;
        root_layout.padding = padding;
        root_layout.border = border;
        let root_area = OofPositioningArea {
            size: Size {
                width: (final_size.width - border.left - border.right).max(0.0),
                height: (final_size.height - border.top - border.bottom).max(0.0),
            },
            offset: Point {
                x: border.left,
                y: border.top,
            },
        };
        for (span, mut list) in by_span {
            for candidate in list.as_mut_slice() {
                let node = &self.nodes[crate::dom_node_id(candidate.node)];
                if node.layout_parent.get() != Some(root_id) {
                    continue;
                }
                let (across, down) = node.parent.map_or((0.0, 0.0), |parent| {
                    self.nodes[parent].inline_relative_shift()
                });
                let axes = &mut candidate.static_position;
                axes.x.area.start += across;
                axes.x.area.end += across;
                axes.y.area.start += down;
                axes.y.area.end += down;
            }
            let area = self.span_padding_box(&span).unwrap_or(root_area);
            let mut result = taffy::compute_oof_layout_for_area(
                self,
                crate::taffy_node_id(root_id),
                list.as_slice(),
                area,
                span.direction,
                span.claims,
            );
            candidates.append(&mut result.unclaimed);
            placement.hoisted.extend(result.hoisted);
            let overflow = result.scrollable_overflow_rect;
            let shift = Point {
                x: area.offset.x - border.left,
                y: area.offset.y - border.top,
            };
            let moved = taffy::Rect {
                left: overflow.left + shift.x,
                right: overflow.right + shift.x,
                top: overflow.top + shift.y,
                bottom: overflow.bottom + shift.y,
            };
            placement.overflow = Some(match placement.overflow {
                Some(known) => taffy::Rect {
                    left: known.left.min(moved.left),
                    right: known.right.max(moved.right),
                    top: known.top.min(moved.top),
                    bottom: known.bottom.max(moved.bottom),
                },
                None => moved,
            });
        }
        placement
    }
}
