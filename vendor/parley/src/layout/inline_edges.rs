use alloc::vec::Vec;

use crate::InlineEdge;
use crate::layout::{Layout, LayoutItemKind, LineItemData};
use crate::style::Brush;

struct EdgeBox {
    owner: u64,
    is_rtl: bool,
    start: Option<usize>,
    end: Option<usize>,
}

fn edge_of<B: Brush>(layout: &Layout<B>, item: &LineItemData) -> Option<InlineEdge> {
    match item.kind {
        LayoutItemKind::TextRun => layout.data.runs[item.index].inline_edge,
        LayoutItemKind::InlineBox => None,
    }
}

fn lies_within<B: Brush>(layout: &Layout<B>, item: &LineItemData, edge_box: &EdgeBox) -> bool {
    match item.kind {
        LayoutItemKind::TextRun => {
            edge_box.start.is_none_or(|start| start <= item.text_range.start)
                && edge_box.end.is_none_or(|end| item.text_range.end <= end)
        }
        LayoutItemKind::InlineBox => {
            let index = layout.data.inline_boxes[item.index].inline_box.index;
            edge_box.start.is_none_or(|start| start < index)
                && edge_box.end.is_none_or(|end| index < end)
        }
    }
}

fn collect_boxes<B: Brush>(layout: &Layout<B>, items: &[LineItemData]) -> (Vec<EdgeBox>, bool) {
    let mut boxes: Vec<EdgeBox> = Vec::new();
    let mut has_rtl = false;
    for item in items {
        let Some(edge) = edge_of(layout, item) else {
            continue;
        };
        has_rtl |= edge.is_rtl;
        let index = match boxes.iter().position(|found| found.owner == edge.owner) {
            Some(index) => index,
            None => {
                boxes.push(EdgeBox {
                    owner: edge.owner,
                    is_rtl: edge.is_rtl,
                    start: None,
                    end: None,
                });
                boxes.len() - 1
            }
        };
        let found = &mut boxes[index];
        if edge.is_start {
            let start = item.text_range.start;
            found.start = Some(found.start.map_or(start, |known| known.min(start)));
        } else {
            let end = item.text_range.end;
            found.end = Some(found.end.map_or(end, |known| known.max(end)));
        }
    }
    (boxes, has_rtl)
}

fn place_box<B: Brush>(layout: &Layout<B>, items: &mut [LineItemData], edge_box: &EdgeBox) {
    let is_mark = |item: &LineItemData| {
        edge_of(layout, item).is_some_and(|edge| edge.owner == edge_box.owner)
    };
    let is_content = |item: &LineItemData| !is_mark(item) && lies_within(layout, item, edge_box);
    let mut marks: Vec<(InlineEdge, LineItemData)> = items
        .iter()
        .filter_map(|item| {
            edge_of(layout, item)
                .filter(|edge| edge.owner == edge_box.owner)
                .map(|edge| (edge, item.clone()))
        })
        .collect();
    marks.sort_by_key(|(_, item)| item.text_range.start);
    let starts = marks.iter().filter(|(edge, _)| edge.is_start);
    let ends = marks.iter().filter(|(edge, _)| !edge.is_start);
    let (left, right): (Vec<_>, Vec<_>) = if edge_box.is_rtl {
        (ends.rev().collect(), starts.rev().collect())
    } else {
        (starts.collect(), ends.collect())
    };
    let first = items.iter().position(is_content);
    let last = items.iter().rposition(is_content);
    let (before, after) = match (first, last) {
        (Some(first), Some(last)) => (first, last),
        _ => {
            let Some(anchor) = items.iter().position(is_mark) else {
                return;
            };
            (anchor, anchor)
        }
    };
    let mut placed: Vec<LineItemData> = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        if index == before {
            placed.extend(left.iter().map(|(_, mark)| mark.clone()));
        }
        if !is_mark(item) {
            placed.push(item.clone());
        }
        if index == after {
            placed.extend(right.iter().map(|(_, mark)| mark.clone()));
        }
    }
    items.clone_from_slice(&placed);
}

pub(crate) fn place_inline_edges<B: Brush>(
    layout: &Layout<B>,
    items: &mut [LineItemData],
    reordered: bool,
) {
    let (mut boxes, has_rtl) = collect_boxes(layout, items);
    if boxes.is_empty() || !(reordered || has_rtl) {
        return;
    }
    boxes.sort_by_key(|edge_box| {
        edge_box
            .end
            .unwrap_or(usize::MAX)
            .saturating_sub(edge_box.start.unwrap_or(0))
    });
    for edge_box in &boxes {
        place_box(layout, items, edge_box);
    }
}
