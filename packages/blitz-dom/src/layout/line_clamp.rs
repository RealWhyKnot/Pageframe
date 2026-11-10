use blitz_traits::node_id::NodeId;
use style::computed_values::float::T as Float;
use stylo_taffy::CopyStyleGetters;
use taffy::{
    BoxSizing, CoreStyle, Display, LayoutInput, LayoutOutput, MaybeResolve, ResolveOrZero, RunMode,
};

use crate::{BaseDocument, layout::resolve_calc_value, node::LineBudget, util};

struct ClampWalk {
    container: NodeId,
    keeps_result: bool,
    max_lines: usize,
    scale: f32,
    seen: usize,
    cut: Option<ClampCut>,
    cut_boxes: Vec<(NodeId, f32)>,
    shifted_boxes: Vec<(NodeId, f32)>,
}

struct ClampCut {
    root: NodeId,
    lines: usize,
    bottom: f32,
}

impl BaseDocument {
    pub(crate) fn clamp_child_lines(
        &mut self,
        node_id: NodeId,
        inputs: &LayoutInput,
        output: &mut LayoutOutput,
    ) {
        let node = &self.nodes[node_id];
        let Some(styles) = node.primary_styles() else {
            return;
        };
        let Some(max_lines) = util::max_clamped_lines(&styles) else {
            return;
        };
        let height_is_auto =
            styles.get_position().height.is_auto() && inputs.known_dimensions.height.is_none();
        drop(styles);
        let style = node.layout_style();
        let inset = style
            .padding()
            .resolve_or_zero(inputs.parent_size.width, resolve_calc_value)
            + style
                .border()
                .resolve_or_zero(inputs.parent_size.width, resolve_calc_value);
        let min_height = style
            .min_size()
            .maybe_resolve(inputs.parent_size, resolve_calc_value)
            .height
            .map(|height| match style.box_sizing() {
                BoxSizing::ContentBox => height + inset.top + inset.bottom,
                BoxSizing::BorderBox => height,
            })
            .unwrap_or(0.0);
        drop(style);

        let keeps_result = inputs.run_mode == RunMode::PerformLayout;
        let mut walk = ClampWalk {
            container: node_id,
            keeps_result,
            max_lines,
            scale: self.viewport.scale(),
            seen: 0,
            cut: None,
            cut_boxes: Vec::new(),
            shifted_boxes: Vec::new(),
        };
        self.count_clamped_lines(&mut walk, node_id, 0.0);
        let Some(cut) = walk.cut.filter(|_| walk.seen > max_lines) else {
            return;
        };
        if keeps_result {
            if let Some(text_layout) = self.nodes[cut.root]
                .element_data_mut()
                .and_then(|element| element.inline_layout_data.as_mut())
            {
                text_layout.line_budget = Some(LineBudget {
                    container: node_id,
                    lines: cut.lines,
                });
            }
            for (box_id, height) in walk.cut_boxes {
                self.nodes[box_id].unrounded_layout_mut().size.height = height;
            }
            for (box_id, shift) in walk.shifted_boxes {
                self.nodes[box_id].unrounded_layout_mut().location.y -= shift;
            }
        }
        if height_is_auto {
            let clamped_height = (cut.bottom + inset.bottom).max(min_height);
            output.size.height = output.size.height.min(clamped_height);
        }
    }

    fn drop_line_budget(&mut self, walk: &ClampWalk, node_id: NodeId) {
        if let Some(text_layout) = self.nodes[node_id]
            .element_data_mut()
            .and_then(|element| element.inline_layout_data.as_mut())
            && walk.keeps_result
            && text_layout
                .line_budget
                .is_some_and(|budget| budget.container == walk.container)
        {
            text_layout.line_budget = None;
        }
    }

    fn clamp_line_bottom(&self, walk: &ClampWalk, node_id: NodeId) -> Option<(usize, f32, usize)> {
        let node = &self.nodes[node_id];
        let text_layout = node.element_data()?.inline_layout_data.as_ref()?;
        let line_count = util::line_count(&text_layout.layout);
        let layout = node.unrounded_layout();
        let content_top = layout.border.top + layout.padding.top + text_layout.block_offset;
        let bottom = walk
            .max_lines
            .checked_sub(walk.seen + 1)
            .filter(|&index| walk.cut.is_none() && index < line_count)
            .and_then(|index| text_layout.layout.get(index).map(|line| (index, line)))
            .map(|(index, line)| {
                (
                    index + 1,
                    content_top + line.metrics().block_max_coord / walk.scale,
                )
            });
        let (lines, bottom) = bottom.unzip();
        Some((line_count, bottom.unwrap_or(0.0), lines.unwrap_or(0)))
    }

    fn count_clamped_lines(&mut self, walk: &mut ClampWalk, node_id: NodeId, top: f32) {
        let Some(children) = self.nodes[node_id].layout_children.borrow().clone() else {
            return;
        };
        let mut shift = 0.0;
        for child_id in children {
            let child = &self.nodes[child_id];
            let display = child.taffy_display();
            let is_floated = child
                .primary_styles()
                .is_some_and(|style| style.clone_float() != Float::None);
            if display != Display::Block || is_floated || child.is_out_of_flow() {
                self.drop_line_budget(walk, child_id);
                continue;
            }
            if shift > 0.0 {
                walk.shifted_boxes.push((child_id, shift));
            }
            let layout = *child.unrounded_layout();
            let child_top = top + layout.location.y;
            let bottom_inset = layout.padding.bottom + layout.border.bottom;
            let height_is_auto = child
                .primary_styles()
                .is_none_or(|style| style.get_position().height.is_auto());
            let had_cut = walk.cut.is_some();
            let content_bottom = if child.flags.is_inline_root() {
                self.drop_line_budget(walk, child_id);
                let Some((line_count, bottom, lines)) = self.clamp_line_bottom(walk, child_id)
                else {
                    continue;
                };
                walk.seen += line_count;
                if lines > 0 {
                    walk.cut = Some(ClampCut {
                        root: child_id,
                        lines,
                        bottom: 0.0,
                    });
                }
                child_top + bottom
            } else {
                self.count_clamped_lines(walk, child_id, child_top);
                walk.cut.as_ref().map_or(0.0, |cut| cut.bottom)
            };
            if let Some(cut) = walk.cut.as_mut().filter(|_| !had_cut) {
                let box_bottom = child_top + layout.size.height;
                cut.bottom = if height_is_auto {
                    box_bottom.min(content_bottom + bottom_inset)
                } else {
                    box_bottom
                };
                walk.cut_boxes.push((child_id, cut.bottom - child_top));
                shift = box_bottom - cut.bottom;
            }
        }
    }
}
