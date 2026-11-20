use super::*;

pub(crate) struct GroupedRun {
    line: usize,
    left: f64,
    right: f64,
    group: Option<NodeId>,
    shift: Vec2,
}

pub(crate) fn grouped_runs(
    layout: &Layout<TextBrush>,
    doc: &BaseDocument,
    inline_root_id: NodeId,
    scale: f64,
) -> Vec<GroupedRun> {
    let mut runs = Vec::new();
    for (line_index, line) in layout.lines().enumerate() {
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };
            let owner = glyph_run.style().brush.id;
            let left = f64::from(glyph_run.offset());
            let right = left + f64::from(glyph_run.advance());
            if is_invisible(doc, owner) {
                runs.push(GroupedRun {
                    line: line_index,
                    left,
                    right,
                    group: None,
                    shift: Vec2::ZERO,
                });
                continue;
            }
            let mut group = None;
            let mut shift = Vec2::ZERO;
            let mut next_id = Some(owner);
            while let Some(node) = next_id
                .filter(|node_id| *node_id != inline_root_id)
                .and_then(|node_id| doc.get_node(node_id))
            {
                next_id = node.parent;
                let Some(styles) = node.primary_styles() else {
                    continue;
                };
                if styles.get_box().display != Display::Inline {
                    continue;
                }
                shift += relative_shift(node);
                if group.is_none() && starts_group(&styles) {
                    group = Some(node.id);
                }
            }
            if let Some(group) = group {
                runs.push(GroupedRun {
                    line: line_index,
                    left,
                    right,
                    group: Some(group),
                    shift: shift * scale,
                });
            }
        }
    }
    runs
}

pub(crate) struct SelectionPaint<'a> {
    pub(crate) range: (usize, usize),
    pub(crate) visible_lines: usize,
    pub(crate) runs: &'a [GroupedRun],
    pub(crate) group: Option<NodeId>,
}

/// Draw selection highlight rectangles for the given byte range in a layout.
/// Uses Parley's Selection type for accurate geometry calculation. Lines from
/// `visible_lines` on (those hidden by `line-clamp`) are not highlighted.
pub(crate) fn draw_text_selection(
    scene: &mut impl PaintScene,
    layout: &Layout<TextBrush>,
    transform: Affine,
    paint: SelectionPaint<'_>,
) {
    let (selection_start, selection_end) = paint.range;
    let anchor = Cursor::from_byte_index(layout, selection_start, Affinity::Downstream);
    let focus = Cursor::from_byte_index(layout, selection_end, Affinity::Downstream);
    let selection = Selection::new(anchor, focus);

    selection.geometry_with(layout, |rect, line_idx| {
        if line_idx >= paint.visible_lines {
            return;
        }
        let first = paint.runs.partition_point(|run| run.line < line_idx);
        let later = paint.runs.get(first..).unwrap_or_default();
        let count = later.partition_point(|run| run.line == line_idx);
        let line_runs = later.iter().take(count);
        let mut fill = |left: f64, right: f64, shift: Vec2| {
            if left < right {
                let rect = kurbo::Rect::new(left, rect.y0, right, rect.y1);
                let transform = transform * Affine::translate(shift);
                scene.fill(Fill::NonZero, transform, SELECTION_COLOR, None, &rect);
            }
        };
        match paint.group {
            None => {
                let mut cuts: Vec<(f64, f64)> =
                    line_runs.map(|run| (run.left, run.right)).collect();
                cuts.sort_by(|first, second| first.0.total_cmp(&second.0));
                let mut left = rect.x0;
                for (cut_left, cut_right) in cuts {
                    fill(left, cut_left.min(rect.x1), Vec2::ZERO);
                    left = left.max(cut_right);
                }
                fill(left, rect.x1, Vec2::ZERO);
            }
            Some(group) => {
                for run in line_runs.filter(|run| run.group == Some(group)) {
                    fill(rect.x0.max(run.left), rect.x1.min(run.right), run.shift);
                }
            }
        }
    });
}
