use super::{ElementCx, SPAN_GROUP_REACH};
use crate::text::{GroupedRun, InlineFragment, LineEllipsis, TextPaintOptions};
use anyrender::PaintScene;
use kurbo::{Affine, Rect};
use pageframe_dom::NodeId;
use parley::PositionedLayoutItem;
use std::collections::HashMap;
use std::rc::Rc;

type LineSpan = Option<(usize, usize)>;

pub(super) struct InlinePaint {
    visible_lines: usize,
    fragments: Vec<InlineFragment>,
    runs: Vec<GroupedRun>,
    groups: Vec<PaintGroup>,
    ungrouped: LineSpan,
}

#[derive(Clone, Copy)]
struct InlinePass<'a> {
    fragments: &'a [InlineFragment],
    runs: &'a [GroupedRun],
    visible_lines: usize,
    ellipsis: Option<LineEllipsis>,
}

struct PaintGroup {
    id: NodeId,
    parent: Option<NodeId>,
    bounds: Rect,
    lines: LineSpan,
    children: Vec<usize>,
}

fn widen(lines: &mut LineSpan, line: usize) {
    *lines = Some(lines.map_or((line, line), |(first, last)| {
        (first.min(line), last.max(line))
    }));
}

impl ElementCx<'_, '_> {
    pub(super) fn draw_inline_pass(
        &self,
        scene: &mut impl PaintScene,
        transform: Affine,
        visible_lines: usize,
        ellipsis: Option<LineEllipsis>,
        only: Option<NodeId>,
    ) {
        let Some(paint) = self.inline_paint(visible_lines) else {
            return;
        };
        let pass = InlinePass {
            fragments: &paint.fragments,
            runs: &paint.runs,
            visible_lines,
            ellipsis,
        };
        if only.is_none() {
            self.draw_inline_group(scene, transform, pass, None, paint.ungrouped);
        }
        let is_drawn = |group: &&PaintGroup| match only {
            Some(layer) => group.id == layer,
            None => !self.node.flags.hoists_inline_layers(),
        };
        for group in paint
            .groups
            .iter()
            .filter(|group| group.parent.is_none())
            .filter(is_drawn)
        {
            self.draw_paint_group(scene, transform, pass, &paint.groups, group);
        }
    }

    fn inline_paint(&self, visible_lines: usize) -> Option<Rc<InlinePaint>> {
        let cached = self
            .context
            .inline_paints
            .borrow()
            .get(&self.node.id)
            .filter(|paint| paint.visible_lines == visible_lines)
            .cloned();
        if cached.is_some() {
            return cached;
        }
        let text_layout = self.element.inline_layout_data.as_ref()?;
        let mut fragments = crate::text::inline_fragments(
            text_layout.layout.lines().take(visible_lines),
            self.context.dom,
            self.node.id,
            &text_layout.text,
            self.scale,
        );
        let (groups, ungrouped) = self.paint_groups(&fragments, visible_lines);
        fragments.retain(|fragment| !crate::text::is_invisible(self.context.dom, fragment.node_id));
        let runs = if self.context.selection_ranges.contains_key(&self.node.id) {
            crate::text::grouped_runs(
                &text_layout.layout,
                self.context.dom,
                self.node.id,
                self.scale,
            )
        } else {
            Vec::new()
        };
        let paint = Rc::new(InlinePaint {
            visible_lines,
            fragments,
            runs,
            groups,
            ungrouped,
        });
        self.context
            .inline_paints
            .borrow_mut()
            .insert(self.node.id, Rc::clone(&paint));
        Some(paint)
    }

    fn paint_groups(
        &self,
        fragments: &[InlineFragment],
        visible_lines: usize,
    ) -> (Vec<PaintGroup>, LineSpan) {
        let dom = self.context.dom;
        let mut groups: Vec<PaintGroup> = Vec::new();
        let mut index: HashMap<NodeId, usize> = HashMap::new();
        for fragment in fragments {
            let mut next_group = fragment.group;
            while let Some(group) = next_group {
                if let Some(&known) = index.get(&group) {
                    let known = &mut groups[known];
                    known.bounds = known.bounds.union(fragment.rect);
                    next_group = known.parent;
                    continue;
                }
                let parent = crate::text::parent_group(dom, group, self.node.id);
                index.insert(group, groups.len());
                groups.push(PaintGroup {
                    id: group,
                    parent,
                    bounds: fragment.rect,
                    lines: None,
                    children: Vec::new(),
                });
                next_group = parent;
            }
        }
        for child in 0..groups.len() {
            if let Some(&parent) = groups[child].parent.and_then(|parent| index.get(&parent)) {
                groups[parent].children.push(child);
            }
        }

        let mut ungrouped = None;
        let Some(text_layout) = self.element.inline_layout_data.as_ref() else {
            return (groups, ungrouped);
        };
        let mut owners: HashMap<NodeId, Option<Option<usize>>> = HashMap::new();
        for (line_index, line) in text_layout.layout.lines().take(visible_lines).enumerate() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let brush = glyph_run.style().brush.id;
                let owner = *owners.entry(brush).or_insert_with(|| {
                    crate::text::run_group(dom, brush, self.node.id)
                        .map(|group| index.get(&group).copied())
                });
                match owner {
                    None => ungrouped = Some((0, line_index)),
                    Some(Some(group)) => widen(&mut groups[group].lines, line_index),
                    Some(None) => {}
                }
            }
        }
        (groups, ungrouped)
    }

    fn draw_paint_group(
        &self,
        scene: &mut impl PaintScene,
        transform: Affine,
        pass: InlinePass<'_>,
        groups: &[PaintGroup],
        group: &PaintGroup,
    ) {
        let Some(opacity) = self
            .context
            .dom
            .get_node(group.id)
            .and_then(|node| node.primary_styles())
            .map(|style| style.get_effects().opacity)
        else {
            return;
        };
        let reach = SPAN_GROUP_REACH * self.scale;
        self.context.layer_manager.maybe_with_layer(
            scene,
            opacity < 1.0,
            peniko::Mix::Normal.into(),
            opacity,
            transform,
            &group.bounds.inflate(reach, reach),
            None,
            None,
            |scene| {
                self.draw_inline_group(scene, transform, pass, Some(group.id), group.lines);
                for &child in &group.children {
                    self.draw_paint_group(scene, transform, pass, groups, &groups[child]);
                }
            },
        );
    }

    fn draw_inline_group(
        &self,
        scene: &mut impl PaintScene,
        transform: Affine,
        pass: InlinePass<'_>,
        group: Option<NodeId>,
        lines: LineSpan,
    ) {
        let InlinePass {
            fragments,
            runs,
            visible_lines,
            ellipsis,
        } = pass;
        let Some(text_layout) = self.element.inline_layout_data.as_ref() else {
            return;
        };
        for fragment in fragments.iter().filter(|fragment| fragment.group == group) {
            self.draw_inline_fragment(scene, transform, fragment);
        }
        if let Some(&range) = self.context.selection_ranges.get(&self.node.id) {
            crate::text::draw_text_selection(
                scene,
                &text_layout.layout,
                transform,
                crate::text::SelectionPaint {
                    range,
                    visible_lines,
                    runs,
                    group,
                },
            );
        }
        let Some((first, last)) = lines else {
            return;
        };
        let ellipsis = ellipsis.map(|ellipsis| LineEllipsis {
            on_line: ellipsis.on_line.and_then(|line| line.checked_sub(first)),
            ..ellipsis
        });
        let mut draw_text_context = self.context.draw_text_context.borrow_mut();
        crate::text::stroke_text(
            scene,
            text_layout
                .layout
                .lines()
                .skip(first)
                .take(last + 1 - first),
            self.context.dom,
            transform,
            TextPaintOptions {
                scale: self.scale,
                brush_override: None,
                ellipsis,
                text: Some(&text_layout.text),
                group,
                flat_ancestors: false,
            },
            self.node.id,
            &mut draw_text_context,
        );
    }
}
