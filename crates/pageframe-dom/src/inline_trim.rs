use crate::node::{Node, TextBrush};
use crate::util::{INLINE_BREAK, INLINE_EDGE_END, INLINE_MARGIN_END};
use num_traits::Float;
use parley::{GlyphRun, InlineBoxKind, Line, PositionedLayoutItem};
use smallvec::SmallVec;
use style::computed_values::white_space_collapse::T as WhiteSpaceCollapse;

pub fn is_end_mark(character: char) -> bool {
    INLINE_EDGE_END.contains(character)
        || INLINE_MARGIN_END.contains(character)
        || INLINE_BREAK.contains(character)
}

pub fn collapses_spaces(owner: &Node) -> bool {
    owner.primary_styles().is_some_and(|styles| {
        matches!(
            styles.get_inherited_text().white_space_collapse,
            WhiteSpaceCollapse::Collapse | WhiteSpaceCollapse::PreserveBreaks
        )
    })
}

pub fn hung_space(
    glyph_run: &GlyphRun<'_, TextBrush>,
    run_text: &str,
    owner: &Node,
) -> Option<(f32, f32)> {
    let body = run_text.trim_end_matches(is_end_mark);
    let marks = run_text.get(body.len()..)?.chars().count();
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineTrim {
    inked_left: f32,
    inked_right: f32,
    closing_space: Option<(f32, f32)>,
    is_rtl: bool,
}

impl LineTrim {
    pub fn new(root: &Node, line: &Line<'_, TextBrush>, text: &str, is_rtl: bool) -> Self {
        let mut line_left = f32::MAX;
        let mut line_right = f32::MIN;
        let mut hung = None;
        let mut is_hung_closed = false;
        let mut edge_owner: Option<&Node> = None;
        for item in line.items() {
            let (left, right) = match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => {
                    let owner = root.with(glyph_run.style().brush.id);
                    let run_text = text.get(glyph_run.run().text_range()).unwrap_or_default();
                    let body = run_text.trim_end_matches(is_end_mark);
                    if body.is_empty() {
                        is_hung_closed = true;
                    } else {
                        hung = hung_space(&glyph_run, run_text, owner);
                        is_hung_closed = body.len() < run_text.len();
                        if !(is_rtl && edge_owner.is_some()) {
                            edge_owner = Some(owner);
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
        Self {
            inked_left: line_left + hanging,
            inked_right: line_right - hanging,
            closing_space: hung.filter(|_| is_hung_closed),
            is_rtl,
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
