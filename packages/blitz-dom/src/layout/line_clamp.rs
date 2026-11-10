use blitz_traits::node_id::NodeId;
use style::computed_values::float::T as Float;
use stylo_taffy::CopyStyleGetters;
use taffy::{CoreStyle, Display, LayoutInput, LayoutOutput, ResolveOrZero};

use crate::{BaseDocument, layout::resolve_calc_value, node::LineBudget, util};

struct ClampWalk {
    max_lines: usize,
    scale: f32,
    seen: usize,
    cut: Option<ClampCut>,
    cut_boxes: Vec<(NodeId, f32)>,
}

struct ClampCut {
    root: NodeId,
    lines: usize,
    bottom: f32,
}

impl ClampWalk {
    fn note_cut_box(&mut self, had_cut: bool, box_id: NodeId, top: f32, trailing: f32) {
        if let Some(cut) = self.cut.as_ref().filter(|_| !had_cut) {
            self.cut_boxes.push((box_id, cut.bottom - trailing - top));
        }
    }
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
        let bottom_inset = style
            .padding()
            .resolve_or_zero(inputs.parent_size.width, resolve_calc_value)
            .bottom
            + style
                .border()
                .resolve_or_zero(inputs.parent_size.width, resolve_calc_value)
                .bottom;
        drop(style);

        let mut walk = ClampWalk {
            max_lines,
            scale: self.viewport.scale(),
            seen: 0,
            cut: None,
            cut_boxes: Vec::new(),
        };
        self.count_clamped_lines(&mut walk, node_id, 0.0, 0.0);
        let Some(cut) = walk.cut.filter(|_| walk.seen > max_lines) else {
            return;
        };
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
            let size = &mut self.nodes[box_id].unrounded_layout_mut().size;
            size.height = size.height.min(height);
        }
        if height_is_auto {
            output.size.height = output.size.height.min(cut.bottom + bottom_inset);
        }
    }

    fn count_clamped_lines(
        &mut self,
        walk: &mut ClampWalk,
        node_id: NodeId,
        top: f32,
        trailing: f32,
    ) {
        let Some(children) = self.nodes[node_id].layout_children.borrow().clone() else {
            return;
        };
        for child_id in children {
            let child = &self.nodes[child_id];
            let display = child.taffy_display();
            let is_floated = child
                .primary_styles()
                .is_some_and(|style| style.clone_float() != Float::None);
            if display == Display::None || is_floated || child.is_out_of_flow() {
                continue;
            }
            let layout = *child.unrounded_layout();
            let child_top = top + layout.location.y;
            let child_trailing = trailing + layout.padding.bottom + layout.border.bottom;
            let had_cut = walk.cut.is_some();
            if !child.flags.is_inline_root() {
                if matches!(display, Display::Block | Display::FlowRoot) {
                    self.count_clamped_lines(walk, child_id, child_top, child_trailing);
                    walk.note_cut_box(had_cut, child_id, child_top, trailing);
                }
                continue;
            }
            let Some(text_layout) = self.nodes[child_id]
                .element_data_mut()
                .and_then(|element| element.inline_layout_data.as_mut())
            else {
                continue;
            };
            text_layout.line_budget = None;
            let line_count = util::line_count(&text_layout.layout);
            if walk.cut.is_none()
                && let Some(index) = (walk.max_lines - walk.seen).checked_sub(1)
                && let Some(line) = text_layout.layout.get(index).filter(|_| index < line_count)
            {
                let content_top =
                    child_top + layout.border.top + layout.padding.top + text_layout.block_offset;
                walk.cut = Some(ClampCut {
                    root: child_id,
                    lines: index + 1,
                    bottom: content_top
                        + line.metrics().block_max_coord / walk.scale
                        + child_trailing,
                });
            }
            walk.seen += line_count;
            walk.note_cut_box(had_cut, child_id, child_top, trailing);
        }
    }
}
