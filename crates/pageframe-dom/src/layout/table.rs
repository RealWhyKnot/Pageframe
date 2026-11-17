use num_traits::ToPrimitive;
use pageframe_traits::node_id::NodeId;
use std::{collections::HashSet, ops::Range, sync::Arc};
use style::logical_geometry::WritingMode;
use style::properties::ComputedValues;
use stylo_taffy::CopyStyleGetters;
use stylo_taffy::WritingModeExt;

use atomic_refcell::AtomicRefCell;
use markup5ever::local_name;
use style::computed_values::direction::T as Direction;
use style::typed_om::NumericBaseType;
use style::values::computed::calc::CalcPercentageLeaf;
use style::values::computed::length_percentage::{
    CalcLengthPercentage, CalcNode, ComputedLeaf, Unpacked as UnpackedLengthPercentage,
};
use style::values::computed::{Length, LengthPercentage};
use style::values::generics::Optional;
use style::values::specified::border::BorderStyle;
use style::values::specified::box_::{DisplayInside, DisplayOutside};
use style::{
    Atom, computed_values::border_collapse::T as BorderCollapse,
    computed_values::caption_side::T as CaptionSide,
    computed_values::table_layout::T as TableLayout,
};
use style_traits::values::specified::AllowedNumericType;
use taffy::{
    AvailableSpace, DetailedGridInfo, LayoutPartialTree as _, MaybeResolve, ResolveOrZero,
    TrackSizingFunction, style_helpers,
};

use crate::BaseDocument;
use crate::layout::LayoutPassState;

use super::damage::{CONSTRUCT_BOX, CONSTRUCT_DESCENDENT, CONSTRUCT_FC};
use super::resolve_calc_value;

pub struct TableTreeWrapper<'a, 'doc> {
    pub(crate) doc: &'a mut LayoutPassState<'doc>,
    pub(crate) ctx: Arc<TableContext>,
    pub(crate) style: Option<taffy::Style<Atom>>,
    pub(crate) cell_baselines: Vec<Option<f32>>,
}

#[derive(Debug)]
struct AutoColumns {
    tracks: Vec<TrackSizingFunction>,
    spacing: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct TableCaption {
    pub node_id: NodeId,
    pub is_below: bool,
}

// Deliberately not `Clone`: `style` may hold raw pointers into `calc_values`.
#[derive(Debug)]
pub struct TableContext {
    pub style: taffy::Style<Atom>,
    pub cells: Vec<TableCell>,
    /// Absolutely positioned descendants of the table's row groups and rows (and of the
    /// table itself). They take no part in the table grid.
    pub oof_children: Vec<(NodeId, taffy::Position)>,
    pub rows: Vec<TableRow>,
    pub columns: Vec<TableColumn>,
    pub computed_grid_info: AtomicRefCell<Option<DetailedGridInfo<Atom>>>,
    pub border_collapse: BorderCollapse,
    auto_columns: Option<AutoColumns>,
    spreads_rows: bool,
    collapsed_cells: HashSet<NodeId>,
    collapsed_column_cells: HashSet<NodeId>,
    has_collapsed_columns: bool,
    pub captions: Vec<TableCaption>,
    /// Backing storage for `calc()` track sizes synthesised by the table layout code.
    /// Taffy stores calc values as raw pointers, so these must outlive the `style`.
    #[allow(dead_code)]
    calc_values: Vec<LengthPercentage>,
}

// #[derive(Debug, Clone, Eq, PartialEq)]
// pub enum TableItemKind {
//     Row,
//     Cell,
// }

#[derive(Debug, Clone)]
pub struct TableCell {
    // kind: TableItemKind,
    pub node_id: NodeId,
    /// Index (into `TableContext::rows`) of the row the cell originates in
    pub row: u16,
    /// Index of the column the cell originates in
    pub column: usize,
    column_span: usize,
    row_span: u16,
    column_width_border: Option<f32>,
    percent_width: Option<f32>,
    style: taffy::Style<Atom>,
}

impl TableCell {
    pub fn columns(&self) -> Range<usize> {
        self.column..self.column + self.column_span
    }

    pub fn rows(&self) -> Range<usize> {
        let start = usize::from(self.row);
        start..start + usize::from(self.row_span)
    }
}

/// Tracks the current column position while walking the table's cells, so that
/// cells are assigned the same columns Taffy's dense auto-placement will give them
/// (skipping columns still occupied by rowspan cells from earlier rows).
#[derive(Debug, Default)]
struct ColumnCursor {
    /// The column the next cell in the current row will be placed in
    col: usize,
    /// The total number of columns seen so far
    num_columns: usize,
    /// For each column, how many rows (counting the current one) a rowspan cell still occupies
    rowspans: Vec<u16>,
}

impl ColumnCursor {
    fn start_row(&mut self) {
        self.col = 0;
        for remaining in self.rowspans.iter_mut() {
            *remaining = remaining.saturating_sub(1);
        }
    }

    /// Advance past occupied columns, returning the column of the next cell
    fn next_free(&mut self) -> usize {
        while self.rowspans.get(self.col).is_some_and(|r| *r > 0) {
            self.col += 1;
        }
        self.col
    }

    fn place(&mut self, colspan: usize, rowspan: u16) {
        let end = self.col + colspan;
        if self.rowspans.len() < end {
            self.rowspans.resize(end, 0);
        }
        for remaining in &mut self.rowspans[self.col..end] {
            *remaining = rowspan;
        }
        self.col = end;
        self.num_columns = self.num_columns.max(end);
    }
}

#[derive(Debug, Clone)]
pub struct TableColumn {
    pub node_id: NodeId,
    pub is_collapsed: bool,
    /// The column group (if any) that contains the column
    pub group: Option<NodeId>,
}

#[derive(Debug, Clone)]
pub struct TableRow {
    // kind: TableItemKind,
    pub node_id: NodeId,
    /// The row group (if any) that contains the row
    pub group: Option<NodeId>,
    /// The row's specified `height`, if it is a length
    pub height: Option<f32>,
    /// Whether the row or any cell originating in it (and spanning only this row) has a
    /// specified length `height`. Such rows do not receive excess table height when there
    /// are unconstrained rows to receive it instead.
    pub is_constrained: bool,
    pub percent: Option<f32>,
    pub is_body: bool,
    pub is_collapsed: bool,
}

/// The used width of one border side: border widths are not adjusted for
/// border-style in computed styles, so a border with `none`/`hidden` style
/// must be treated as zero-width.
/// The writing mode the table's grid runs in (the table's own)
#[cfg_attr(not(feature = "writing-mode"), allow(unused_variables))]
fn table_wm(doc: &BaseDocument, table_root_node_id: NodeId) -> WritingMode {
    #[cfg(feature = "writing-mode")]
    {
        if doc.try_root_element().map(|root| root.id) == Some(table_root_node_id) {
            doc.root_layout_wm(table_root_node_id)
        } else {
            doc.nodes[table_root_node_id].writing_mode()
        }
    }
    #[cfg(not(feature = "writing-mode"))]
    {
        WritingMode::empty()
    }
}

/// Convert a table part's style into the table's writing mode
fn table_taffy_style(
    style: &ComputedValues,
    table_wm: WritingMode,
    scale: f32,
) -> taffy::Style<Atom> {
    stylo_taffy::to_taffy_style_in(style, table_wm, scale)
}

fn flip_inline(rect: taffy::Rect<f32>, is_ltr: bool) -> taffy::Rect<f32> {
    if is_ltr {
        return rect;
    }
    taffy::Rect {
        left: rect.right,
        right: rect.left,
        ..rect
    }
}

fn widest<I: Iterator<Item = usize>>(
    slots: I,
    grid: &[Option<usize>],
    side: impl Fn(usize) -> f32,
) -> f32 {
    slots
        .filter_map(|slot| grid.get(slot).copied().flatten())
        .map(side)
        .fold(0.0, f32::max)
}

fn refresh_candidates(
    output: &mut taffy::tree::LayoutOutput,
    mut moved: taffy::tree::OofCandidates,
    location: taffy::Point<f32>,
) {
    moved.translate(location);
    for candidate in output.oof_candidates.as_mut_slice() {
        if let Some(fresh) = moved.iter().find(|fresh| fresh.node == candidate.node) {
            candidate.static_position = fresh.static_position;
        }
    }
}

const NO_BORDER: taffy::Rect<f32> = taffy::Rect {
    left: 0.0,
    right: 0.0,
    top: 0.0,
    bottom: 0.0,
};

fn widen(rect: taffy::Rect<f32>, other: taffy::Rect<f32>) -> taffy::Rect<f32> {
    taffy::Rect {
        left: rect.left.max(other.left),
        right: rect.right.max(other.right),
        top: rect.top.max(other.top),
        bottom: rect.bottom.max(other.bottom),
    }
}

#[derive(Clone, Copy)]
struct Frame {
    wm: WritingMode,
    is_ltr: bool,
}

fn visible(width: f32) -> f32 {
    if width.is_finite() { width } else { 0.0 }
}

fn mark_hidden(
    border: taffy::Rect<f32>,
    style: &ComputedValues,
    wm: WritingMode,
) -> taffy::Rect<f32> {
    if wm.is_vertical() {
        return border;
    }
    let styles = style.get_border();
    let side = |width: f32, style: BorderStyle| {
        if style == BorderStyle::Hidden {
            f32::INFINITY
        } else {
            width
        }
    };
    taffy::Rect {
        left: side(border.left, styles.border_left_style),
        right: side(border.right, styles.border_right_style),
        top: side(border.top, styles.border_top_style),
        bottom: side(border.bottom, styles.border_bottom_style),
    }
}

fn part_half_border(doc: &BaseDocument, node_id: NodeId, frame: Frame) -> taffy::Rect<f32> {
    doc.nodes[node_id]
        .primary_styles()
        .map_or(NO_BORDER, |style| {
            let border = table_taffy_style(&style, frame.wm, doc.nodes.device_scale())
                .border
                .resolve_or_zero(None, resolve_calc_value);
            let border = mark_hidden(border, &style, frame.wm);
            flip_inline(border.map(|width| width / 2.0), frame.is_ltr)
        })
}

fn track_half_borders(
    doc: &BaseDocument,
    tracks: &[(Option<NodeId>, Option<NodeId>)],
    frame: Frame,
    is_row: bool,
) -> Vec<taffy::Rect<f32>> {
    tracks
        .iter()
        .enumerate()
        .map(|(index, (node_id, group))| {
            let own = node_id.map_or(NO_BORDER, |node_id| part_half_border(doc, node_id, frame));
            let Some(group_id) = group else {
                return own;
            };
            let is_first = index == 0 || tracks[index - 1].1 != *group;
            let is_last = tracks.get(index + 1).is_none_or(|next| next.1 != *group);
            let mut outer = part_half_border(doc, *group_id, frame);
            if is_row {
                if !is_first {
                    outer.top = 0.0;
                }
                if !is_last {
                    outer.bottom = 0.0;
                }
            } else {
                if !is_first {
                    outer.left = 0.0;
                }
                if !is_last {
                    outer.right = 0.0;
                }
            }
            widen(own, outer)
        })
        .collect()
}

struct TrackBorders {
    frame: Frame,
    rows: Vec<taffy::Rect<f32>>,
    columns: Vec<taffy::Rect<f32>>,
}

impl TrackBorders {
    fn new(doc: &BaseDocument, rows: &[TableRow], columns: &[TableColumn], frame: Frame) -> Self {
        let rows: Vec<_> = rows
            .iter()
            .map(|row| (Some(row.node_id), row.group))
            .collect();
        let is_column = |node_id: NodeId| {
            doc.nodes[node_id]
                .display_style()
                .is_some_and(|display| display.inside() == DisplayInside::TableColumn)
        };
        let columns: Vec<_> = columns
            .iter()
            .map(|column| match column.group {
                None if !is_column(column.node_id) => (None, Some(column.node_id)),
                group => (Some(column.node_id), group),
            })
            .collect();
        Self {
            frame,
            rows: track_half_borders(doc, &rows, frame, true),
            columns: track_half_borders(doc, &columns, frame, false),
        }
    }

    fn row(&self, index: usize) -> taffy::Rect<f32> {
        self.rows.get(index).copied().unwrap_or(NO_BORDER)
    }

    fn column(&self, index: usize) -> taffy::Rect<f32> {
        self.columns.get(index).copied().unwrap_or(NO_BORDER)
    }

    fn around(
        &self,
        columns: &Range<usize>,
        rows: &Range<usize>,
        num_columns: usize,
        num_rows: usize,
    ) -> taffy::Rect<f32> {
        let widest = |values: &mut dyn Iterator<Item = f32>| values.fold(0.0, f32::max);
        let mut half = NO_BORDER;
        if columns.is_empty() || rows.is_empty() {
            return half;
        }
        half.left = self.column(columns.start).left;
        if columns.start == 0 {
            let edge = widest(&mut rows.clone().map(|row| self.row(row).left));
            half.left = half.left.max(edge);
        } else {
            half.left = half.left.max(self.column(columns.start - 1).right);
        }
        half.right = self.column(columns.end - 1).right;
        if columns.end >= num_columns {
            let edge = widest(&mut rows.clone().map(|row| self.row(row).right));
            half.right = half.right.max(edge);
        } else {
            half.right = half.right.max(self.column(columns.end).left);
        }
        half.top = self.row(rows.start).top;
        if rows.start == 0 {
            let edge = widest(&mut columns.clone().map(|column| self.column(column).top));
            half.top = half.top.max(edge);
        } else {
            half.top = half.top.max(self.row(rows.start - 1).bottom);
        }
        half.bottom = self.row(rows.end - 1).bottom;
        if rows.end >= num_rows {
            let edge = widest(&mut columns.clone().map(|column| self.column(column).bottom));
            half.bottom = half.bottom.max(edge);
        } else {
            half.bottom = half.bottom.max(self.row(rows.end).top);
        }
        half
    }
}

fn resolve_collapsed_borders(
    doc: &mut BaseDocument,
    cells: &mut [TableCell],
    num_columns: usize,
    tracks: &TrackBorders,
    own: taffy::Rect<f32>,
) -> taffy::Rect<taffy::LengthPercentage> {
    let Frame { wm, is_ltr } = tracks.frame;
    let columns = num_columns;
    let num_rows = tracks.rows.len();
    let table = flip_inline(own.map(|width| width / 2.0), is_ltr);
    let halves: Vec<taffy::Rect<f32>> = cells
        .iter()
        .map(|cell| {
            let border = cell.style.border.resolve_or_zero(None, resolve_calc_value);
            let border = match doc.nodes[cell.node_id].primary_styles() {
                Some(style) => mark_hidden(border, &style, wm),
                None => border,
            };
            flip_inline(border, is_ltr)
        })
        .collect();
    let span = |cell: &TableCell| {
        let first_column = cell.column.min(columns);
        let first_row = usize::from(cell.row).min(num_rows);
        let end_column = (first_column + cell.column_span).min(columns);
        let end_row = (first_row + usize::from(cell.row_span)).min(num_rows);
        (first_column..end_column, first_row..end_row)
    };

    let mut grid: Vec<Option<usize>> = vec![None; columns * num_rows];
    for (index, cell) in cells.iter().enumerate() {
        let (cell_columns, cell_rows) = span(cell);
        for row in cell_rows {
            for column in cell_columns.clone() {
                grid[row * columns + column].get_or_insert(index);
            }
        }
    }

    let mut outer = table.map(visible);
    for (index, cell) in cells.iter_mut().enumerate() {
        let (cell_columns, cell_rows) = span(cell);
        let around = tracks.around(&cell_columns, &cell_rows, columns, num_rows);
        let mut half = widen(halves[index], around);
        if cell_columns.start == 0 {
            half.left = half.left.max(table.left);
        } else {
            let slots = cell_rows
                .clone()
                .map(|row| row * columns + cell_columns.start - 1);
            half.left = half
                .left
                .max(widest(slots, &grid, |cell| halves[cell].right));
        }
        if cell_columns.end >= columns {
            half.right = half.right.max(table.right);
        } else {
            let slots = cell_rows
                .clone()
                .map(|row| row * columns + cell_columns.end);
            half.right = half
                .right
                .max(widest(slots, &grid, |cell| halves[cell].left));
        }
        if cell_rows.start == 0 {
            half.top = half.top.max(table.top);
        } else {
            let slots = cell_columns
                .clone()
                .map(|column| (cell_rows.start - 1) * columns + column);
            half.top = half
                .top
                .max(widest(slots, &grid, |cell| halves[cell].bottom));
        }
        if cell_rows.end >= num_rows {
            half.bottom = half.bottom.max(table.bottom);
        } else {
            let slots = cell_columns
                .clone()
                .map(|column| cell_rows.end * columns + column);
            half.bottom = half
                .bottom
                .max(widest(slots, &grid, |cell| halves[cell].top));
        }

        let half = half.map(visible);
        if cell_columns.start == 0 {
            outer.left = outer.left.max(half.left);
        }
        if cell_columns.end >= columns {
            outer.right = outer.right.max(half.right);
        }
        if cell_rows.start == 0 {
            outer.top = outer.top.max(half.top);
        }
        if cell_rows.end >= num_rows {
            outer.bottom = outer.bottom.max(half.bottom);
        }
        let half = flip_inline(half, is_ltr);
        cell.style.border = half.map(style_helpers::length);
        if let Some(element) = doc.nodes[cell.node_id].element_data_mut() {
            element.collapsed_border = Some(wm.physical_rect(half));
        }
    }
    flip_inline(outer, is_ltr).map(style_helpers::length)
}

/// Build a `calc(<percent> + <length>)` track sizing function. The calc value is
/// boxed and stored in `calc_values` so that the raw pointer Taffy holds stays valid.
fn percent_plus_length(
    calc_values: &mut Vec<LengthPercentage>,
    percent: f32,
    length: f32,
) -> TrackSizingFunction {
    let node = CalcNode::Sum(
        vec![
            CalcNode::Leaf(ComputedLeaf::Percentage(CalcPercentageLeaf::new(
                percent,
                Optional::Some(NumericBaseType::Length),
            ))),
            CalcNode::Leaf(ComputedLeaf::Length(Length::new(length))),
        ]
        .into(),
    );
    let value = LengthPercentage::new_calc(node, AllowedNumericType::NonNegative);
    let dim = match value.unpack() {
        UnpackedLengthPercentage::Calc(calc) => {
            let ptr = calc as *const CalcLengthPercentage as *const ();
            // SAFETY: the pointer targets the heap allocation owned by `value`, which
            // is kept alive by `calc_values` for as long as the TableContext exists.
            unsafe { taffy::Dimension::from_raw(taffy::CompactLength::calc(ptr)) }
        }
        UnpackedLengthPercentage::Length(len) => style_helpers::length(len.px()),
        UnpackedLengthPercentage::Percentage(p) => style_helpers::percent(p.0),
    };
    calc_values.push(value);
    dim.into()
}

pub(crate) fn build_table_context(
    doc: &mut BaseDocument,
    table_root_node_id: NodeId,
) -> (TableContext, Vec<NodeId>) {
    let mut cells: Vec<TableCell> = Vec::new();
    let mut oof_children: Vec<(NodeId, taffy::Position)> = Vec::new();
    let mut rows: Vec<TableRow> = Vec::new();
    let mut row = 0u16;
    let mut cursor = ColumnCursor::default();

    let wm = table_wm(doc, table_root_node_id);
    let scale = doc.nodes.device_scale();
    let root_node = &mut doc.nodes[table_root_node_id];

    let children = std::mem::take(&mut root_node.children);

    let Some(stylo_styles) = root_node.primary_styles() else {
        panic!("Ignoring table because it has no styles");
    };

    let mut style = table_taffy_style(&stylo_styles, wm, scale);
    style.item_is_table = true;
    // Use `dense` row-flow so that each cell scans the row from its
    // leftmost column for the first free track. Without `dense`,
    // `place_definite_secondary_axis_item` keeps a per-item secondary
    // cursor across rows, which means cells in later rows do not
    // backfill columns freed up by rowspan cells from earlier rows.
    style.grid_auto_flow = taffy::GridAutoFlow::RowDense;
    style.grid_auto_columns = Vec::new();
    style.grid_auto_rows = Vec::new();

    // The fixed table layout algorithm only applies when the table has a non-auto width
    let is_fixed = match stylo_styles.clone_table_layout() {
        TableLayout::Fixed => !style.size.width.is_auto(),
        TableLayout::Auto => false,
    };

    let border_collapse = stylo_styles.clone_border_collapse();
    let border_spacing = stylo_styles.clone_border_spacing().0;

    drop(stylo_styles);

    let mut columns: Vec<TableColumn> = Vec::new();
    let mut column_sizes: Vec<taffy::TrackSizingFunction> = Vec::new();
    for child_id in children.iter().copied() {
        collect_columns(doc, child_id, wm, &mut columns, &mut column_sizes);
    }
    // Percentage column widths only take effect in the fixed table layout algorithm
    if !is_fixed {
        for column in column_sizes.iter_mut() {
            if column.max.into_raw().tag() == taffy::CompactLength::PERCENT_TAG {
                *column = style_helpers::auto();
            }
        }
    }
    // Percentage widths set on first-row cells: (column index, percentage, padding + border)
    let mut percent_columns: Vec<(usize, f32, f32)> = Vec::new();
    let mut calc_values: Vec<LengthPercentage> = Vec::new();
    // Header row groups are laid out before other rows, and footer row groups after
    let row_group_order = |doc: &BaseDocument, child_id: NodeId| -> u8 {
        let display = doc.nodes[child_id]
            .primary_styles()
            .map(|s| s.clone_display());
        match display.map(|d| d.inside()) {
            Some(DisplayInside::TableHeaderGroup) => 0,
            Some(DisplayInside::TableFooterGroup) => 2,
            _ => 1,
        }
    };
    let captions: Vec<TableCaption> = children
        .iter()
        .copied()
        .filter_map(|child_id| {
            let styles = doc.nodes[child_id].primary_styles()?;
            let is_caption = styles.clone_display().outside() == DisplayOutside::TableCaption
                && !styles.clone_position().is_absolutely_positioned()
                && !wm.is_vertical();
            is_caption.then(|| TableCaption {
                node_id: child_id,
                is_below: styles.get_inherited_table().caption_side == CaptionSide::Bottom,
            })
        })
        .collect();
    let is_caption = |child_id: NodeId| captions.iter().any(|caption| caption.node_id == child_id);
    for order in 0..3 {
        for child_id in children.iter().copied() {
            if row_group_order(doc, child_id) != order || is_caption(child_id) {
                continue;
            }
            collect_table_cells(
                doc,
                child_id,
                wm,
                is_fixed,
                border_collapse,
                &mut row,
                &mut cursor,
                &mut cells,
                &mut oof_children,
                &mut rows,
                &mut column_sizes,
                &mut percent_columns,
                u16::MAX,
            );
        }
    }
    for cell in &mut cells {
        let start = usize::from(cell.row);
        let Some(first_row) = rows.get(start) else {
            continue;
        };
        let section_rows = rows[start..]
            .iter()
            .take_while(|row| row.group == first_row.group)
            .count();
        cell.row_span = cell
            .row_span
            .min(u16::try_from(section_rows).unwrap_or(u16::MAX));
        cell.style.grid_row.end = style_helpers::span(cell.row_span);
    }
    if cells.iter().any(|cell| cell.column_span > 1) {
        let length = cursor.num_columns.max(column_sizes.len());
        let mut occupied = vec![false; length];
        for cell in &cells {
            occupied[cell.column] = true;
        }
        for (index, column) in columns.iter().enumerate() {
            let sized = column_sizes.get(index).is_some_and(|track| {
                track_length(track).is_some_and(|width| width > 0.0)
                    || (track.min.into_raw().tag() == taffy::CompactLength::LENGTH_TAG
                        && track.min.into_raw().value() > 0.0)
            });
            let percent = doc.nodes[column.node_id]
                .primary_styles()
                .is_some_and(|style| {
                    let width = stylo_taffy::convert::dimension(style.get_width());
                    width.tag() == taffy::CompactLength::PERCENT_TAG && width.value() > 0.0
                });
            occupied[index] |= sized || percent;
        }
        if occupied.iter().any(|&occupied| !occupied) {
            let mut offsets = Vec::with_capacity(length + 1);
            offsets.push(0_usize);
            for &occupied in &occupied {
                offsets.push(offsets.last().copied().unwrap_or(0) + usize::from(occupied));
            }
            for cell in &mut cells {
                let start = cell.column;
                let end = start + cell.column_span;
                cell.column = offsets[start];
                cell.column_span = offsets[end] - offsets[start];
                cell.style.grid_column.end =
                    style_helpers::span(u16::try_from(cell.column_span).unwrap_or(u16::MAX));
            }
            cursor.num_columns = offsets[cursor.num_columns];
            columns = columns
                .into_iter()
                .enumerate()
                .filter_map(|(index, column)| occupied[index].then_some(column))
                .collect();
            column_sizes = column_sizes
                .into_iter()
                .enumerate()
                .filter_map(|(index, column)| occupied[index].then_some(column))
                .collect();
            for (column, _, _) in &mut percent_columns {
                *column = offsets[*column];
            }
        }
    }

    let remaining_column = if is_fixed {
        style_helpers::minmax(style_helpers::length(0.0), style_helpers::fr(1.0))
    } else {
        style_helpers::auto()
    };
    // Trailing `<col>` columns which contain no cells and have no definite width
    // (i.e. would be zero-width) are dropped, so they don't add border-spacing.
    while column_sizes.len() > cursor.num_columns
        && column_sizes
            .last()
            .is_some_and(|c| c.min.into_raw().tag() == taffy::CompactLength::AUTO_TAG)
    {
        column_sizes.pop();
        columns.pop();
    }
    let num_columns = cursor.num_columns.max(column_sizes.len());
    column_sizes.resize(num_columns, remaining_column);
    if border_collapse == BorderCollapse::Collapse {
        let own = style.border.resolve_or_zero(None, resolve_calc_value);
        let table_style = doc.nodes[table_root_node_id].primary_styles();
        let is_rtl = table_style
            .as_ref()
            .is_some_and(|style| style.clone_direction() == Direction::Rtl);
        let own = match &table_style {
            Some(style) => mark_hidden(own, style, wm),
            None => own,
        };
        drop(table_style);
        let frame = Frame {
            wm,
            is_ltr: wm.direction() == taffy::Direction::Ltr && !is_rtl,
        };
        let tracks = TrackBorders::new(doc, &rows, &columns, frame);
        style.border = resolve_collapsed_borders(doc, &mut cells, num_columns, &tracks, own);
        for cell in &cells {
            let Some(counted) = cell.column_width_border else {
                continue;
            };
            let border = cell.style.border.resolve_or_zero(None, resolve_calc_value);
            let growth = border.left + border.right - counted;
            if let Some(column) = column_sizes.get_mut(cell.column) {
                if column.max.into_raw().tag() == taffy::CompactLength::LENGTH_TAG {
                    *column = style_helpers::length(column.max.into_raw().value() + growth);
                }
            }
            for (column, _, extra) in percent_columns.iter_mut() {
                if *column == cell.column {
                    *extra += growth;
                }
            }
        }
    } else {
        for cell in &cells {
            if let Some(element) = doc.nodes[cell.node_id].element_data_mut() {
                element.collapsed_border = None;
            }
        }
    }
    if is_fixed {
        // In the fixed table layout algorithm, percentage column widths resolve against
        // the table's content width minus the horizontal border-spacing between columns,
        // whereas Taffy resolves percentage tracks against the container's inner width
        // (from which only the outer border-spacing has been removed via padding). Fold
        // the inner border-spacing into the percentage using `calc(N% - N * spacing)`.
        let spacing_x = match border_collapse {
            BorderCollapse::Separate => border_spacing.width.px(),
            BorderCollapse::Collapse => 0.0,
        };
        let inner_spacing = spacing_x * num_columns.saturating_sub(1) as f32;
        let mut percent_track = |percent: f32, extra: f32| -> TrackSizingFunction {
            let extra = extra - percent * inner_spacing;
            if extra == 0.0 {
                style_helpers::percent(percent)
            } else {
                percent_plus_length(&mut calc_values, percent, extra)
            }
        };

        for column in column_sizes.iter_mut() {
            if column.max.is_auto() {
                *column = remaining_column;
            } else if column.max.into_raw().tag() == taffy::CompactLength::PERCENT_TAG {
                *column = percent_track(column.max.into_raw().value(), 0.0);
            }
        }

        // Percentage widths on cells apply to the cell's content box, so the
        // cell's horizontal padding and border must be added to the column width.
        for (col_idx, percent, extra) in percent_columns {
            if let Some(column) = column_sizes.get_mut(col_idx) {
                *column = percent_track(percent, extra);
            }
        }
    }

    if is_fixed
        && columns.iter().any(|column| column.is_collapsed)
        && column_sizes
            .iter()
            .all(|column| track_length(column).is_some())
    {
        for column in &mut column_sizes {
            let width = track_length(column).unwrap_or(0.0);
            *column = style_helpers::minmax(
                style_helpers::length(width),
                style_helpers::fr(width.max(1.0)),
            );
        }
    }

    let auto_columns = (!is_fixed && !wm.is_vertical()).then(|| AutoColumns {
        tracks: column_sizes.clone(),
        spacing: match border_collapse {
            BorderCollapse::Separate => border_spacing.width.px(),
            BorderCollapse::Collapse => 0.0,
        },
    });
    style.grid_template_columns = column_sizes.into_iter().map(|dim| dim.into()).collect();
    // A row's `height` is a minimum: the row grows to fit the content of its cells.
    // Excess table height is distributed to unconstrained rows (via the stretching of
    // `auto` tracks) if there are any, and to all rows otherwise. So constrained rows
    // use a `max-content` (rather than `auto`) maximum when there are unconstrained rows.
    let is_sized = !(style.size.height.is_auto() && style.min_size.height.is_auto());
    if !is_sized {
        for row in &mut rows {
            row.percent = None;
        }
    }
    let spreads_rows = rows.len() > 1 && !wm.is_vertical();
    let mut unclaimed = 1.0_f32;
    let shares: Vec<Option<f32>> = rows
        .iter()
        .map(|row| {
            let share = row.percent?.clamp(0.0, unclaimed);
            unclaimed -= share;
            Some(share)
        })
        .collect();
    let claimed = 1.0 - unclaimed;
    let has_plain_rows = shares.iter().any(Option::is_none);
    let has_unconstrained_rows = rows
        .iter()
        .zip(&shares)
        .any(|(row, share)| !row.is_constrained && share.is_none());
    style.grid_template_rows = rows
        .iter()
        .zip(shares)
        .map(|(row, share)| {
            let min = match row.height {
                Some(height) => style_helpers::length(height),
                None if share.is_some() => style_helpers::min_content(),
                None => style_helpers::auto(),
            };
            let max = match share {
                _ if spreads_rows => style_helpers::max_content(),
                Some(share) if has_plain_rows => style_helpers::percent(share),
                Some(share) if claimed > 0.0 => style_helpers::percent(share / claimed),
                _ if row.is_constrained && has_unconstrained_rows => style_helpers::max_content(),
                _ => style_helpers::auto(),
            };
            style_helpers::minmax(min, max)
        })
        .collect();

    style.gap = match border_collapse {
        BorderCollapse::Separate => {
            // In the separated borders model, `border-spacing` also applies between
            // the table border and the outermost cells, in addition to between cells.
            let spacing_x = border_spacing.width.px();
            let spacing_y = border_spacing.height.px();
            let padding = style.padding.resolve_or_zero(None, resolve_calc_value);
            style.padding = taffy::Rect {
                left: style_helpers::length(padding.left + spacing_x),
                right: style_helpers::length(padding.right + spacing_x),
                top: style_helpers::length(padding.top + spacing_y),
                bottom: style_helpers::length(padding.bottom + spacing_y),
            };
            taffy::Size {
                width: style_helpers::length(spacing_x),
                height: style_helpers::length(spacing_y),
            }
        }
        BorderCollapse::Collapse => taffy::Size::ZERO.map(style_helpers::length),
    };

    let has_collapsed_columns = columns.iter().any(|column| column.is_collapsed);
    let collapsed_column_cells = cells
        .iter()
        .filter_map(|cell| {
            (cell.column..(cell.column + cell.column_span))
                .all(|index| columns.get(index).is_some_and(|column| column.is_collapsed))
                .then_some(cell.node_id)
        })
        .collect();
    let collapsed_cells = cells
        .iter()
        .filter_map(|cell| {
            (rows
                .get(usize::from(cell.row))
                .is_some_and(|row| row.is_collapsed)
                || (cell.column..(cell.column + cell.column_span))
                    .all(|index| columns.get(index).is_some_and(|column| column.is_collapsed)))
            .then_some(cell.node_id)
        })
        .collect();
    let layout_children = cells
        .iter()
        .map(|cell| cell.node_id)
        .chain(oof_children.iter().map(|(node_id, _)| *node_id))
        .chain(captions.iter().map(|caption| caption.node_id))
        .collect();
    let root_node = &mut doc.nodes[table_root_node_id];
    root_node.children = children;

    (
        TableContext {
            style,
            cells,
            oof_children,
            rows,
            columns,
            computed_grid_info: AtomicRefCell::new(None),
            border_collapse,
            auto_columns,
            spreads_rows,
            collapsed_cells,
            collapsed_column_cells,
            has_collapsed_columns,
            captions,
            calc_values,
        },
        layout_children,
    )
}

/// Collect `<col>` elements (and `display: table-column` elements) along with their
/// widths, which take precedence over cell widths in the fixed table layout algorithm.
/// Columns without a specified width are recorded as `auto`.
fn collect_columns(
    doc: &mut BaseDocument,
    node_id: NodeId,
    wm: WritingMode,
    columns: &mut Vec<TableColumn>,
    column_sizes: &mut Vec<TrackSizingFunction>,
) {
    let scale = doc.nodes.device_scale();
    let node = &doc.nodes[node_id];
    if !node.is_element() {
        return;
    }
    let Some(display) = node.display_style() else {
        return;
    };

    match display.inside() {
        DisplayInside::TableColumnGroup
            if node.children.iter().any(|&child| {
                doc.nodes[child]
                    .display_style()
                    .is_some_and(|display| display.inside() == DisplayInside::TableColumn)
            }) =>
        {
            let first_column = columns.len();
            let children = std::mem::take(&mut doc.nodes[node_id].children);
            for child_id in children.iter().copied() {
                collect_columns(doc, child_id, wm, columns, column_sizes);
            }
            doc.nodes[node_id].children = children;
            for column in &mut columns[first_column..] {
                column.group.get_or_insert(node_id);
            }
        }
        DisplayInside::TableColumn | DisplayInside::TableColumnGroup => {
            let is_collapsed = node
                .primary_styles()
                .unwrap()
                .get_inherited_box()
                .visibility
                == style::computed_values::visibility::T::Collapse;
            let style = table_taffy_style(&node.primary_styles().unwrap(), wm, scale);
            let span: u16 = node
                .attr(local_name!("span"))
                .and_then(|val| val.parse::<u16>().ok())
                .map(|v| v.max(1))
                .unwrap_or(1);
            let column: TrackSizingFunction = match style.size.width.tag() {
                taffy::CompactLength::LENGTH_TAG => {
                    // A definite `max-width` clamps the column width; a zero width is treated as auto
                    let mut width = style.size.width.value();
                    if style.max_size.width.into_raw().tag() == taffy::CompactLength::LENGTH_TAG {
                        width = width.min(style.max_size.width.into_raw().value());
                    }
                    if width > 0.0 {
                        style_helpers::length(width)
                    } else {
                        style_helpers::auto()
                    }
                }
                taffy::CompactLength::PERCENT_TAG => {
                    style_helpers::percent(style.size.width.value())
                }
                // Browsers treat calc() widths on columns as auto
                _ => style_helpers::auto(),
            };
            // A definite `min-width` on a column acts as a floor on its width
            let column = match style.min_size.width.into_raw().tag() {
                taffy::CompactLength::LENGTH_TAG if column.max.is_auto() => style_helpers::minmax(
                    style_helpers::length(style.min_size.width.into_raw().value()),
                    style_helpers::auto(),
                ),
                _ => column,
            };
            for _ in 0..span {
                columns.push(TableColumn {
                    node_id,
                    is_collapsed,
                    group: None,
                });
                column_sizes.push(column);
            }
        }
        _ => {}
    }
}

fn table_row_count(doc: &BaseDocument, node_id: NodeId) -> u16 {
    let node = &doc.nodes[node_id];
    let Some(style) = node.primary_styles() else {
        return 0;
    };
    if style.clone_position().is_absolutely_positioned() {
        return 0;
    }
    match style.clone_display().inside() {
        DisplayInside::TableRow => 1,
        DisplayInside::TableRowGroup
        | DisplayInside::TableHeaderGroup
        | DisplayInside::TableFooterGroup
        | DisplayInside::Contents => node.children.iter().fold(0u16, |total, &child| {
            total.saturating_add(table_row_count(doc, child))
        }),
        _ => 0,
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_table_cells(
    doc: &mut BaseDocument,
    node_id: NodeId,
    wm: WritingMode,
    is_fixed: bool,
    border_collapse: BorderCollapse,
    row: &mut u16,
    cursor: &mut ColumnCursor,
    cells: &mut Vec<TableCell>,
    oof_children: &mut Vec<(NodeId, taffy::Position)>,
    rows: &mut Vec<TableRow>,
    columns: &mut Vec<TrackSizingFunction>,
    percent_columns: &mut Vec<(usize, f32, f32)>,
    section_end: u16,
) {
    let scale = doc.nodes.device_scale();
    let node = &mut doc.nodes[node_id];

    if !node.is_element() {
        return;
    }

    let Some(display) = node.display_style() else {
        #[cfg(feature = "tracing")]
        tracing::info!("Ignoring table descendent because it has no styles");
        return;
    };

    if display.outside() == DisplayOutside::None {
        node.remove_damage(CONSTRUCT_DESCENDENT | CONSTRUCT_FC | CONSTRUCT_BOX);
        return;
    }

    // Absolutely positioned boxes are out-of-flow: they are laid out by their containing
    // block rather than as part of the table grid.
    let position = node.primary_styles().unwrap().clone_position();
    if position.is_absolutely_positioned() {
        oof_children.push((node_id, stylo_taffy::convert::position(position)));
        return;
    }

    match display.inside() {
        DisplayInside::TableRowGroup
        | DisplayInside::TableHeaderGroup
        | DisplayInside::TableFooterGroup
        | DisplayInside::Contents => {
            let first_row = rows.len();
            let section_end = if display.inside() == DisplayInside::Contents {
                section_end
            } else {
                cursor.rowspans.fill(0);
                row.saturating_add(table_row_count(doc, node_id))
            };
            let children = std::mem::take(&mut doc.nodes[node_id].children);
            for child_id in children.iter().copied() {
                doc.nodes[child_id]
                    .remove_damage(CONSTRUCT_DESCENDENT | CONSTRUCT_FC | CONSTRUCT_BOX);
                collect_table_cells(
                    doc,
                    child_id,
                    wm,
                    is_fixed,
                    border_collapse,
                    row,
                    cursor,
                    cells,
                    oof_children,
                    rows,
                    columns,
                    percent_columns,
                    section_end,
                );
            }
            doc.nodes[node_id].children = children;
            if display.inside() != DisplayInside::Contents {
                cursor.rowspans.fill(0);
                let is_body = !matches!(
                    display.inside(),
                    DisplayInside::TableHeaderGroup | DisplayInside::TableFooterGroup
                );
                let is_collapsed = doc.nodes[node_id].primary_styles().is_some_and(|style| {
                    style.clone_visibility() == style::computed_values::visibility::T::Collapse
                });
                for row in rows[first_row..]
                    .iter_mut()
                    .filter(|row| row.group.is_none())
                {
                    row.group = Some(node_id);
                    row.is_body = is_body;
                    row.is_collapsed |= is_collapsed;
                }
            }
        }
        DisplayInside::TableRow => {
            node.remove_damage(CONSTRUCT_DESCENDENT | CONSTRUCT_FC | CONSTRUCT_BOX);
            *row += 1;
            cursor.start_row();

            let specified = node
                .primary_styles()
                .map(|style| stylo_taffy::convert::dimension(style.get_height()));
            let height = specified
                .filter(|height| height.tag() == taffy::CompactLength::LENGTH_TAG)
                .map(|height| height.value());
            let percent = specified
                .filter(|height| height.tag() == taffy::CompactLength::PERCENT_TAG)
                .map(|height| height.value());
            rows.push(TableRow {
                node_id,
                group: None,
                height,
                is_constrained: height.is_some(),
                percent,
                is_body: true,
                is_collapsed: node.primary_styles().is_some_and(|style| {
                    style.clone_visibility() == style::computed_values::visibility::T::Collapse
                }),
            });

            let children = std::mem::take(&mut doc.nodes[node_id].children);
            for child_id in children.iter().copied() {
                collect_table_cells(
                    doc,
                    child_id,
                    wm,
                    is_fixed,
                    border_collapse,
                    row,
                    cursor,
                    cells,
                    oof_children,
                    rows,
                    columns,
                    percent_columns,
                    section_end,
                );
            }
            doc.nodes[node_id].children = children;
        }
        DisplayInside::TableCell => {
            // node.remove_damage(CONSTRUCT_DESCENDENT | CONSTRUCT_FC | CONSTRUCT_BOX);
            let stylo_style = &node.primary_styles().unwrap();
            let colspan: u16 = node
                .attr(local_name!("colspan"))
                .and_then(|val| val.parse::<u32>().ok())
                .and_then(|span| u16::try_from(span.clamp(1, 1000)).ok())
                .unwrap_or(1);
            let rowspan: u16 = node
                .attr(local_name!("rowspan"))
                .and_then(|val| val.parse::<u16>().ok())
                .map(|v| if v == 0 { 65534 } else { v.min(65534) })
                .unwrap_or(1)
                .min(section_end.saturating_sub(*row).saturating_add(1));
            let mut style = table_taffy_style(stylo_style, wm, scale);
            style.inset = style_helpers::auto();
            let col = cursor.next_free();

            if border_collapse == BorderCollapse::Collapse {
                let border = style.border.resolve_or_zero(None, resolve_calc_value);
                style.border = border.map(|width| style_helpers::length(width / 2.0));
            }

            // In the fixed table layout algorithm the widths of columns are not
            // affected by cell contents, so cells must not impose a min-content
            // floor on their tracks.
            if is_fixed {
                style.min_size.width = style_helpers::length(0.0);
            }

            // Column widths from `<col>` elements take precedence over cell widths
            let col_needs_width = col >= columns.len() || columns[col].max.is_auto();
            let mut column_width_border = None;
            if *row == 1 && col_needs_width {
                let column: TrackSizingFunction = match style.size.width.tag() {
                    taffy::CompactLength::LENGTH_TAG => {
                        let len = style.size.width.value();
                        let padding = style.padding.resolve_or_zero(None, resolve_calc_value);
                        let border = style.border.resolve_or_zero(None, resolve_calc_value);
                        match style.box_sizing {
                            taffy::BoxSizing::ContentBox => {
                                column_width_border = Some(border.left + border.right);
                                style_helpers::length(
                                    len + padding.left + padding.right + border.left + border.right,
                                )
                            }
                            taffy::BoxSizing::BorderBox => style_helpers::length(len),
                        }
                    }
                    taffy::CompactLength::PERCENT_TAG => {
                        if is_fixed {
                            let extra = match style.box_sizing {
                                taffy::BoxSizing::ContentBox => {
                                    let padding =
                                        style.padding.resolve_or_zero(None, resolve_calc_value);
                                    let border =
                                        style.border.resolve_or_zero(None, resolve_calc_value);
                                    column_width_border = Some(border.left + border.right);
                                    padding.left + padding.right + border.left + border.right
                                }
                                taffy::BoxSizing::BorderBox => 0.0,
                            };
                            // Resolved in `build_table_context` once the column count is known
                            percent_columns.push((col, style.size.width.value(), extra));
                            style_helpers::percent(style.size.width.value())
                        } else {
                            style_helpers::auto()
                        }
                    }
                    taffy::CompactLength::AUTO_TAG => style_helpers::auto(),
                    // Dimension values are always length, percentage, auto or calc(),
                    // so any other tag is a calc() value. Pass it through so that
                    // Taffy resolves it against the table's inner width.
                    _ => style.size.width.into(),
                };
                if col < columns.len() {
                    if !column.max.is_auto() {
                        columns[col] = column;
                    }
                } else {
                    columns.resize(col, style_helpers::auto());
                    columns.push(column);
                }
            }

            // The margin properties do not apply to table-internal elements
            style.margin = taffy::Rect::ZERO.map(style_helpers::length);

            // Let Taffy auto-place the column. Combined with
            // `grid_auto_flow: RowDense` set on the table root, each cell
            // scans from the first track in its row for a free position,
            // which makes cells automatically skip columns occupied by
            // rowspan cells from earlier rows.
            style.grid_column = taffy::Line {
                start: style_helpers::auto(),
                end: style_helpers::span(colspan),
            };
            style.grid_row = taffy::Line {
                start: style_helpers::line(*row as i16),
                end: style_helpers::span(rowspan),
            };
            if style.size.height.tag() == taffy::CompactLength::LENGTH_TAG {
                if rowspan == 1
                    && let Some(row) = rows.last_mut()
                {
                    row.is_constrained = true;
                }
                style.min_size.height = style_helpers::length(style.size.height.value());
                style.size.height = style_helpers::auto();
            } else if style.size.height.tag() == taffy::CompactLength::PERCENT_TAG {
                if rowspan == 1
                    && let Some(row) = rows.last_mut()
                {
                    let percent = style.size.height.value();
                    row.percent = Some(row.percent.map_or(percent, |seen| seen.max(percent)));
                }
                style.size.height = style_helpers::auto();
            } else {
                style.size.height = style_helpers::auto();
            }

            let percent_width = (style.size.width.tag() == taffy::CompactLength::PERCENT_TAG)
                .then(|| style.size.width.value());
            style.size.width = style_helpers::auto();
            cells.push(TableCell {
                node_id,
                row: row.saturating_sub(1),
                column: col,
                column_span: usize::from(colspan),
                row_span: rowspan,
                column_width_border,
                percent_width,
                style,
            });

            cursor.place(usize::from(colspan), rowspan);
        }
        DisplayInside::Flow
        | DisplayInside::FlowRoot
        | DisplayInside::Flex
        | DisplayInside::Grid
        | DisplayInside::WebkitBox
        | DisplayInside::GridLanes => {
            node.remove_damage(CONSTRUCT_DESCENDENT | CONSTRUCT_FC | CONSTRUCT_BOX);
            // Probably a table caption: ignore
            // println!(
            //     "Warning: ignoring non-table typed descendent of table ({:?})",
            //     display.inside()
            // );
        }
        DisplayInside::TableColumnGroup | DisplayInside::TableColumn | DisplayInside::Table => {
            node.remove_damage(CONSTRUCT_DESCENDENT | CONSTRUCT_FC | CONSTRUCT_BOX);
            //Ignore
        }
        DisplayInside::None => {
            node.remove_damage(CONSTRUCT_DESCENDENT | CONSTRUCT_FC | CONSTRUCT_BOX);
            // Ignore
        }
    }
}

fn track_length(track: &TrackSizingFunction) -> Option<f32> {
    let max = track.max.into_raw();
    (max.tag() == taffy::CompactLength::LENGTH_TAG).then(|| max.value())
}

fn count(len: usize) -> f32 {
    u16::try_from(len).map_or(f32::from(u16::MAX), f32::from)
}

fn grow(values: &mut [f32], weights: &[f32], need: f32) {
    let extra = need - values.iter().sum::<f32>();
    if extra <= 0.0 {
        return;
    }
    let total: f32 = weights.iter().sum();
    let even = extra / count(values.len());
    for (value, weight) in values.iter_mut().zip(weights) {
        *value += if total > 0.0 {
            extra * weight / total
        } else {
            even
        };
    }
}

fn grow_mins(mins: &mut [f32], maxes: &[f32], weights: &[f32], need: f32) {
    let total_min: f32 = mins.iter().sum();
    let total_max: f32 = mins.iter().zip(maxes).map(|(min, max)| max.max(*min)).sum();
    if need <= total_min {
        return;
    }
    if need >= total_max {
        for (min, max) in mins.iter_mut().zip(maxes) {
            *min = max.max(*min);
        }
        grow(mins, weights, need);
        return;
    }
    let share = (need - total_min) / (total_max - total_min);
    for (min, max) in mins.iter_mut().zip(maxes) {
        *min += (max - *min).max(0.0) * share;
    }
}

fn spread_rows(rows: &[TableRow], heights: &mut [f32], spare: f32, gap: f32) {
    let mut sections: Vec<Range<usize>> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        match sections.last_mut() {
            Some(last) if rows[last.start].group == row.group => last.end = index + 1,
            _ => sections.push(index..index + 1),
        }
    }
    if rows.iter().any(|row| row.is_body) {
        sections.retain(|section| rows[section.start].is_body);
    }

    let sums: Vec<f32> = sections
        .iter()
        .map(|section| {
            heights[section.clone()].iter().sum::<f32>()
                + gap * count(section.len().saturating_sub(1))
        })
        .collect();
    let mut totals = sums.clone();
    grow(&mut totals, &sums, sums.iter().sum::<f32>() + spare);

    let is_open = |row: &TableRow| !row.is_constrained && row.percent.is_none();
    for ((section, sum), total) in sections.into_iter().zip(sums).zip(totals) {
        let mut unclaimed = 1.0_f32;
        let growth: Vec<f32> = section
            .clone()
            .map(|index| {
                let share = rows[index].percent.unwrap_or(0.0).clamp(0.0, unclaimed);
                unclaimed -= share;
                (total * share - heights[index]).max(0.0)
            })
            .collect();
        let requested: f32 = growth.iter().sum();
        let available = total - sum;
        let scale = if requested > available {
            available / requested
        } else {
            1.0
        };
        for (index, extra) in section.clone().zip(growth) {
            heights[index] += extra * scale;
        }
        let remaining = (available - requested).max(0.0);
        let has_open = rows[section.clone()].iter().any(is_open);
        let picked: Vec<usize> = section
            .filter(|&index| !has_open || is_open(&rows[index]))
            .collect();
        let weights: Vec<f32> = picked.iter().map(|&index| heights[index]).collect();
        let mut grown = weights.clone();
        grow(
            &mut grown,
            &weights,
            weights.iter().sum::<f32>() + remaining,
        );
        for (index, height) in picked.into_iter().zip(grown) {
            heights[index] = height;
        }
    }
}

fn share_columns(
    mins: &[f32],
    maxes: &[f32],
    space: f32,
    is_stretched: bool,
    unit: f32,
) -> Vec<f32> {
    let min_sum: f32 = mins.iter().sum();
    let max_sum: f32 = maxes.iter().sum();
    if space <= min_sum {
        return mins.to_vec();
    }
    if space < max_sum {
        let distributable = space - min_sum;
        let units = |length: f32| (f64::from(length) / f64::from(unit)).round();
        let total_units: f64 = mins
            .iter()
            .zip(maxes)
            .map(|(min, max)| units(max - min))
            .sum();
        if total_units <= 0.0 {
            let part = distributable / (max_sum - min_sum);
            return mins
                .iter()
                .zip(maxes)
                .map(|(min, max)| min + (max - min) * part)
                .collect();
        }
        let spare_units = (f64::from(distributable) / f64::from(unit) + 0.01)
            .floor()
            .min(total_units);
        let last = maxes.iter().zip(mins).rposition(|(max, min)| max > min);
        let mut left = distributable;
        return mins
            .iter()
            .zip(maxes)
            .enumerate()
            .map(|(index, (min, max))| {
                if Some(index) == last {
                    return min + left.max(0.0);
                }
                let share = (spare_units * units(max - min) / total_units).floor();
                let delta = share.to_f32().unwrap_or(0.0) * unit;
                left -= delta;
                min + delta
            })
            .collect();
    }
    let mut widths = maxes.to_vec();
    if is_stretched && space.is_finite() {
        grow(&mut widths, maxes, space);
    }
    widths
}

struct ColumnLimits {
    mins: Vec<f32>,
    maxes: Vec<f32>,
    percents: Vec<Option<f32>>,
}

fn column_widths(
    limits: &ColumnLimits,
    fixed: &[Option<f32>],
    space: f32,
    is_stretched: bool,
    unit: f32,
) -> Vec<f32> {
    let mut widths = limits.mins.clone();
    let mut rest = space;
    let mut shared = Vec::new();
    let mut scaled = Vec::new();
    for (index, fixed) in fixed.iter().enumerate() {
        let percent = limits.percents[index].filter(|_| is_stretched);
        match (fixed, percent) {
            (Some(_), _) => {}
            (None, Some(percent)) => {
                widths[index] = (percent * space).max(limits.mins[index]);
                scaled.push(index);
            }
            (None, None) => {
                shared.push(index);
                continue;
            }
        }
        rest -= widths[index];
    }
    let mins: Vec<f32> = shared.iter().map(|&index| limits.mins[index]).collect();
    let missing = mins.iter().sum::<f32>() - rest;
    let slack: f32 = scaled
        .iter()
        .map(|&index| widths[index] - limits.mins[index])
        .sum();
    if missing > 0.0 && slack > 0.0 {
        let part = (missing / slack).min(1.0);
        for index in scaled {
            let cut = (widths[index] - limits.mins[index]) * part;
            widths[index] -= cut;
            rest += cut;
        }
    }
    let maxes: Vec<f32> = shared.iter().map(|&index| limits.maxes[index]).collect();
    for (index, width) in shared
        .iter()
        .zip(share_columns(&mins, &maxes, rest, is_stretched, unit))
    {
        widths[*index] = width;
    }
    widths
}

fn percent_fit(limits: &ColumnLimits, fixed: &[Option<f32>]) -> Option<f32> {
    let mut fit: f32 = limits.maxes.iter().sum();
    let mut claimed = 0.0_f32;
    let mut rest = 0.0_f32;
    for (index, max) in limits.maxes.iter().enumerate() {
        match limits.percents[index] {
            Some(percent) if fixed[index].is_none() && percent > 0.0 => {
                claimed += percent;
                fit = fit.max(max / percent);
            }
            _ => rest += max,
        }
    }
    if claimed <= 0.0 {
        return None;
    }
    if rest > 0.0 {
        fit = if claimed < 1.0 {
            fit.max(rest / (1.0 - claimed))
        } else {
            f32::INFINITY
        };
    }
    Some(fit)
}

impl TableContext {
    fn row_width(&self) -> f32 {
        let info = self.computed_grid_info.borrow();
        let Some(info) = info.as_ref() else {
            return 0.0;
        };
        let lines = info
            .columns
            .positions
            .iter()
            .skip(usize::from(info.columns.negative_implicit_tracks));
        let (low, high) = lines.fold((f32::MAX, f32::MIN), |(low, high), line| {
            (
                low.min(line.start).min(line.end),
                high.max(line.start).max(line.end),
            )
        });
        (high - low).max(0.0)
    }

    fn offset_basis(&self) -> taffy::Size<f32> {
        taffy::Size {
            width: self.row_width(),
            height: 0.0,
        }
    }

    pub fn group_offset(&self, group: &crate::Node, is_rtl: bool) -> taffy::Point<f32> {
        group.relative_offset(self.offset_basis(), is_rtl)
    }

    pub fn cell_offset(&self, cell: &crate::Node, is_rtl: bool) -> taffy::Point<f32> {
        let basis = self.offset_basis();
        let mut offset = taffy::Point::ZERO;
        let mut part = Some(cell);
        while let Some(node) = part.filter(|node| !node.flags.is_table_root()) {
            let own = node.relative_offset(basis, is_rtl);
            offset.x += own.x;
            offset.y += own.y;
            part = node.parent.map(|id| node.with(id));
        }
        offset
    }

    pub fn has_collapsed_columns(&self) -> bool {
        self.has_collapsed_columns
    }

    pub fn column_cell_is_collapsed(&self, node_id: NodeId) -> bool {
        self.collapsed_column_cells.contains(&node_id)
    }

    pub fn cell_is_collapsed(&self, node_id: NodeId) -> bool {
        self.collapsed_cells.contains(&node_id)
    }

    pub fn has_collapsed_rows(&self) -> bool {
        self.rows.iter().any(|row| row.is_collapsed)
    }

    pub fn collapsed_cell_span_height(&self, node_id: NodeId) -> Option<f32> {
        if !self.cell_is_collapsed(node_id) {
            return None;
        }
        let cell = self.cells.iter().find(|cell| cell.node_id == node_id)?;
        let start = usize::from(cell.row);
        if !self.rows.get(start)?.is_collapsed || cell.row_span <= 1 {
            return None;
        }
        let info = self.computed_grid_info.borrow();
        let info = info.as_ref()?;
        let start = start + usize::from(info.rows.negative_implicit_tracks);
        let end = start + usize::from(cell.row_span) - 1;
        Some(
            info.rows.positions.get(end)?.end.round()
                - info.rows.positions.get(start)?.start.round(),
        )
    }
}

impl TableTreeWrapper<'_, '_> {
    fn intrinsic_width(
        &mut self,
        node_id: NodeId,
        inputs: &taffy::tree::LayoutInput,
        space: AvailableSpace,
    ) -> f32 {
        let inputs = taffy::tree::LayoutInput {
            run_mode: taffy::RunMode::ComputeSize,
            sizing_mode: if space == AvailableSpace::MinContent {
                taffy::SizingMode::ContentSize
            } else {
                taffy::SizingMode::InherentSize
            },
            axis: taffy::RequestedAxis::Horizontal,
            known_dimensions: taffy::Size::NONE,
            parent_size: taffy::Size::NONE,
            available_space: taffy::Size {
                width: space,
                height: AvailableSpace::MaxContent,
            },
            ..*inputs
        };
        let output = self
            .doc
            .compute_child_layout(crate::taffy_node_id(node_id), inputs);
        output.size.width
    }

    fn column_limits(
        &mut self,
        inputs: &taffy::tree::LayoutInput,
        fixed: &[Option<f32>],
        preferred: &[Option<f32>],
        spacing: f32,
    ) -> ColumnLimits {
        let ctx = Arc::clone(&self.ctx);
        let columns = fixed.len();
        let mut mins = vec![0.0_f32; columns];
        let mut maxes = vec![0.0_f32; columns];
        let mut percents: Vec<Option<f32>> = vec![None; columns];
        for (index, column) in ctx
            .columns
            .iter()
            .enumerate()
            .take(columns)
            .filter(|_| ctx.has_collapsed_columns())
        {
            if let Some(style) = self.doc.nodes[column.node_id].primary_styles() {
                let width = stylo_taffy::convert::dimension(style.get_width());
                if width.tag() == taffy::CompactLength::PERCENT_TAG {
                    percents[index] = Some(width.value());
                }
            }
        }
        let mut spanning = Vec::new();
        for cell in &ctx.cells {
            let start = cell.column.min(columns);
            let end = (start + cell.column_span).min(columns);
            if start == end {
                continue;
            }
            let min = self.intrinsic_width(cell.node_id, inputs, AvailableSpace::MinContent);
            let max = self
                .intrinsic_width(cell.node_id, inputs, AvailableSpace::MaxContent)
                .max(min);
            if end - start > 1 {
                spanning.push((start..end, min, max));
                continue;
            }
            mins[start] = mins[start].max(min);
            maxes[start] = maxes[start].max(max);
            if let Some(percent) = cell.percent_width {
                percents[start] = Some(percents[start].map_or(percent, |seen| seen.max(percent)));
            }
        }
        for (index, preferred) in preferred.iter().enumerate() {
            if let Some(preferred) = preferred {
                maxes[index] = maxes[index].max(*preferred);
            }
        }
        for (index, fixed) in fixed.iter().enumerate() {
            if let Some(fixed) = fixed {
                let width = fixed.max(mins[index]);
                mins[index] = width;
                maxes[index] = width;
            }
        }

        spanning.sort_by_key(|(span, _, _)| span.len());
        for (span, min, max) in spanning {
            let all_fixed = span.clone().all(|index| fixed[index].is_some());
            let gaps = spacing * (count(span.len()) - 1.0);
            let weights: Vec<f32> = span
                .clone()
                .map(|index| match fixed[index] {
                    Some(_) if !all_fixed => 0.0,
                    _ => maxes[index].max(1.0),
                })
                .collect();
            grow_mins(
                &mut mins[span.clone()],
                &maxes[span.clone()],
                &weights,
                min - gaps,
            );
            grow(&mut maxes[span], &weights, max - gaps);
        }
        for (max, min) in maxes.iter_mut().zip(&mins) {
            *max = max.max(*min);
        }
        let mut unclaimed = 1.0_f32;
        for percent in percents.iter_mut().flatten() {
            *percent = percent.clamp(0.0, unclaimed);
            unclaimed -= *percent;
        }
        ColumnLimits {
            mins,
            maxes,
            percents,
        }
    }

    pub(crate) fn size_auto_columns(&mut self, inputs: &taffy::tree::LayoutInput) {
        let ctx = Arc::clone(&self.ctx);
        let Some(auto) = &ctx.auto_columns else {
            return;
        };
        let mut preferred: Vec<Option<f32>> = auto.tracks.iter().map(track_length).collect();
        for cell in &ctx.cells {
            let has_column_width = ctx.columns.get(cell.column).is_some_and(|column| {
                self.doc.nodes[column.node_id]
                    .primary_styles()
                    .is_some_and(|style| {
                        stylo_taffy::convert::dimension(style.get_width()).tag()
                            == taffy::CompactLength::LENGTH_TAG
                    })
            });
            if cell.column_span != 1 || has_column_width {
                continue;
            }
            let Some(style) = self.doc.nodes[cell.node_id].primary_styles() else {
                continue;
            };
            let width = stylo_taffy::convert::dimension(style.get_width());
            if width.tag() == taffy::CompactLength::LENGTH_TAG {
                let padding = cell
                    .style
                    .padding
                    .resolve_or_zero(inputs.parent_size.width, resolve_calc_value);
                let border = cell
                    .style
                    .border
                    .resolve_or_zero(inputs.parent_size.width, resolve_calc_value);
                let width = match cell.style.box_sizing {
                    taffy::BoxSizing::BorderBox => width.value(),
                    taffy::BoxSizing::ContentBox => {
                        width.value() + padding.left + padding.right + border.left + border.right
                    }
                };
                let preferred = &mut preferred[cell.column];
                *preferred = Some(preferred.map_or(width, |seen| seen.max(width)));
            }
        }
        let is_plain = auto
            .tracks
            .iter()
            .zip(&preferred)
            .all(|(track, fixed)| fixed.is_some() || track.max.is_auto());
        if !is_plain || preferred.is_empty() {
            return;
        }
        let mut fixed: Vec<Option<f32>> = preferred
            .iter()
            .enumerate()
            .map(|(index, width)| {
                let column = ctx.columns.get(index)?;
                let style = self.doc.nodes[column.node_id].primary_styles()?;
                (stylo_taffy::convert::dimension(style.get_width()).tag()
                    == taffy::CompactLength::LENGTH_TAG)
                    .then_some(*width)
                    .flatten()
            })
            .collect();
        let mut limits = self.column_limits(inputs, &fixed, &preferred, auto.spacing);

        let parent_width = inputs.parent_size.width;
        let padding = ctx
            .style
            .padding
            .resolve_or_zero(parent_width, resolve_calc_value);
        let border = ctx
            .style
            .border
            .resolve_or_zero(parent_width, resolve_calc_value);
        let frame = padding.left + padding.right + border.left + border.right;
        let gaps = auto.spacing * (count(fixed.len()) - 1.0);
        let outer = |width: f32| match ctx.style.box_sizing {
            taffy::BoxSizing::BorderBox => width,
            taffy::BoxSizing::ContentBox => width + frame,
        };
        let min_outer = ctx
            .style
            .min_size
            .width
            .maybe_resolve(parent_width, resolve_calc_value)
            .map_or(0.0, outer);
        let max_outer = ctx
            .style
            .max_size
            .width
            .maybe_resolve(parent_width, resolve_calc_value)
            .map_or(f32::INFINITY, outer)
            .max(min_outer);
        let specified = inputs.known_dimensions.width.or_else(|| {
            let width = ctx
                .style
                .size
                .width
                .maybe_resolve(parent_width, resolve_calc_value)?;
            Some(outer(width).min(max_outer).max(min_outer))
        });
        let space = match (specified, inputs.available_space.width) {
            (Some(width), _) => width - frame - gaps,
            (None, AvailableSpace::Definite(width)) => width.min(max_outer) - frame - gaps,
            (None, AvailableSpace::MinContent) => 0.0,
            (None, AvailableSpace::MaxContent) => max_outer - frame - gaps,
        };
        let is_stretched = specified.is_some();
        if ctx.has_collapsed_columns() && is_stretched && preferred.iter().all(Option::is_some) {
            fixed.fill(None);
        }
        let preferred_floor: f32 = limits
            .mins
            .iter()
            .zip(&preferred)
            .map(|(min, preferred)| min.max(preferred.unwrap_or(0.0)))
            .sum();
        if space >= preferred_floor && preferred.iter().any(Option::is_none) {
            for (index, preferred) in preferred.iter().enumerate() {
                if let Some(preferred) = preferred {
                    limits.mins[index] = limits.mins[index].max(*preferred);
                    fixed[index] = Some(limits.mins[index]);
                }
            }
        }

        let scale = self.doc.viewport().scale();
        let unit = if scale.is_finite() && scale > 0.0 {
            1.0 / (64.0 * scale)
        } else {
            1.0 / 64.0
        };
        let mut widths = column_widths(&limits, &fixed, space, is_stretched, unit);
        if !is_stretched && let Some(fit) = percent_fit(&limits, &fixed) {
            let fit = fit.min(space);
            if fit.is_finite() && fit > widths.iter().sum::<f32>() {
                widths = column_widths(&limits, &fixed, fit, true, unit);
            }
        }
        let floor = self.caption_floor(inputs).max(min_outer);
        if widths.iter().sum::<f32>() + frame + gaps < floor {
            widths = column_widths(&limits, &fixed, floor - frame - gaps, true, unit);
        }

        let mut style = ctx.style.clone();
        let content = widths.iter().sum::<f32>() + gaps;
        if specified.unwrap_or(max_outer) < content + frame {
            style.min_size.width = style_helpers::length(match style.box_sizing {
                taffy::BoxSizing::BorderBox => content + frame,
                taffy::BoxSizing::ContentBox => content,
            });
        }
        style.grid_template_columns = widths
            .into_iter()
            .map(|width| {
                let track: TrackSizingFunction = style_helpers::length(width);
                track.into()
            })
            .collect();
        self.style = Some(style);
    }

    pub(crate) fn spread_spare_height(
        &mut self,
        inputs: &taffy::tree::LayoutInput,
        output: &taffy::tree::LayoutOutput,
    ) -> bool {
        let ctx = Arc::clone(&self.ctx);
        if !ctx.spreads_rows || inputs.run_mode != taffy::RunMode::PerformLayout {
            return false;
        }
        let lines: Vec<taffy::Line<f32>> = match ctx.computed_grid_info.borrow().as_ref() {
            Some(info) => info
                .rows
                .positions
                .iter()
                .skip(usize::from(info.rows.negative_implicit_tracks))
                .take(ctx.rows.len())
                .copied()
                .collect(),
            None => return false,
        };
        let Some(last) = lines.last().filter(|_| lines.len() == ctx.rows.len()) else {
            return false;
        };

        let parent_width = inputs.parent_size.width;
        let style = self.style.as_ref().unwrap_or(&ctx.style);
        let padding = style
            .padding
            .resolve_or_zero(parent_width, resolve_calc_value);
        let border = style
            .border
            .resolve_or_zero(parent_width, resolve_calc_value);
        let spare = output.size.height - padding.bottom - border.bottom - last.end;
        if spare < 0.01 {
            return false;
        }

        let mut heights: Vec<f32> = lines.iter().map(|line| line.end - line.start).collect();
        let gap = lines[1].start - lines[0].end;
        spread_rows(&ctx.rows, &mut heights, spare, gap);
        let mut style = self.style.take().unwrap_or_else(|| ctx.style.clone());
        style.grid_template_rows = heights
            .into_iter()
            .map(|height| {
                let track: TrackSizingFunction = style_helpers::length(height);
                track.into()
            })
            .collect();
        self.style = Some(style);
        true
    }

    pub(crate) fn size_spanning_rows(&mut self, inputs: &taffy::tree::LayoutInput) -> bool {
        let ctx = Arc::clone(&self.ctx);
        if !ctx.rows.iter().any(|row| row.is_collapsed)
            || !ctx.cells.iter().any(|cell| cell.row_span > 1)
        {
            return false;
        }
        let gap = ctx
            .style
            .gap
            .height
            .resolve_or_zero(inputs.parent_size.height, resolve_calc_value);
        let mut heights: Vec<f32> = ctx
            .rows
            .iter()
            .map(|row| row.height.unwrap_or(0.0))
            .collect();
        let mut spanning = Vec::new();
        for cell in &ctx.cells {
            let width = self.doc.nodes[cell.node_id].unrounded_layout().size.width;
            let measured = self.doc.compute_child_layout(
                crate::taffy_node_id(cell.node_id),
                taffy::tree::LayoutInput {
                    run_mode: taffy::RunMode::ComputeSize,
                    known_dimensions: taffy::Size {
                        width: Some(width),
                        height: None,
                    },
                    available_space: taffy::Size {
                        width: AvailableSpace::Definite(width),
                        height: AvailableSpace::MaxContent,
                    },
                    ..*inputs
                },
            );
            let start = usize::from(cell.row);
            if cell.row_span == 1 {
                heights[start] = heights[start].max(measured.size.height);
            } else {
                spanning.push((
                    start..start + usize::from(cell.row_span),
                    measured.size.height,
                ));
            }
        }
        spanning.sort_by_key(|(span, _)| span.len());
        for (span, height) in spanning {
            let weights = heights[span.clone()].to_vec();
            let need = height - gap * count(span.len().saturating_sub(1));
            grow(&mut heights[span], &weights, need);
        }
        let mut style = self.style.take().unwrap_or_else(|| ctx.style.clone());
        style.grid_template_rows = heights
            .into_iter()
            .map(|height| {
                let track: TrackSizingFunction = style_helpers::length(height);
                track.into()
            })
            .collect();
        self.style = Some(style);
        true
    }

    pub(crate) fn collapse_columns(
        &mut self,
        inputs: &taffy::tree::LayoutInput,
        output: &mut taffy::tree::LayoutOutput,
    ) {
        let ctx = Arc::clone(&self.ctx);
        if !ctx.has_collapsed_columns() {
            return;
        }
        let mut lines: Vec<taffy::Line<f32>> = match ctx.computed_grid_info.borrow().as_ref() {
            Some(info) => info
                .columns
                .positions
                .iter()
                .skip(usize::from(info.columns.negative_implicit_tracks))
                .copied()
                .collect(),
            None => return,
        };
        if lines.is_empty() {
            return;
        }
        let padding = ctx
            .style
            .padding
            .resolve_or_zero(inputs.parent_size.width, resolve_calc_value);
        let border = ctx
            .style
            .border
            .resolve_or_zero(inputs.parent_size.width, resolve_calc_value);
        let edge = lines
            .iter()
            .map(|line| line.start.max(line.end))
            .fold(0.0_f32, f32::max);
        output.size.width = output.size.width.max(edge + padding.right + border.right);
        let rtl = lines.get(1).is_some_and(|line| line.start < lines[0].start);
        if rtl {
            for line in &mut lines {
                *line = taffy::Line {
                    start: output.size.width - line.end,
                    end: output.size.width - line.start,
                };
            }
        }
        let collapsed = |index: usize| {
            ctx.columns
                .get(index)
                .is_some_and(|column| column.is_collapsed)
        };
        let gap = lines.get(1).map_or(0.0, |line| line.start - lines[0].end);
        let all_collapsed = (0..lines.len()).all(collapsed);
        let mut removed = vec![0.0; lines.len() + 1];
        let mut positions = Vec::with_capacity(lines.len());
        let mut seen_visible = false;
        for (index, line) in lines.iter().enumerate() {
            let before = removed[index];
            let mut start = line.start - before;
            let width = if collapsed(index) {
                if seen_visible {
                    start -= gap;
                }
                let removed_gap = if all_collapsed && index + 1 == lines.len() {
                    0.0
                } else {
                    gap
                };
                removed[index + 1] = before + line.end - line.start + removed_gap;
                0.0
            } else {
                removed[index + 1] = before;
                seen_visible = true;
                line.end - line.start
            };
            positions.push(taffy::Line {
                start,
                end: start + width,
            });
        }
        for cell in &ctx.cells {
            let start = cell.column;
            let end = start + cell.column_span;
            let mut layout = *self.doc.nodes[cell.node_id].unrounded_layout();
            layout.location.x = if rtl {
                output.size.width - removed[lines.len()] - positions[end - 1].end
            } else {
                positions[start].start
            };
            layout.size.width = (positions[end - 1].end - positions[start].start).max(0.0);
            let moved = self
                .doc
                .compute_child_layout(
                    crate::taffy_node_id(cell.node_id),
                    taffy::tree::LayoutInput {
                        known_dimensions: layout.size.map(Some),
                        ..*inputs
                    },
                )
                .oof_candidates;
            refresh_candidates(output, moved, layout.location);
            self.doc
                .set_unrounded_layout(crate::taffy_node_id(cell.node_id), &layout);
        }
        output.size.width -= removed[lines.len()];
        output.size.width = output.size.width.max(self.caption_floor(inputs));
        if rtl {
            for line in &mut positions {
                *line = taffy::Line {
                    start: output.size.width - line.end,
                    end: output.size.width - line.start,
                };
            }
        }
        if let Some(info) = ctx.computed_grid_info.borrow_mut().as_mut() {
            let first = usize::from(info.columns.negative_implicit_tracks);
            info.columns.positions[first..first + positions.len()].copy_from_slice(&positions);
        }
    }

    pub(crate) fn seat_on_first_row(
        &self,
        inputs: &taffy::tree::LayoutInput,
        output: &mut taffy::tree::LayoutOutput,
    ) {
        if inputs.run_mode != taffy::RunMode::PerformLayout {
            return;
        }
        let Some(first) = self.ctx.cells.iter().map(|cell| cell.row).min() else {
            return;
        };
        if first != 0 {
            let top = self
                .ctx
                .computed_grid_info
                .borrow()
                .as_ref()
                .and_then(|info| {
                    let skipped = usize::from(info.rows.negative_implicit_tracks);
                    info.rows.positions.get(skipped).map(|line| line.start)
                });
            if let Some(top) = top {
                output.baselines.first = Some(top);
            }
            return;
        }
        let mut edge: Option<f32> = None;
        let mut text_line: Option<f32> = None;
        let is_collapsed = self
            .ctx
            .rows
            .get(usize::from(first))
            .is_some_and(|row| row.is_collapsed);
        let cells = self.ctx.cells.iter().enumerate();
        for (index, cell) in cells.filter(|(_, cell)| cell.row == first) {
            let node = &self.doc.nodes[cell.node_id];
            let on_baseline = node.primary_styles().is_none_or(|style| {
                stylo_taffy::convert::table_cell_vertical_align(&style).is_none()
            });
            let layout = node.unrounded_layout();
            if is_collapsed {
                let top = layout.location.y;
                edge = Some(edge.map_or(top, |edge| edge.min(top)));
                continue;
            }
            if on_baseline {
                let Some(baseline) = self.cell_baselines.get(index).copied().flatten() else {
                    return;
                };
                let line = layout.location.y + baseline;
                text_line = Some(text_line.map_or(line, |other| other.max(line)));
                continue;
            }
            if cell.row_span != 1 {
                continue;
            }
            let bottom = layout.location.y + layout.size.height
                - layout.padding.bottom
                - layout.border.bottom;
            edge = Some(edge.map_or(bottom, |edge| edge.max(bottom)));
        }
        if let Some(baseline) = text_line.or(edge) {
            output.baselines.first = Some(baseline);
        }
    }

    pub(crate) fn collapse_rows(
        &mut self,
        inputs: &taffy::tree::LayoutInput,
        output: &mut taffy::tree::LayoutOutput,
    ) {
        let ctx = Arc::clone(&self.ctx);
        if !ctx.rows.iter().any(|row| row.is_collapsed) {
            return;
        }
        let lines: Vec<taffy::Line<f32>> = match ctx.computed_grid_info.borrow().as_ref() {
            Some(info) => info
                .rows
                .positions
                .iter()
                .skip(usize::from(info.rows.negative_implicit_tracks))
                .take(ctx.rows.len())
                .copied()
                .collect(),
            None => return,
        };
        if lines.len() != ctx.rows.len() || lines.is_empty() {
            return;
        }
        let gap = lines.get(1).map_or(0.0, |line| line.start - lines[0].end);
        let all_collapsed = ctx.rows.iter().all(|row| row.is_collapsed);
        let mut removed = vec![0.0; lines.len() + 1];
        let mut positions = Vec::with_capacity(lines.len());
        let mut seen_visible = false;
        for (index, (row, line)) in ctx.rows.iter().zip(&lines).enumerate() {
            let before = removed[index];
            let mut start = line.start - before;
            let height = if row.is_collapsed {
                if seen_visible {
                    start -= gap;
                }
                let removed_gap = if all_collapsed && index + 1 == lines.len() {
                    0.0
                } else {
                    gap
                };
                removed[index + 1] = before + line.end - line.start + removed_gap;
                0.0
            } else {
                removed[index + 1] = before;
                seen_visible = true;
                line.end - line.start
            };
            positions.push(taffy::Line {
                start,
                end: start + height,
            });
        }
        for cell in &ctx.cells {
            let start = usize::from(cell.row);
            let end = start + usize::from(cell.row_span);
            let mut layout = *self.doc.nodes[cell.node_id].unrounded_layout();
            let shift = positions[start].start - layout.location.y;
            layout.location.y = positions[start].start;
            let mut relaid = None;
            if ctx.rows[start].is_collapsed {
                layout.size.height = 0.0;
            } else {
                let height = (layout.size.height - (removed[end] - removed[start])).max(0.0);
                if height != layout.size.height {
                    layout.size.height = height;
                    let cell_output = self.doc.compute_child_layout(
                        crate::taffy_node_id(cell.node_id),
                        taffy::tree::LayoutInput {
                            known_dimensions: layout.size.map(Some),
                            ..*inputs
                        },
                    );
                    relaid = Some(cell_output.oof_candidates);
                }
            }
            match relaid {
                Some(moved) => refresh_candidates(output, moved, layout.location),
                None if shift != 0.0 => {
                    for candidate in output.oof_candidates.as_mut_slice() {
                        let node = crate::dom_node_id(candidate.node);
                        let mut ancestors =
                            std::iter::successors(Some(node), |&id| self.doc.nodes[id].parent);
                        if ancestors.any(|id| id == cell.node_id) {
                            candidate.static_position.y.area.start += shift;
                            candidate.static_position.y.area.end += shift;
                        }
                    }
                }
                None => {}
            }
            self.doc
                .set_unrounded_layout(crate::taffy_node_id(cell.node_id), &layout);
        }
        output.size.height -= removed[lines.len()];
        if let Some(info) = ctx.computed_grid_info.borrow_mut().as_mut() {
            let first = usize::from(info.rows.negative_implicit_tracks);
            info.rows.positions[first..first + positions.len()].copy_from_slice(&positions);
        }
    }

    fn caption_floor(&mut self, inputs: &taffy::tree::LayoutInput) -> f32 {
        let ctx = Arc::clone(&self.ctx);
        let mut floor = 0.0_f32;
        for caption in &ctx.captions {
            let content = self.intrinsic_width(caption.node_id, inputs, AvailableSpace::MinContent);
            let style = self.doc.nodes[caption.node_id].layout_style();
            let margin = taffy::CoreStyle::margin(&style).resolve_or_zero(None, resolve_calc_value);
            let specified = taffy::CoreStyle::size(&style)
                .width
                .maybe_resolve(None, resolve_calc_value)
                .map(|width| match taffy::CoreStyle::box_sizing(&style) {
                    taffy::BoxSizing::BorderBox => width,
                    taffy::BoxSizing::ContentBox => {
                        let padding = taffy::CoreStyle::padding(&style)
                            .resolve_or_zero(None, resolve_calc_value);
                        let border = taffy::CoreStyle::border(&style)
                            .resolve_or_zero(None, resolve_calc_value);
                        width + padding.left + padding.right + border.left + border.right
                    }
                });
            drop(style);
            floor = floor.max(specified.unwrap_or(content) + margin.left + margin.right);
        }
        floor
    }

    pub(crate) fn widen_for_captions(
        &mut self,
        inputs: taffy::tree::LayoutInput,
    ) -> taffy::tree::LayoutInput {
        let floor = self.caption_floor(&inputs);
        if floor <= 0.0 {
            return inputs;
        }
        let ctx = Arc::clone(&self.ctx);
        let parent_width = inputs.parent_size.width;
        let inner = match ctx.style.box_sizing {
            taffy::BoxSizing::BorderBox => floor,
            taffy::BoxSizing::ContentBox => {
                let padding = ctx
                    .style
                    .padding
                    .resolve_or_zero(parent_width, resolve_calc_value);
                let border = ctx
                    .style
                    .border
                    .resolve_or_zero(parent_width, resolve_calc_value);
                floor - padding.left - padding.right - border.left - border.right
            }
        };
        let mut style = self.style.take().unwrap_or_else(|| ctx.style.clone());
        let current = style
            .min_size
            .width
            .maybe_resolve(parent_width, resolve_calc_value)
            .unwrap_or(0.0);
        style.min_size.width = style_helpers::length(current.max(inner));
        self.style = Some(style);

        let mut widened = inputs;
        widened.known_dimensions.width =
            inputs.known_dimensions.width.map(|width| width.max(floor));
        widened
    }

    pub(crate) fn offset_cells(&mut self, table: NodeId, inputs: &taffy::tree::LayoutInput) {
        if inputs.run_mode != taffy::RunMode::PerformLayout {
            return;
        }
        let ctx = Arc::clone(&self.ctx);
        let is_rtl = self.doc.nodes[table]
            .primary_styles()
            .is_some_and(|style| style.clone_direction() == Direction::Rtl);
        for cell in &ctx.cells {
            let offset = ctx.cell_offset(&self.doc.nodes[cell.node_id], is_rtl);
            let location = &mut self.doc.nodes[cell.node_id].unrounded_layout_mut().location;
            location.x += offset.x;
            location.y += offset.y;
        }
    }

    pub(crate) fn place_captions(
        &mut self,
        inputs: &taffy::tree::LayoutInput,
        output: &mut taffy::LayoutOutput,
    ) {
        let ctx = Arc::clone(&self.ctx);
        if ctx.captions.is_empty() {
            return;
        }
        let width = output.size.width;
        let boxes: Vec<CaptionBox> = ctx
            .captions
            .iter()
            .map(|caption| {
                let (margin, auto, specified) = {
                    let style = self.doc.nodes[caption.node_id].layout_style();
                    let margin = taffy::CoreStyle::margin(&style);
                    let auto = [margin.left.is_auto(), margin.right.is_auto()];
                    let frame = taffy::CoreStyle::padding(&style)
                        .resolve_or_zero(Some(width), resolve_calc_value)
                        .horizontal_axis_sum()
                        + taffy::CoreStyle::border(&style)
                            .resolve_or_zero(Some(width), resolve_calc_value)
                            .horizontal_axis_sum();
                    let specified = taffy::CoreStyle::size(&style)
                        .width
                        .maybe_resolve(Some(width), resolve_calc_value)
                        .map(|width| match taffy::CoreStyle::box_sizing(&style) {
                            taffy::BoxSizing::BorderBox => width,
                            taffy::BoxSizing::ContentBox => width + frame,
                        });
                    (
                        margin.resolve_or_zero(Some(width), resolve_calc_value),
                        auto,
                        specified,
                    )
                };
                let space = width - margin.left - margin.right;
                let box_width = specified.unwrap_or(space.max(0.0));
                let caption_inputs = taffy::tree::LayoutInput {
                    sizing_mode: taffy::SizingMode::InherentSize,
                    axis: taffy::RequestedAxis::Both,
                    known_dimensions: taffy::Size {
                        width: Some(box_width),
                        height: None,
                    },
                    parent_size: taffy::Size {
                        width: Some(width),
                        height: None,
                    },
                    available_space: taffy::Size {
                        width: AvailableSpace::Definite(box_width),
                        height: AvailableSpace::MaxContent,
                    },
                    ..*inputs
                };
                let node_id = crate::taffy_node_id(caption.node_id);
                let size = self.doc.compute_child_layout(node_id, caption_inputs).size;
                let free = (space - size.width).max(0.0);
                let x = margin.left
                    + match auto {
                        [true, true] => free / 2.0,
                        [true, false] => free,
                        _ => 0.0,
                    };
                CaptionBox { size, margin, x }
            })
            .collect();
        let side_height = |is_below: bool| -> f32 {
            ctx.captions
                .iter()
                .zip(&boxes)
                .filter(|(caption, _)| caption.is_below == is_below)
                .map(|(_, placed)| placed.size.height + placed.margin.top + placed.margin.bottom)
                .sum()
        };
        let above = side_height(false);
        let grid_height = output.size.height;
        output.size.height += above + side_height(true);
        output.baselines.first = output.baselines.first.map(|baseline| baseline + above);
        if inputs.run_mode != taffy::RunMode::PerformLayout {
            return;
        }

        for cell in &ctx.cells {
            self.doc.nodes[cell.node_id]
                .unrounded_layout_mut()
                .location
                .y += above;
        }
        let first_order = ctx.cells.len() + ctx.oof_children.len();
        let mut top = 0.0;
        let mut bottom = above + grid_height;
        for (index, (caption, placed)) in ctx.captions.iter().zip(&boxes).enumerate() {
            let edge = if caption.is_below {
                &mut bottom
            } else {
                &mut top
            };
            let mut layout =
                taffy::Layout::with_order(u32::try_from(first_order + index).unwrap_or(u32::MAX));
            let basis = {
                let style = self.style.as_ref().unwrap_or(&ctx.style);
                let parent_width = inputs.parent_size.width;
                let frame = style
                    .padding
                    .resolve_or_zero(parent_width, resolve_calc_value)
                    + style
                        .border
                        .resolve_or_zero(parent_width, resolve_calc_value);
                let gap = style
                    .gap
                    .resolve_or_zero(taffy::Size::NONE, resolve_calc_value);
                let height = if style.size.height.is_auto() {
                    0.0
                } else {
                    grid_height - frame.vertical_axis_sum() + 2.0 * gap.height
                };
                taffy::Size {
                    width: width - frame.horizontal_axis_sum() + 2.0 * gap.width,
                    height,
                }
            };
            let is_rtl = self.doc.nodes[caption.node_id]
                .parent
                .and_then(|parent| self.doc.nodes[parent].primary_styles())
                .is_some_and(|style| style.clone_direction() == Direction::Rtl);
            let offset = self.doc.nodes[caption.node_id].relative_offset(basis, is_rtl);
            layout.location.x = placed.x + offset.x;
            layout.location.y = *edge + placed.margin.top + offset.y;
            layout.size = placed.size;
            layout.margin = placed.margin;
            *edge += placed.margin.top + placed.size.height + placed.margin.bottom;
            {
                let style = self.doc.nodes[caption.node_id].layout_style();
                layout.padding = taffy::CoreStyle::padding(&style)
                    .resolve_or_zero(Some(width), resolve_calc_value);
                layout.border = taffy::CoreStyle::border(&style)
                    .resolve_or_zero(Some(width), resolve_calc_value);
            }
            self.doc
                .set_unrounded_layout(crate::taffy_node_id(caption.node_id), &layout);
        }
    }
}

struct CaptionBox {
    size: taffy::Size<f32>,
    margin: taffy::Rect<f32>,
    x: f32,
}

pub struct RangeIter(Range<usize>);

impl Iterator for RangeIter {
    type Item = taffy::NodeId;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(taffy::NodeId::from)
    }
}

impl taffy::TraversePartialTree for TableTreeWrapper<'_, '_> {
    type ChildIter<'a>
        = RangeIter
    where
        Self: 'a;

    #[inline(always)]
    fn child_ids(&self, _node_id: taffy::NodeId) -> Self::ChildIter<'_> {
        RangeIter(0..self.ctx.cells.len())
    }

    #[inline(always)]
    fn child_count(&self, _node_id: taffy::NodeId) -> usize {
        self.ctx.cells.len()
    }

    #[inline(always)]
    fn get_child_id(&self, _node_id: taffy::NodeId, index: usize) -> taffy::NodeId {
        index.into()
    }
}
impl taffy::TraverseTree for TableTreeWrapper<'_, '_> {}

impl taffy::LayoutPartialTree for TableTreeWrapper<'_, '_> {
    type CoreContainerStyle<'a>
        = &'a taffy::Style<Atom>
    where
        Self: 'a;

    type CustomIdent = Atom;

    fn get_core_container_style(&self, _node_id: taffy::NodeId) -> &taffy::Style<Atom> {
        self.style.as_ref().unwrap_or(&self.ctx.style)
    }

    fn resolve_calc_value(&self, calc_ptr: *const (), parent_size: f32) -> f32 {
        resolve_calc_value(calc_ptr, parent_size)
    }

    fn set_unrounded_layout(&mut self, node_id: taffy::NodeId, layout: &taffy::Layout) {
        let node_id = crate::taffy_node_id(self.ctx.cells[usize::from(node_id)].node_id);
        self.doc.set_unrounded_layout(node_id, layout)
    }

    fn compute_child_layout(
        &mut self,
        node_id: taffy::NodeId,
        inputs: taffy::tree::LayoutInput,
    ) -> taffy::LayoutOutput {
        let index = usize::from(node_id);
        let cell = &self.ctx.cells[index];
        let node_id = crate::taffy_node_id(cell.node_id);
        let output = self.doc.compute_child_layout(node_id, inputs);
        if inputs.run_mode == taffy::RunMode::PerformLayout {
            if self.cell_baselines.len() <= index {
                self.cell_baselines.resize(index + 1, None);
            }
            self.cell_baselines[index] = output.baselines.first;
        }
        output
    }
}

impl taffy::LayoutGridContainer for TableTreeWrapper<'_, '_> {
    type GridContainerStyle<'a>
        = &'a taffy::Style<Atom>
    where
        Self: 'a;

    type GridItemStyle<'a>
        = &'a taffy::Style<Atom>
    where
        Self: 'a;

    fn get_grid_container_style(&self, node_id: taffy::NodeId) -> Self::GridContainerStyle<'_> {
        self.get_core_container_style(node_id)
    }

    fn get_grid_child_style(&self, child_node_id: taffy::NodeId) -> Self::GridItemStyle<'_> {
        &self.ctx.cells[usize::from(child_node_id)].style
    }

    fn set_detailed_grid_info(
        &mut self,
        _node_id: taffy::NodeId,
        detailed_grid_info: DetailedGridInfo<Atom>,
    ) {
        *self.ctx.computed_grid_info.borrow_mut() = Some(detailed_grid_info);
    }
}

#[cfg(test)]
mod tests {
    use super::share_columns;

    fn close(widths: &[f32], expected: &[f32]) -> bool {
        widths.len() == expected.len()
            && widths
                .iter()
                .zip(expected)
                .all(|(width, expected)| (width - expected).abs() < 0.01)
    }

    #[test]
    fn a_tight_table_keeps_its_narrowest_columns() {
        assert_eq!(
            share_columns(&[30.0, 50.0], &[60.0, 90.0], 70.0, true, 1.0 / 64.0),
            [30.0, 50.0]
        );
    }

    #[test]
    fn room_between_the_limits_goes_to_the_columns_that_can_grow() {
        let widths = share_columns(
            &[40.0, 50.0, 30.0],
            &[40.0, 450.0, 110.0],
            300.0,
            true,
            1.0 / 64.0,
        );
        assert!(close(&widths, &[40.0, 200.0, 60.0]), "{widths:?}");
    }

    #[test]
    fn spare_room_is_shared_by_widest_content() {
        let widths = share_columns(&[10.0, 20.0], &[50.0, 150.0], 300.0, true, 1.0 / 64.0);
        assert!(close(&widths, &[75.0, 225.0]), "{widths:?}");
    }

    #[test]
    fn a_table_without_a_width_stops_at_its_content() {
        assert_eq!(
            share_columns(&[10.0, 20.0], &[50.0, 150.0], 300.0, false, 1.0 / 64.0),
            [50.0, 150.0]
        );
    }

    #[test]
    fn off_grid_columns_share_exactly_the_spare_width() {
        let widths = share_columns(&[0.0, 0.0, 0.0], &[0.8, 0.8, 0.8], 1.6, true, 1.0);
        assert!(widths.iter().all(|width| *width >= 0.0), "{widths:?}");
        assert!(
            (widths.iter().sum::<f32>() - 1.6).abs() < 1e-5,
            "{widths:?}"
        );
        let widths = share_columns(&[0.0, 0.0], &[0.4, 0.4], 0.7, true, 1.0);
        assert!(
            widths.iter().all(|width| (width - 0.35).abs() < 1e-5),
            "{widths:?}"
        );
    }
}
