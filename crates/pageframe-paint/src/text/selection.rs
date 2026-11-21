use super::*;

pub(crate) struct GroupedRun {
    line: usize,
    left: f64,
    right: f64,
    group: Option<NodeId>,
    shift: Vec2,
    cuts: bool,
    top: Option<f64>,
    span: Option<(f64, f64)>,
}

fn ruby_base_top(node: &pageframe_dom::Node) -> Option<f64> {
    if !node.is_ruby_box() {
        return None;
    }
    let children = node.layout_children.borrow();
    children
        .iter()
        .flatten()
        .filter_map(|&child| node.tree().get(child))
        .filter(|child| {
            child
                .element_data()
                .is_none_or(|data| &*data.name.local != "rt")
        })
        .map(|child| f64::from(child.unrounded_layout().location.y))
        .reduce(f64::min)
}

fn annotation_free_top(line: &Line<'_, TextBrush>, doc: &BaseDocument, scale: f64) -> Option<f64> {
    let mut has_ruby = false;
    let mut top = f64::INFINITY;
    for item in line.items() {
        match item {
            PositionedLayoutItem::GlyphRun(glyph_run) => {
                let ascent = glyph_run.run().font_metrics().ascent.round();
                top = top.min(f64::from(glyph_run.baseline() - ascent));
            }
            PositionedLayoutItem::InlineBox(inline_box) => {
                let y = f64::from(inline_box.y);
                let base = doc
                    .get_node(NodeId::from_u64(inline_box.id))
                    .and_then(ruby_base_top);
                has_ruby |= base.is_some();
                top = top.min(y + base.unwrap_or_default() * scale);
            }
        }
    }
    has_ruby.then_some(top)
}

fn space_advance(run: &parley::Run<'_, TextBrush>) -> Option<f32> {
    use skrifa::MetadataProvider as _;
    use skrifa::instance::{LocationRef, Size};
    use skrifa::raw::FontRef;
    use skrifa::raw::types::F2Dot14;

    let font = run.font();
    let font_ref = FontRef::from_index(font.data.as_ref(), font.index).ok()?;
    let id = font_ref.charmap().map(' ')?;
    let normalized_coords: &[i16] = bytemuck::cast_slice(run.normalized_coords());
    let coords: Vec<F2Dot14> = normalized_coords
        .iter()
        .map(|&coord| F2Dot14::from_bits(coord))
        .collect();
    font_ref
        .glyph_metrics(Size::new(run.font_size()), LocationRef::new(&coords))
        .advance_width(id)
}

fn annotation_span(
    text_layout: &Layout<TextBrush>,
    doc: &BaseDocument,
    inline_root_id: NodeId,
    scale: f64,
) -> Option<(f64, f64)> {
    let root = doc.get_node(inline_root_id)?;
    if &*root.element_data()?.name.local != "rt" {
        return None;
    }
    let layout = root.unrounded_layout();
    let width = layout.size.width
        - layout.padding.left
        - layout.padding.right
        - layout.border.left
        - layout.border.right;
    let line_break = text_layout
        .lines()
        .flat_map(|line| line.items().collect::<Vec<_>>())
        .find_map(|item| match item {
            PositionedLayoutItem::GlyphRun(glyph_run) => space_advance(glyph_run.run()),
            PositionedLayoutItem::InlineBox(_) => None,
        })
        .unwrap_or_default();
    let width = f64::from(width) * scale;
    let line_break = f64::from(line_break);
    Some(if text_layout.is_rtl() {
        (-line_break, width)
    } else {
        (0.0, width + line_break)
    })
}

pub(crate) fn grouped_runs(
    layout: &Layout<TextBrush>,
    doc: &BaseDocument,
    inline_root_id: NodeId,
    scale: f64,
) -> Vec<GroupedRun> {
    let mut runs = Vec::new();
    let span = annotation_span(layout, doc, inline_root_id, scale);
    for (line_index, line) in layout.lines().enumerate() {
        let top = annotation_free_top(&line, doc, scale);
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };
            let owner = glyph_run.style().brush.id;
            let left = f64::from(glyph_run.offset());
            let right = left + f64::from(glyph_run.advance());
            let mut run = GroupedRun {
                line: line_index,
                left,
                right,
                group: None,
                shift: Vec2::ZERO,
                cuts: false,
                top,
                span,
            };
            if is_invisible(doc, owner) {
                runs.push(GroupedRun { cuts: true, ..run });
                continue;
            }
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
                if run.group.is_none() && starts_group(&styles) {
                    run.group = Some(node.id);
                }
            }
            run.shift = shift * scale;
            runs.push(run);
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
        let line_runs = || later.iter().take(count);
        let top = line_runs()
            .find_map(|run| run.top)
            .map_or(rect.y0, |top| rect.y0.max(top));
        let covers_line = layout.get(line_idx).is_some_and(|line| {
            let range = line.text_range();
            selection_start <= range.start && range.end <= selection_end
        });
        let (x0, x1) = match line_runs().find_map(|run| run.span) {
            Some((start, end)) if covers_line => (rect.x0.min(start), rect.x1.max(end)),
            _ => (rect.x0, rect.x1),
        };
        let mut fill = |left: f64, right: f64, shift: Vec2| {
            if left < right {
                let rect = kurbo::Rect::new(left, top, right, rect.y1);
                let transform = transform * Affine::translate(shift);
                scene.fill(Fill::NonZero, transform, SELECTION_COLOR, None, &rect);
            }
        };
        match paint.group {
            None => {
                let mut cuts: Vec<(f64, f64)> = line_runs()
                    .filter(|run| run.cuts || run.group.is_some())
                    .map(|run| (run.left, run.right))
                    .collect();
                cuts.sort_by(|first, second| first.0.total_cmp(&second.0));
                let mut left = x0;
                for (cut_left, cut_right) in cuts {
                    fill(left, cut_left.min(x1), Vec2::ZERO);
                    left = left.max(cut_right);
                }
                fill(left, x1, Vec2::ZERO);
            }
            Some(group) => {
                for run in line_runs().filter(|run| run.group == Some(group)) {
                    fill(rect.x0.max(run.left), rect.x1.min(run.right), run.shift);
                }
            }
        }
    });
}
