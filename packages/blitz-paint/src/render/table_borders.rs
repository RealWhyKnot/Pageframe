use std::cmp::Ordering;
use std::collections::HashMap;
use std::f64::consts::TAU;
use std::ops::Range;
use std::rc::Rc;

use anyrender::PaintScene;
use blitz_dom::CopyStyleGetters as _;
use blitz_dom::node::SpecialElementData;
use blitz_dom::{BaseDocument, Node};
use blitz_traits::node_id::NodeId;
use kurbo::{Affine, BezPath, Point, Rect, Vec2};
use peniko::{Color, Fill};
use style::{
    computed_values::border_collapse::T as BorderCollapse,
    computed_values::direction::T as Direction, computed_values::position::T as Position,
    computed_values::visibility::T as Visibility, properties::ComputedValues,
    values::computed::BorderStyle, values::specified::box_::DisplayInside,
};

use crate::{
    color::ToColorColor as _,
    kurbo_css::Edge,
    render::{ElementCx, border::beveled_edge_color},
};

const DOT_SLACK: f64 = 0.01;
const COVER_SLACK: f64 = 0.01;
const CURVE_TOLERANCE: f64 = 0.001;
const ARC_ERROR: f64 = 0.060_660_171_779_821_29;
const ARC_TOLERANCE: f64 = 0.25;
const MOST_ARCS: u32 = 128;
const MOST_PIECES: u32 = 64;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum LineStyle {
    Groove,
    Ridge,
    Dotted,
    Dashed,
    Solid,
    Double,
}

#[derive(Clone, Copy)]
struct Line {
    style: LineStyle,
    width: f64,
    color: Color,
    order: usize,
}

#[derive(Clone, Copy)]
enum Slot {
    Empty,
    Blocked,
    Hidden,
    Line(Line),
}

impl Slot {
    fn line(self) -> Option<Line> {
        match self {
            Slot::Line(line) if line.width > 0.0 => Some(line),
            _ => None,
        }
    }

    fn width(self) -> f64 {
        match self {
            Slot::Line(line) => line.width,
            _ => 0.0,
        }
    }

    fn yields_to(self, source: Slot) -> bool {
        match (self, source) {
            (Slot::Blocked, _) | (_, Slot::Empty | Slot::Blocked) => false,
            (Slot::Empty, _) | (_, Slot::Hidden) => true,
            (Slot::Hidden, _) => false,
            (Slot::Line(old), Slot::Line(new)) => {
                new.width > old.width || (new.width == old.width && new.style > old.style)
            }
        }
    }
}

fn contest(first: Slot, second: Slot) -> Ordering {
    match (first.line(), second.line()) {
        (Some(first), Some(second)) => first
            .width
            .total_cmp(&second.width)
            .then(first.style.cmp(&second.style))
            .then(second.order.cmp(&first.order)),
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
    }
}

fn device_width(width: f64) -> f64 {
    if width <= 0.0 {
        0.0
    } else {
        (width + 1.0e-4).floor().max(1.0)
    }
}

fn side_slot(style: &ComputedValues, side: Edge, scale: f64, order: usize) -> Slot {
    let border = style.get_border();
    let (line_style, width, color) = match side {
        Edge::Top => (
            border.border_top_style,
            &border.border_top_width,
            &border.border_top_color,
        ),
        Edge::Right => (
            border.border_right_style,
            &border.border_right_width,
            &border.border_right_color,
        ),
        Edge::Bottom => (
            border.border_bottom_style,
            &border.border_bottom_width,
            &border.border_bottom_color,
        ),
        Edge::Left => (
            border.border_left_style,
            &border.border_left_width,
            &border.border_left_color,
        ),
    };
    let style_kind = match line_style {
        BorderStyle::None => return Slot::Empty,
        BorderStyle::Hidden => return Slot::Hidden,
        BorderStyle::Outset | BorderStyle::Groove => LineStyle::Groove,
        BorderStyle::Inset | BorderStyle::Ridge => LineStyle::Ridge,
        BorderStyle::Dotted => LineStyle::Dotted,
        BorderStyle::Dashed => LineStyle::Dashed,
        BorderStyle::Solid => LineStyle::Solid,
        BorderStyle::Double => LineStyle::Double,
    };
    Slot::Line(Line {
        style: style_kind,
        width: device_width(f64::from(width.0.to_f32_px()) * scale),
        color: color
            .resolve_to_absolute(&style.clone_color())
            .as_srgb_color(),
        order,
    })
}

fn covers_background(style: &ComputedValues) -> bool {
    [Edge::Top, Edge::Right, Edge::Bottom, Edge::Left]
        .into_iter()
        .all(|side| match side_slot(style, side, 1.0, 0) {
            Slot::Line(line) => {
                line.width > 0.0
                    && line.color.components[3] >= 1.0
                    && matches!(
                        line.style,
                        LineStyle::Solid | LineStyle::Groove | LineStyle::Ridge
                    )
            }
            _ => false,
        })
}

struct Sides {
    top: Slot,
    bottom: Slot,
    start: Slot,
    end: Slot,
}

impl Sides {
    fn new(style: &ComputedValues, is_rtl: bool, scale: f64, order: usize) -> Self {
        let (start, end) = if is_rtl {
            (Edge::Right, Edge::Left)
        } else {
            (Edge::Left, Edge::Right)
        };
        Self {
            top: side_slot(style, Edge::Top, scale, order),
            bottom: side_slot(style, Edge::Bottom, scale, order),
            start: side_slot(style, start, scale, order),
            end: side_slot(style, end, scale, order),
        }
    }
}

struct Grid {
    columns: usize,
    rows: usize,
    across: Vec<Slot>,
    down: Vec<Slot>,
}

impl Grid {
    fn new(columns: usize, rows: usize) -> Self {
        Self {
            columns,
            rows,
            across: vec![Slot::Empty; columns * (rows + 1)],
            down: vec![Slot::Empty; (columns + 1) * rows],
        }
    }

    fn across(&self, row: Option<usize>, column: Option<usize>) -> Slot {
        match (row, column) {
            (Some(row), Some(column)) if row <= self.rows && column < self.columns => {
                self.across[row * self.columns + column]
            }
            _ => Slot::Empty,
        }
    }

    fn down(&self, row: Option<usize>, column: Option<usize>) -> Slot {
        match (row, column) {
            (Some(row), Some(column)) if row < self.rows && column <= self.columns => {
                self.down[row * (self.columns + 1) + column]
            }
            _ => Slot::Empty,
        }
    }

    fn offer_across(&mut self, row: usize, columns: Range<usize>, source: Slot) {
        for column in columns {
            let slot = &mut self.across[row * self.columns + column];
            if slot.yields_to(source) {
                *slot = source;
            }
        }
    }

    fn offer_down(&mut self, rows: Range<usize>, column: usize, source: Slot) {
        for row in rows {
            let slot = &mut self.down[row * (self.columns + 1) + column];
            if slot.yields_to(source) {
                *slot = source;
            }
        }
    }

    fn merge(&mut self, rows: Range<usize>, columns: Range<usize>, sides: &Sides) {
        let rows = rows.start.min(self.rows)..rows.end.min(self.rows);
        let columns = columns.start.min(self.columns)..columns.end.min(self.columns);
        if rows.is_empty() || columns.is_empty() {
            return;
        }
        self.offer_across(rows.start, columns.clone(), sides.top);
        self.offer_across(rows.end, columns.clone(), sides.bottom);
        self.offer_down(rows.clone(), columns.start, sides.start);
        self.offer_down(rows, columns.end, sides.end);
    }

    fn block_inside(&mut self, rows: Range<usize>, columns: Range<usize>) {
        let rows = rows.start.min(self.rows)..rows.end.min(self.rows);
        let columns = columns.start.min(self.columns)..columns.end.min(self.columns);
        for row in rows.clone() {
            for column in columns.clone().skip(1) {
                let slot = &mut self.down[row * (self.columns + 1) + column];
                if matches!(slot, Slot::Empty) {
                    *slot = Slot::Blocked;
                }
            }
        }
        for row in rows.skip(1) {
            for column in columns.clone() {
                let slot = &mut self.across[row * self.columns + column];
                if matches!(slot, Slot::Empty) {
                    *slot = Slot::Blocked;
                }
            }
        }
    }
}

struct Joint {
    across: f64,
    down: f64,
    inline: Ordering,
    block: Ordering,
    versus: Ordering,
}

fn joint(before: Slot, after: Slot, over: Slot, under: Slot) -> Joint {
    let inline = contest(before, after);
    let block = contest(over, under);
    let inline_winner = if inline == Ordering::Greater {
        before
    } else {
        after
    };
    let block_winner = if block == Ordering::Greater {
        over
    } else {
        under
    };
    Joint {
        across: block_winner.width(),
        down: inline_winner.width(),
        inline,
        block,
        versus: contest(inline_winner, block_winner),
    }
}

fn stretch(start: &mut f64, size: &mut f64, by: f64, wins: bool, at_start: bool) {
    let by = if wins { by / 2.0 } else { -by / 2.0 };
    *size += by;
    if at_start {
        *start -= by;
    }
}

fn fill_lines(lines: &mut [Option<f64>]) {
    let first = lines.iter().flatten().next().copied().unwrap_or(0.0);
    let mut last = first;
    for line in lines {
        match line {
            Some(value) => last = *value,
            None => *line = Some(last),
        }
    }
}

fn best_gap(length: f64, dash: f64, gap: f64) -> f64 {
    let fewest = ((length + gap) / (dash + gap)).floor();
    let wide = (length - fewest * dash) / (fewest - 1.0);
    let narrow = (length - (fewest + 1.0) * dash) / fewest;
    if narrow <= 0.0 || (wide - gap).abs() < (narrow - gap).abs() {
        wide
    } else {
        narrow
    }
}

fn quarter(value: f64) -> f64 {
    (value * 4.0 + 0.5).floor() / 4.0
}

fn arc_pieces(across: f64, down: f64) -> u32 {
    let across = (across * 64.0).floor();
    let down = (down * 64.0).floor();
    let distance = ((across.max(down) + across.min(down) / 2.0 + 16.0) / 32.0).floor();
    let mut pieces = 2_u32;
    let mut bound = 8.0;
    while distance >= bound && pieces < MOST_PIECES {
        pieces *= 2;
        bound *= 4.0;
    }
    pieces
}

struct Span {
    along: Range<f64>,
    cross: Range<f64>,
    is_horizontal: bool,
}

impl Span {
    fn rect(&self, along: Range<f64>, cross: Range<f64>) -> Rect {
        if self.is_horizontal {
            Rect::new(along.start, cross.start, along.end, cross.end)
        } else {
            Rect::new(cross.start, along.start, cross.end, along.end)
        }
    }

    fn dot(&self, along: f64, radius: f64) -> BezPath {
        let cross = (self.cross.start + self.cross.end) / 2.0;
        let centre = if self.is_horizontal {
            Point::new(along, cross)
        } else {
            Point::new(cross, along)
        };
        let mut arcs = 4_u32;
        let mut error = radius * ARC_ERROR;
        while error > ARC_TOLERANCE && arcs < MOST_ARCS {
            arcs *= 2;
            error /= 4.0;
        }
        let turn = TAU / f64::from(arcs);
        let reach = radius / (turn / 2.0).cos();
        let bend = 2.0 * (reach - radius * (turn / 2.0).cos());
        let spoke = |angle: f64, length: f64| centre + Vec2::new(angle.cos(), angle.sin()) * length;
        let mut path = BezPath::new();
        path.move_to(spoke(0.0, radius));
        for arc in 0..arcs {
            let from = f64::from(arc) * turn;
            let middle = from + turn / 2.0;
            let pieces = arc_pieces(bend * middle.cos().abs(), bend * middle.sin().abs());
            let first = spoke(from, radius).to_vec2();
            let control = spoke(middle, reach).to_vec2();
            let last = spoke(from + turn, radius).to_vec2();
            for piece in 1..=pieces {
                let along = f64::from(piece) / f64::from(pieces);
                let back = 1.0 - along;
                let corner =
                    first * (back * back) + control * (2.0 * back * along) + last * (along * along);
                path.line_to(corner.to_point());
            }
        }
        path.close_path();
        path
    }
}

struct Brush<'scene, S: PaintScene> {
    scene: &'scene mut S,
    transform: Affine,
    origin: Vec2,
    run: BezPath,
    run_color: Option<Color>,
}

impl<S: PaintScene> Brush<'_, S> {
    fn flush(&mut self) {
        if let Some(color) = self.run_color.take() {
            self.scene
                .fill(Fill::NonZero, self.transform, color, None, &self.run);
            self.run = BezPath::new();
        }
    }

    fn shape(&mut self, shape: impl kurbo::Shape, color: Color) {
        let is_opaque = color.components[3] >= 1.0;
        if self.run_color != Some(color) || !is_opaque {
            self.flush();
        }
        self.run.extend(shape.path_elements(CURVE_TOLERANCE));
        self.run_color = Some(color);
    }

    fn block(&mut self, rect: Rect, color: Color) {
        if rect.width() > 0.0 && rect.height() > 0.0 {
            self.shape(rect, color);
        }
    }

    fn snap(&self, rect: Rect) -> Rect {
        let snap = |value: f64, origin: f64| (value + origin + 0.5).floor() - origin;
        Rect::new(
            snap(rect.x0, self.origin.x),
            snap(rect.y0, self.origin.y),
            snap(rect.x1, self.origin.x),
            snap(rect.y1, self.origin.y),
        )
    }

    fn edge(&mut self, rect: Rect, is_horizontal: bool, line: Line) {
        let rect = self.snap(rect);
        let span = if is_horizontal {
            Span {
                along: rect.x0..rect.x1,
                cross: rect.y0..rect.y1,
                is_horizontal,
            }
        } else {
            Span {
                along: rect.y0..rect.y1,
                cross: rect.x0..rect.x1,
                is_horizontal,
            }
        };
        let thickness = span.cross.end - span.cross.start;
        if span.along.end <= span.along.start || thickness <= 0.0 {
            return;
        }
        let side = if is_horizontal { Edge::Top } else { Edge::Left };
        match line.style {
            LineStyle::Double if thickness >= 3.0 => {
                let third = ((thickness + 1.0) / 3.0).floor();
                let near = span.cross.start..span.cross.start + third;
                let far = span.cross.end - third..span.cross.end;
                self.block(span.rect(span.along.clone(), near), line.color);
                self.block(span.rect(span.along.clone(), far), line.color);
            }
            LineStyle::Groove | LineStyle::Ridge if thickness > 1.0 => {
                let middle = span.cross.start + ((thickness + 1.0) / 2.0).floor();
                let is_groove = line.style == LineStyle::Groove;
                let near = beveled_edge_color(line.color, side, is_groove);
                let far = beveled_edge_color(line.color, side, !is_groove);
                let first = span.rect(span.along.clone(), span.cross.start..middle);
                self.block(first, near);
                self.block(span.rect(span.along.clone(), middle..span.cross.end), far);
            }
            LineStyle::Dashed => self.dashes(&span, thickness, line.color, false),
            LineStyle::Dotted if thickness <= 3.0 => {
                self.dashes(&span, thickness, line.color, true)
            }
            LineStyle::Dotted => self.dots(&span, thickness, line.color),
            LineStyle::Double | LineStyle::Groove | LineStyle::Ridge | LineStyle::Solid => {
                self.block(rect, line.color)
            }
        }
    }

    fn dots(&mut self, span: &Span, width: f64, color: Color) {
        let length = span.along.end - span.along.start;
        let step = if length < width * 2.0 {
            width * 2.0
        } else {
            best_gap(length, width, width) + width - DOT_SLACK
        };
        let first = span.along.start + width / 2.0;
        let last = span.along.end - width / 2.0;
        self.flush();
        let mut centre = first;
        loop {
            self.shape(span.dot(centre, width / 2.0), color);
            centre += step;
            if !step.is_finite() || step <= 0.0 || centre > last + 1.0e-6 {
                break;
            }
        }
    }

    fn dashes(&mut self, span: &Span, width: f64, color: Color, is_dotted: bool) {
        let length = span.along.end - span.along.start;
        let (dash, gap) = match (is_dotted, width >= 3.0) {
            (true, _) => (width, width),
            (false, true) => (width * 2.0, width),
            (false, false) => (width * 3.0, width * 2.0),
        };
        let pair = dash * 2.0 + gap;
        let pattern = if length <= dash * 2.0 {
            None
        } else if length <= pair {
            Some((dash * length / pair, gap * length / pair))
        } else if is_dotted {
            Some((dash, gap))
        } else {
            Some((dash, best_gap(length, dash, gap)))
        };
        let mut along = span.along.clone();
        if is_dotted {
            along = self.end_dots(span, width, color);
        }
        let Some((dash, gap)) = pattern else {
            if along.end > along.start {
                self.block(span.rect(along, span.cross.clone()), color);
            }
            return;
        };
        let mut start = along.start;
        while start < along.end {
            let end = (start + dash).min(along.end);
            let piece = if span.is_horizontal {
                start..end
            } else {
                quarter(start)..quarter(end)
            };
            self.block(span.rect(piece, span.cross.clone()), color);
            if !gap.is_finite() || dash + gap <= 0.0 {
                break;
            }
            start += dash + gap;
        }
    }

    fn end_dots(&mut self, span: &Span, width: f64, color: Color) -> Range<f64> {
        let length = span.along.end - span.along.start;
        let by_four = length % 4.0;
        let by_six = length % 6.0;
        let mut start_dot = None;
        let mut end_dot = None;
        if (width == 1.0 && length % 2.0 == 0.0) || (width == 3.0 && by_six == 0.0) {
            start_dot = Some((1.0, 1.0));
        }
        if (width == 2.0 && (by_four == 0.0 || by_four == 1.0))
            || (width == 3.0 && (by_six == 1.0 || by_six == 2.0))
        {
            start_dot = Some((start_dot.map_or(0.0, |(growth, _)| growth), -1.0));
        }
        if (width == 2.0 && by_four == 0.0) || (width == 3.0 && by_six == 1.0) {
            end_dot = Some(0.0);
        }
        if (width == 2.0 && by_four == 3.0) || (width == 3.0 && (by_six == 4.0 || by_six == 5.0)) {
            start_dot = Some((start_dot.map_or(0.0, |(growth, _)| growth), 1.0));
        }
        if width == 3.0 && by_six == 5.0 {
            end_dot = Some(0.0);
        } else if width == 3.0 && by_six == 0.0 {
            end_dot = Some(1.0);
        }
        let mut along = span.along.clone();
        if let Some((growth, offset)) = start_dot {
            let dot = along.start..along.start + width + growth;
            self.block(span.rect(dot, span.cross.clone()), color);
            along.start += width * 2.0 + offset;
        }
        if let Some(growth) = end_dot {
            let dot = along.end - width - growth..along.end;
            self.block(span.rect(dot, span.cross.clone()), color);
            along.end -= width + growth + 1.0;
        }
        along
    }
}

pub(crate) struct Edges {
    grid: Grid,
    column_lines: Vec<f64>,
    row_lines: Vec<f64>,
    sections: Vec<Range<usize>>,
    cells: HashMap<NodeId, (Range<usize>, Range<usize>)>,
    is_rtl: bool,
}

impl Edges {
    fn collect(dom: &BaseDocument, node: &Node, scale: f64) -> Option<Self> {
        let SpecialElementData::TableRoot(table) = &node.element_data()?.special_data else {
            return None;
        };
        if table.border_collapse != BorderCollapse::Collapse {
            return None;
        }
        let rows = table
            .cells
            .iter()
            .map(|cell| cell.rows().end)
            .fold(table.rows.len(), usize::max);
        let columns = table
            .cells
            .iter()
            .map(|cell| cell.columns().end)
            .max()
            .unwrap_or(0);
        if rows == 0 || columns == 0 {
            return None;
        }
        let table_style = node.primary_styles();
        let is_rtl = table_style
            .as_ref()
            .is_some_and(|style| style.clone_direction() == Direction::Rtl);
        let sides = |node_id: NodeId, order: usize| {
            let style = dom.get_node(node_id)?.primary_styles()?;
            Some(Sides::new(&style, is_rtl, scale, order))
        };

        let mut grid = Grid::new(columns, rows);
        let mut column_lines = vec![None; columns + 1];
        let mut row_lines = vec![None; rows + 1];
        let mut cells = HashMap::with_capacity(table.cells.len());
        let mut order = 0;
        for cell in &table.cells {
            order += 1;
            let cell_rows = cell.rows();
            let cell_columns = cell.columns();
            if cell_rows.len() > 1 || cell_columns.len() > 1 {
                grid.block_inside(cell_rows.clone(), cell_columns.clone());
            }
            if let Some(sides) = sides(cell.node_id, order) {
                grid.merge(cell_rows.clone(), cell_columns.clone(), &sides);
            }
            let Some(cell_node) = dom.get_node(cell.node_id) else {
                continue;
            };
            let layout = cell_node.unrounded_layout();
            let left = f64::from(layout.location.x);
            let right = left + f64::from(layout.size.width);
            let top = f64::from(layout.location.y);
            let height = table
                .collapsed_cell_span_height(cell.node_id)
                .unwrap_or(layout.size.height);
            let (start, end) = if is_rtl {
                (-right, -left)
            } else {
                (left, right)
            };
            if let Some(line) = column_lines.get_mut(cell_columns.start) {
                line.get_or_insert(start * scale);
            }
            if let Some(line) = column_lines.get_mut(cell_columns.end) {
                line.get_or_insert(end * scale);
            }
            if let Some(line) = row_lines.get_mut(cell_rows.start) {
                line.get_or_insert(top * scale);
            }
            if let Some(line) = row_lines.get_mut(cell_rows.end) {
                line.get_or_insert((top + f64::from(height)) * scale);
            }
            cells.insert(cell.node_id, (cell_rows, cell_columns));
        }
        fill_lines(&mut column_lines);
        fill_lines(&mut row_lines);

        let mut sections: Vec<Range<usize>> = Vec::new();
        for (index, row) in table.rows.iter().enumerate() {
            order += 1;
            if let Some(sides) = sides(row.node_id, order) {
                grid.merge(index..index + 1, 0..columns, &sides);
            }
            match sections.last_mut() {
                Some(last) if table.rows[last.start].group == row.group => last.end = index + 1,
                _ => sections.push(index..index + 1),
            }
        }
        for section in &sections {
            order += 1;
            let group = table.rows[section.start].group;
            if let Some(sides) = group.and_then(|group| sides(group, order)) {
                grid.merge(section.clone(), 0..columns, &sides);
            }
        }
        if table.rows.len() < rows {
            sections.push(table.rows.len()..rows);
        }
        let is_column = |node_id: NodeId| {
            dom.get_node(node_id)
                .and_then(|node| node.primary_styles())
                .is_some_and(|style| style.clone_display().inside() == DisplayInside::TableColumn)
        };
        order += 1;
        for (index, column) in table.columns.iter().enumerate().take(columns) {
            if !is_column(column.node_id) {
                continue;
            }
            if let Some(sides) = sides(column.node_id, order) {
                grid.merge(0..rows, index..index + 1, &sides);
            }
        }
        order += 1;
        let group_of = |index: usize| {
            let column = table.columns.get(index)?;
            match column.group {
                Some(group) => Some(group),
                None if is_column(column.node_id) => None,
                None => Some(column.node_id),
            }
        };
        let grouped = columns.min(table.columns.len());
        let mut start = 0;
        while start < grouped {
            let group = group_of(start);
            let mut end = start + 1;
            while end < grouped && group_of(end) == group {
                end += 1;
            }
            if let Some(sides) = group.and_then(|group| sides(group, order)) {
                grid.merge(0..rows, start..end, &sides);
            }
            start = end;
        }
        order += 1;
        if let Some(style) = &table_style {
            grid.merge(
                0..rows,
                0..columns,
                &Sides::new(style, is_rtl, scale, order),
            );
        }
        Some(Self {
            grid,
            column_lines: column_lines.into_iter().flatten().collect(),
            row_lines: row_lines.into_iter().flatten().collect(),
            sections,
            cells,
            is_rtl,
        })
    }

    fn physical(&self, start: f64, size: f64) -> (f64, f64) {
        if self.is_rtl {
            (-(start + size), -start)
        } else {
            (start, start + size)
        }
    }

    fn paint<S: PaintScene>(&self, brush: &mut Brush<'_, S>) {
        let grid = &self.grid;
        for section in &self.sections {
            for row in section.start..=section.end {
                for column in 0..=grid.columns {
                    let above = row.checked_sub(1);
                    let left = column.checked_sub(1);
                    if row < section.end
                        && let Some(line) = grid.down(Some(row), Some(column)).line()
                    {
                        let start = joint(
                            grid.across(Some(row), left),
                            grid.across(Some(row), Some(column)),
                            grid.down(above, Some(column)),
                            grid.down(Some(row), Some(column)),
                        );
                        let end = joint(
                            grid.across(Some(row + 1), left),
                            grid.across(Some(row + 1), Some(column)),
                            grid.down(Some(row), Some(column)),
                            grid.down(Some(row + 1), Some(column)),
                        );
                        let mut top = self.row_lines[row];
                        let mut height = self.row_lines[row + 1] - top;
                        let start_wins =
                            start.versus != Ordering::Greater && start.block != Ordering::Greater;
                        let end_wins =
                            end.versus != Ordering::Greater && end.block != Ordering::Less;
                        stretch(&mut top, &mut height, start.down, start_wins, true);
                        stretch(&mut top, &mut height, end.down, end_wins, false);
                        let (x0, x1) =
                            self.physical(self.column_lines[column] - line.width / 2.0, line.width);
                        brush.edge(Rect::new(x0, top, x1, top + height), false, line);
                    }
                    let is_painted_above = row == section.start && section.start > 0;
                    if column < grid.columns
                        && !is_painted_above
                        && let Some(line) = grid.across(Some(row), Some(column)).line()
                    {
                        let start = joint(
                            grid.across(Some(row), left),
                            grid.across(Some(row), Some(column)),
                            grid.down(above, Some(column)),
                            grid.down(Some(row), Some(column)),
                        );
                        let end = joint(
                            grid.across(Some(row), Some(column)),
                            grid.across(Some(row), Some(column + 1)),
                            grid.down(above, Some(column + 1)),
                            grid.down(Some(row), Some(column + 1)),
                        );
                        let mut inline = self.column_lines[column];
                        let mut width = self.column_lines[column + 1] - inline;
                        let start_wins =
                            start.versus != Ordering::Less && start.inline != Ordering::Greater;
                        let end_wins = end.versus != Ordering::Less && end.inline != Ordering::Less;
                        stretch(&mut inline, &mut width, start.across, start_wins, true);
                        stretch(&mut inline, &mut width, end.across, end_wins, false);
                        let (x0, x1) = self.physical(inline, width);
                        let top = self.row_lines[row] - line.width / 2.0;
                        brush.edge(Rect::new(x0, top, x1, top + line.width), true, line);
                    }
                }
            }
        }
        brush.flush();
    }
}

fn snap_origin(transform: Affine) -> Vec2 {
    let [a, b, c, d, x, y] = transform.as_coeffs();
    if [a, b, c, d] == [1.0, 0.0, 0.0, 1.0] {
        Vec2::new(x, y)
    } else {
        Vec2::ZERO
    }
}

impl ElementCx<'_, '_> {
    fn table_edges(&self, table_id: NodeId) -> Option<Rc<Edges>> {
        let mut cache = self.context.table_edges.borrow_mut();
        if let Some(edges) = cache.get(&table_id) {
            return edges.as_ref().map(Rc::clone);
        }
        let dom = self.context.dom;
        let edges = dom
            .get_node(table_id)
            .and_then(|node| Edges::collect(dom, node, self.scale))
            .map(Rc::new);
        cache.insert(table_id, edges.as_ref().map(Rc::clone));
        edges
    }

    pub(crate) fn cell_edges(&self) -> Option<Rc<Edges>> {
        let is_cell = self.style.clone_display().inside() == DisplayInside::TableCell;
        if !is_cell || !self.has_collapsed_borders() {
            return None;
        }
        self.table_edges(self.node.layout_parent.get()?)
            .filter(|edges| edges.cells.contains_key(&self.node.id))
    }

    pub(crate) fn paints_under_cell_borders(&self) -> bool {
        let has_layer = self.node.is_stacking_context_root(false)
            || self.style.clone_position() != Position::Static;
        if has_layer {
            return false;
        }
        let is_hidden = covers_background(&self.style) && self.has_covering_child();
        !is_hidden && self.cell_edges().is_some()
    }

    fn has_covering_child(&self) -> bool {
        let layout = self.node.final_layout();
        let inner = Rect::new(
            f64::from(layout.border.left),
            f64::from(layout.border.top),
            f64::from(layout.size.width - layout.border.right),
            f64::from(layout.size.height - layout.border.bottom),
        );
        let children = self.node.layout_children.borrow();
        children.iter().flatten().any(|child_id| {
            let Some(child) = self.context.dom.get_node(*child_id) else {
                return false;
            };
            let Some(style) = child.primary_styles() else {
                return false;
            };
            let color = style
                .get_background()
                .background_color
                .resolve_to_absolute(&style.clone_color())
                .as_srgb_color();
            let layout = child.final_layout();
            let left = f64::from(layout.location.x);
            let top = f64::from(layout.location.y);
            let outer = Rect::new(
                left - COVER_SLACK,
                top - COVER_SLACK,
                left + f64::from(layout.size.width) + COVER_SLACK,
                top + f64::from(layout.size.height) + COVER_SLACK,
            );
            color.components[3] >= 1.0
                && style.clone_visibility() == Visibility::Visible
                && style.get_effects().opacity >= 1.0
                && outer.union(inner) == outer
        })
    }

    pub(crate) fn is_table_root(&self) -> bool {
        matches!(self.element.special_data, SpecialElementData::TableRoot(_))
    }

    pub(crate) fn paint_table_borders(&self, scene: &mut impl PaintScene) {
        if !self.is_table_root() {
            return;
        }
        let Some(edges) = self.table_edges(self.node.id) else {
            return;
        };
        let mut brush = Brush {
            scene,
            transform: self.transform,
            origin: snap_origin(self.transform),
            run: BezPath::new(),
            run_color: None,
        };
        edges.paint(&mut brush);
    }
}
