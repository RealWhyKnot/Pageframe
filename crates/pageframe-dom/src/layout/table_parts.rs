use std::sync::Arc;

use pageframe_traits::node_id::NodeId;
use style::computed_values::direction::T as Direction;
use stylo_taffy::CopyStyleGetters;
use taffy::{Layout, Line, Point, Size};

use super::LayoutPassState;
use super::round::{DeviceSnap, snap_layout};
use super::table::{TableContext, TableTreeWrapper};
use crate::node::SpecialElementData;
use crate::taffy_node_id;

impl TableContext {
    fn row_tracks(&self) -> Option<(Line<f32>, Vec<Line<f32>>)> {
        let info = self.computed_grid_info.borrow();
        let info = info.as_ref()?;
        let columns = info
            .columns
            .positions
            .iter()
            .skip(usize::from(info.columns.negative_implicit_tracks))
            .fold(None, |span: Option<Line<f32>>, line| {
                let low = line.start.min(line.end);
                let high = line.start.max(line.end);
                Some(span.map_or(
                    Line {
                        start: low,
                        end: high,
                    },
                    |span| Line {
                        start: span.start.min(low),
                        end: span.end.max(high),
                    },
                ))
            })?;
        let rows = info
            .rows
            .positions
            .iter()
            .skip(usize::from(info.rows.negative_implicit_tracks))
            .take(self.rows.len())
            .copied()
            .collect();
        Some((columns, rows))
    }

    fn parts(&self) -> Vec<NodeId> {
        let mut parts: Vec<NodeId> = Vec::with_capacity(self.rows.len());
        let mut groups: Vec<NodeId> = Vec::new();
        for row in &self.rows {
            parts.push(row.node_id);
            if let Some(group) = row.group.filter(|group| !groups.contains(group)) {
                groups.push(group);
            }
        }
        parts.extend(groups);
        parts
    }
}

impl TableTreeWrapper<'_, '_> {
    pub(crate) fn place_rows(&mut self, table: NodeId, inputs: &taffy::tree::LayoutInput) {
        if inputs.run_mode != taffy::RunMode::PerformLayout {
            return;
        }
        let ctx = Arc::clone(&self.ctx);
        let Some((columns, tracks)) = ctx.row_tracks() else {
            return;
        };
        let above: f32 = ctx
            .captions
            .iter()
            .filter(|caption| !caption.is_below)
            .map(|caption| {
                let layout = self.doc.nodes[caption.node_id].unrounded_layout();
                layout.margin.top + layout.size.height + layout.margin.bottom
            })
            .sum();
        let is_rtl = self.doc.nodes[table]
            .primary_styles()
            .is_some_and(|style| style.clone_direction() == Direction::Rtl);
        let mut groups: Vec<(NodeId, Line<f32>)> = Vec::new();
        for (row, track) in ctx.rows.iter().zip(tracks) {
            let track = Line {
                start: track.start + above,
                end: track.end + above,
            };
            self.place_part(&ctx, row.node_id, columns, track, is_rtl);
            let Some(group) = row.group else {
                continue;
            };
            match groups.iter_mut().find(|(id, _)| *id == group) {
                Some((_, span)) => {
                    span.start = span.start.min(track.start);
                    span.end = span.end.max(track.end);
                }
                None => groups.push((group, track)),
            }
        }
        for (group, span) in groups {
            self.place_part(&ctx, group, columns, span, is_rtl);
        }
    }

    fn place_part(
        &mut self,
        ctx: &TableContext,
        node_id: NodeId,
        columns: Line<f32>,
        rows: Line<f32>,
        is_rtl: bool,
    ) {
        let offset = ctx.cell_offset(&self.doc.nodes[node_id], is_rtl);
        let mut layout = Layout::new();
        layout.location = Point {
            x: columns.start + offset.x,
            y: rows.start + offset.y,
        };
        layout.size = Size {
            width: columns.end - columns.start,
            height: (rows.end - rows.start).max(0.0),
        };
        *self.doc.nodes[node_id].unrounded_layout_mut() = layout;
    }
}

impl LayoutPassState<'_> {
    pub(crate) fn snap_table_parts(
        &mut self,
        table: taffy::NodeId,
        physical: impl Fn(Layout) -> Layout,
        position: Point<f32>,
        snap: DeviceSnap,
    ) {
        let node = &self.nodes[crate::dom_node_id(table)];
        if !node.flags.is_table_root() {
            return;
        }
        let parts = match node
            .data
            .downcast_element()
            .map(|element| &element.special_data)
        {
            Some(SpecialElementData::TableRoot(ctx)) => ctx.parts(),
            _ => return,
        };
        for part in parts {
            let unrounded = physical(*self.nodes[part].unrounded_layout());
            snap_layout(self, taffy_node_id(part), unrounded, position, snap);
        }
    }
}
