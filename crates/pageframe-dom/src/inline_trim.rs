use crate::node::{Node, TextBrush};
use crate::util::{INLINE_BREAK, INLINE_EDGE_END, INLINE_MARGIN_END, inline_clone_insets};
use num_traits::Float;
use pageframe_traits::node_id::NodeId;
use parley::{GlyphRun, InlineBoxKind, Line, PositionedLayoutItem};
use smallvec::SmallVec;
use style::computed_values::white_space_collapse::T as WhiteSpaceCollapse;

fn is_end_mark(line: &Line<'_, TextBrush>, offset: usize, character: char) -> bool {
    (INLINE_EDGE_END.contains(character)
        || INLINE_MARGIN_END.contains(character)
        || INLINE_BREAK.contains(character))
        && line.is_inline_end_mark_at(offset)
}

pub fn collapses_spaces(owner: &Node) -> bool {
    owner.primary_styles().is_some_and(|styles| {
        matches!(
            styles.get_inherited_text().white_space_collapse,
            WhiteSpaceCollapse::Collapse | WhiteSpaceCollapse::PreserveBreaks
        )
    })
}

pub fn is_reordered_preserved_space(
    root: &Node,
    glyph_run: &GlyphRun<'_, TextBrush>,
    text: &str,
) -> bool {
    let run = glyph_run.run();
    run.is_rtl() != run.is_line_rtl()
        && glyph_run.advance() > 0.0
        && text
            .get(glyph_run.text_range())
            .is_some_and(|text| !text.is_empty() && text.bytes().all(|byte| byte == b' '))
        && root
            .tree()
            .get(glyph_run.style().brush.id)
            .is_some_and(|owner| !collapses_spaces(owner))
}

fn hung_space(
    glyph_run: &GlyphRun<'_, TextBrush>,
    body: &str,
    marks: usize,
    owner: &Node,
) -> Option<(f32, f32)> {
    let is_blank = body.chars().all(|ch| ch == ' ');
    if !body.ends_with(' ') || (glyph_run.run().is_rtl() && !is_blank) {
        return None;
    }
    if !collapses_spaces(owner) {
        return None;
    }
    if is_blank && marks == 0 {
        return Some((glyph_run.offset(), glyph_run.advance()));
    }
    let glyphs: SmallVec<[(f32, f32); 16]> = glyph_run
        .positioned_glyphs()
        .map(|glyph| (glyph.x, glyph.advance))
        .collect();
    let (left, width) = *glyphs.get(glyphs.len().checked_sub(marks + 1)?)?;
    (width > 0.0).then_some((left, width))
}

fn trailing_space_advance(
    line: &Line<'_, TextBrush>,
    glyph_run: &GlyphRun<'_, TextBrush>,
    text: &str,
) -> f32 {
    glyph_run.run().clusters().fold(0.0, |sum, cluster| {
        let range = cluster.text_range();
        let start = range.start;
        let chars = text.get(range).unwrap_or_default();
        if chars == " " {
            sum + cluster.advance()
        } else if chars
            .char_indices()
            .all(|(offset, character)| is_end_mark(line, start + offset, character))
        {
            sum
        } else {
            0.0
        }
    })
}

const HUNG_SLACK: f32 = 1.0 / 128.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineTrim {
    inked_left: f32,
    inked_right: f32,
    hanging: f32,
    closing_space: Option<(f32, f32)>,
    trailing_space: f32,
    is_rtl: bool,
}

impl LineTrim {
    pub fn new(root: &Node, line: &Line<'_, TextBrush>, text: &str) -> Self {
        let is_rtl = line.is_rtl();
        let mut line_left = f32::MAX;
        let mut line_right = f32::MIN;
        let mut hung = None;
        let mut is_hung_closed = false;
        let mut edge_run = None;
        let mut edge_owner: Option<&Node> = None;
        for item in line.items() {
            let (left, right) = match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => {
                    let owner = root.tree().get(glyph_run.style().brush.id);
                    let range = glyph_run.run().text_range();
                    let start = range.start;
                    let run_text = text.get(range).unwrap_or_default();
                    let body_end = run_text
                        .char_indices()
                        .rev()
                        .find(|&(offset, character)| !is_end_mark(line, start + offset, character))
                        .map_or(0, |(offset, character)| offset + character.len_utf8());
                    let body = &run_text[..body_end];
                    if body.is_empty() {
                        is_hung_closed = true;
                    } else {
                        let marks = run_text[body_end..].chars().count();
                        hung = owner.and_then(|owner| hung_space(&glyph_run, body, marks, owner));
                        is_hung_closed = body.len() < run_text.len();
                        if !(is_rtl && edge_owner.is_some()) {
                            edge_owner = owner;
                            edge_run = Some(glyph_run.clone());
                        }
                    }
                    let left = glyph_run.offset();
                    (left, left + glyph_run.advance())
                }
                PositionedLayoutItem::InlineBox(inline_box)
                    if inline_box.kind == InlineBoxKind::InFlow =>
                {
                    hung = None;
                    (inline_box.x, inline_box.x + inline_box.width)
                }
                PositionedLayoutItem::InlineBox(_) => continue,
            };
            line_left = line_left.min(left);
            line_right = line_right.max(right);
        }
        let hanging = if edge_owner.is_some_and(|owner| !collapses_spaces(owner)) {
            0.0
        } else {
            line.metrics().hanging_advance
        };
        let trailing_space = edge_run
            .filter(|_| hanging > 0.0)
            .map_or(0.0, |run| trailing_space_advance(line, &run, text));
        Self {
            inked_left: line_left + hanging,
            inked_right: line_right - hanging,
            hanging,
            closing_space: hung.filter(|_| is_hung_closed),
            trailing_space,
            is_rtl,
        }
    }

    pub fn is_hung(&self, left: f32, right: f32) -> bool {
        if self.hanging <= 0.0 {
            return false;
        }
        if self.is_rtl {
            right <= self.inked_left + HUNG_SLACK
        } else {
            left >= self.inked_right - HUNG_SLACK
        }
    }

    pub fn hung_span(&self) -> Option<(f32, f32)> {
        if self.hanging <= 0.0 || self.trailing_space + HUNG_SLACK < self.hanging {
            return None;
        }
        Some(if self.is_rtl {
            (self.inked_left - self.hanging, self.inked_left)
        } else {
            (self.inked_right, self.inked_right + self.hanging)
        })
    }

    fn hung_edge(&self) -> f32 {
        if self.is_rtl {
            self.inked_left
        } else {
            self.inked_right
        }
    }

    pub fn clamp<T: Float + From<f32>>(&self, left: T, right: T, is_end: bool) -> (T, T) {
        let lift = <T as From<f32>>::from;
        if self.is_rtl {
            if is_end {
                return (left, right);
            }
            return (left.max(lift(self.inked_left)).min(right), right);
        }
        let mut right = right;
        if let Some((space_left, space_width)) = self.closing_space
            && right > lift(space_left)
        {
            right = (right - lift(space_width)).max(left);
        }
        if !is_end {
            right = right.min(lift(self.inked_right)).max(left);
        }
        (left, right)
    }
}

#[derive(Clone, Copy)]
pub struct LineSpan<T> {
    pub node_id: NodeId,
    pub left: T,
    pub right: T,
    pub is_start: bool,
    pub is_end: bool,
    pub has_reordered_space: bool,
}

impl<T: Float + From<f32>> LineSpan<T> {
    pub fn continues(&self, left: T, is_reordered_space: bool) -> bool {
        !(self.has_reordered_space || is_reordered_space)
            || left <= self.right + <T as From<f32>>::from(HUNG_SLACK)
    }
}

fn grow<T: Float + From<f32>>(left: T, right: T, start: f32, end: f32, is_rtl: bool) -> (T, T) {
    let lift = <T as From<f32>>::from;
    if is_rtl {
        (left - lift(end), right + lift(start))
    } else {
        (left - lift(start), right + lift(end))
    }
}

pub fn settle_line_spans<T: Float + From<f32>>(
    root: &Node,
    spans: &mut [LineSpan<T>],
    trim: &LineTrim,
    basis: f32,
    scale: f32,
) {
    let tree = root.tree();
    let depth_of = |id: NodeId| {
        let mut depth = 0_usize;
        let mut cursor = tree.get(id);
        while let Some(node) = cursor.filter(|node| node.id != root.id) {
            depth += 1;
            cursor = node.parent.and_then(|parent| tree.get(parent));
        }
        depth
    };
    for span in spans.iter_mut() {
        if span.left > span.right {
            let edge = <T as From<f32>>::from(trim.hung_edge());
            (span.left, span.right) = (edge, edge);
        }
        (span.left, span.right) = trim.clamp(span.left, span.right, span.is_end);
    }
    let mut order: SmallVec<[(usize, usize); 8]> = spans
        .iter()
        .enumerate()
        .map(|(index, span)| (depth_of(span.node_id), index))
        .collect();
    order.sort_by(|first, second| second.cmp(first));
    let mut grown: SmallVec<[bool; 8]> = SmallVec::from_elem(false, spans.len());
    for (_, index) in order {
        let span = spans[index];
        let node = tree.get(span.node_id);
        let outer = match node.and_then(|node| inline_clone_insets(node, basis)) {
            Some(insets) => {
                let needs_edge = |is_left, has_edge: fn(&LineSpan<T>) -> bool| {
                    !spans.iter().enumerate().any(|(at, peer)| {
                        peer.node_id == span.node_id
                            && (has_edge(peer) || if is_left { at < index } else { at > index })
                    })
                };
                let clone_start = needs_edge(!insets.is_rtl, |peer| peer.is_start);
                let clone_end = needs_edge(insets.is_rtl, |peer| peer.is_end);
                let start_scale = if clone_start { scale } else { 0.0 };
                let end_scale = if clone_end { scale } else { 0.0 };
                let (left, right) = (span.left, span.right);
                let fragment = &mut spans[index];
                (fragment.left, fragment.right) = grow(
                    left,
                    right,
                    start_scale * insets.start(),
                    end_scale * insets.end(),
                    insets.is_rtl,
                );
                fragment.is_start |= clone_start;
                fragment.is_end |= clone_end;
                grow(
                    left,
                    right,
                    start_scale * insets.outer_start(),
                    end_scale * insets.outer_end(),
                    insets.is_rtl,
                )
            }
            None if grown[index] => (span.left, span.right),
            None => continue,
        };
        let mut cursor = node.and_then(|node| node.parent);
        while let Some(parent) = cursor.filter(|parent| *parent != root.id) {
            let mut fragments = spans
                .iter()
                .enumerate()
                .filter(|(_, fragment)| fragment.node_id == parent);
            let first = fragments.next();
            let at = first
                .into_iter()
                .chain(fragments)
                .find(|(_, fragment)| fragment.left <= span.right && span.left <= fragment.right)
                .or(first);
            if let Some((at, _)) = at {
                let ancestor = &mut spans[at];
                ancestor.left = ancestor.left.min(outer.0);
                ancestor.right = ancestor.right.max(outer.1);
                grown[at] = true;
                break;
            }
            cursor = tree.get(parent).and_then(|node| node.parent);
        }
    }
}
