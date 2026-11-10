use blitz_traits::node_id::NodeId;
use std::{ops::Range, sync::Arc};
use style::logical_geometry::WritingMode;
use style::properties::ComputedValues;
use stylo_taffy::CopyStyleGetters;
use stylo_taffy::WritingModeExt;

use atomic_refcell::AtomicRefCell;
use markup5ever::local_name;
use style::typed_om::NumericBaseType;
use style::values::computed::calc::CalcPercentageLeaf;
use style::values::computed::length_percentage::{
    CalcLengthPercentage, CalcNode, ComputedLeaf, Unpacked as UnpackedLengthPercentage,
};
use style::values::computed::{Length, LengthPercentage};
use style::values::generics::Optional;
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
    pub column: u16,
    column_span: u16,
    row_span: u16,
    column_width_border: Option<f32>,
    percent_width: Option<f32>,
    style: taffy::Style<Atom>,
}

/// Tracks the current column position while walking the table's cells, so that
/// cells are assigned the same columns Taffy's dense auto-placement will give them
/// (skipping columns still occupied by rowspan cells from earlier rows).
#[derive(Debug, Default)]
struct ColumnCursor {
    /// The column the next cell in the current row will be placed in
    col: u16,
    /// The total number of columns seen so far
    num_columns: u16,
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
    fn next_free(&mut self) -> u16 {
        while self.rowspans.get(self.col as usize).is_some_and(|r| *r > 0) {
            self.col += 1;
        }
        self.col
    }

    fn place(&mut self, colspan: u16, rowspan: u16) {
        let end = self.col + colspan;
        if self.rowspans.len() < end as usize {
            self.rowspans.resize(end as usize, 0);
        }
        for remaining in &mut self.rowspans[self.col as usize..end as usize] {
            *remaining = rowspan;
        }
        self.col = end;
        self.num_columns = self.num_columns.max(end);
    }
}

#[derive(Debug, Clone)]
pub struct TableColumn {
    pub node_id: NodeId,
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
fn table_taffy_style(style: &ComputedValues, table_wm: WritingMode) -> taffy::Style<Atom> {
    stylo_taffy::to_taffy_style_in(style, table_wm)
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

fn resolve_collapsed_borders(
    doc: &mut BaseDocument,
    cells: &mut [TableCell],
    num_columns: u16,
    num_rows: usize,
    own: taffy::Rect<f32>,
    wm: WritingMode,
) -> taffy::Rect<taffy::LengthPercentage> {
    let is_ltr = wm.direction() == taffy::Direction::Ltr;
    let columns = usize::from(num_columns);
    let table = flip_inline(own.map(|width| width / 2.0), is_ltr);
    let halves: Vec<taffy::Rect<f32>> = cells
        .iter()
        .map(|cell| {
            let border = cell.style.border.resolve_or_zero(None, resolve_calc_value);
            flip_inline(border, is_ltr)
        })
        .collect();
    let span = |cell: &TableCell| {
        let first_column = usize::from(cell.column).min(columns);
        let first_row = usize::from(cell.row).min(num_rows);
        let end_column = (first_column + usize::from(cell.column_span)).min(columns);
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

    let mut outer = table;
    for (index, cell) in cells.iter_mut().enumerate() {
        let (cell_columns, cell_rows) = span(cell);
        let own = halves[index];
        let mut half = own;
        if cell_columns.start == 0 {
            half.left = half.left.max(table.left);
            outer.left = outer.left.max(half.left);
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
            outer.right = outer.right.max(half.right);
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
            outer.top = outer.top.max(half.top);
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
            outer.bottom = outer.bottom.max(half.bottom);
        } else {
            let slots = cell_columns
                .clone()
                .map(|column| cell_rows.end * columns + column);
            half.bottom = half
                .bottom
                .max(widest(slots, &grid, |cell| halves[cell].top));
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
    let root_node = &mut doc.nodes[table_root_node_id];

    let children = std::mem::take(&mut root_node.children);

    let Some(stylo_styles) = root_node.primary_styles() else {
        panic!("Ignoring table because it has no styles");
    };

    let mut style = table_taffy_style(&stylo_styles, wm);
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
    let mut percent_columns: Vec<(u16, f32, f32)> = Vec::new();
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
            );
        }
    }
    let remaining_column = if is_fixed {
        style_helpers::minmax(style_helpers::length(0.0), style_helpers::fr(1.0))
    } else {
        style_helpers::auto()
    };
    // Trailing `<col>` columns which contain no cells and have no definite width
    // (i.e. would be zero-width) are dropped, so they don't add border-spacing.
    while column_sizes.len() > cursor.num_columns as usize
        && column_sizes
            .last()
            .is_some_and(|c| c.min.into_raw().tag() == taffy::CompactLength::AUTO_TAG)
    {
        column_sizes.pop();
        columns.pop();
    }
    let num_columns = cursor.num_columns.max(column_sizes.len() as u16);
    column_sizes.resize(num_columns as usize, remaining_column);
    if border_collapse == BorderCollapse::Collapse {
        let own = style.border.resolve_or_zero(None, resolve_calc_value);
        style.border = resolve_collapsed_borders(doc, &mut cells, num_columns, rows.len(), own, wm);
        for cell in &cells {
            let Some(counted) = cell.column_width_border else {
                continue;
            };
            let border = cell.style.border.resolve_or_zero(None, resolve_calc_value);
            let growth = border.left + border.right - counted;
            if let Some(column) = column_sizes.get_mut(usize::from(cell.column)) {
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
            if let Some(column) = column_sizes.get_mut(col_idx as usize) {
                *column = percent_track(percent, extra);
            }
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
    let has_unconstrained_rows = rows.iter().any(|row| !row.is_constrained);
    style.grid_template_rows = rows
        .iter()
        .map(|row| {
            let min = match row.height {
                Some(height) => style_helpers::length(height),
                None => style_helpers::auto(),
            };
            let max = if row.is_constrained && has_unconstrained_rows {
                style_helpers::max_content()
            } else {
                style_helpers::auto()
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
    let node = &doc.nodes[node_id];
    if !node.is_element() {
        return;
    }
    let Some(display) = node.display_style() else {
        return;
    };

    match display.inside() {
        DisplayInside::TableColumnGroup => {
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
        DisplayInside::TableColumn => {
            let style = table_taffy_style(&node.primary_styles().unwrap(), wm);
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
                    group: None,
                });
                column_sizes.push(column);
            }
        }
        _ => {}
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
    percent_columns: &mut Vec<(u16, f32, f32)>,
) {
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
                );
            }
            doc.nodes[node_id].children = children;
            if display.inside() != DisplayInside::Contents {
                for row in &mut rows[first_row..] {
                    row.group.get_or_insert(node_id);
                }
            }
        }
        DisplayInside::TableRow => {
            node.remove_damage(CONSTRUCT_DESCENDENT | CONSTRUCT_FC | CONSTRUCT_BOX);
            *row += 1;
            cursor.start_row();

            let height = node.primary_styles().and_then(|style| {
                let height = stylo_taffy::convert::dimension(style.get_height());
                (height.tag() == taffy::CompactLength::LENGTH_TAG).then(|| height.value())
            });
            rows.push(TableRow {
                node_id,
                group: None,
                height,
                is_constrained: height.is_some(),
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
                .map(|v| v.clamp(1, 65534))
                .unwrap_or(1);
            let mut style = table_taffy_style(stylo_style, wm);
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
            let col_needs_width =
                (col as usize) >= columns.len() || columns[col as usize].max.is_auto();
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
                if (col as usize) < columns.len() {
                    if !column.max.is_auto() {
                        columns[col as usize] = column;
                    }
                } else {
                    columns.resize(col as usize, style_helpers::auto());
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
            if rowspan == 1 && style.size.height.tag() == taffy::CompactLength::LENGTH_TAG {
                if let Some(row) = rows.last_mut() {
                    row.is_constrained = true;
                }
            }

            let percent_width = (style.size.width.tag() == taffy::CompactLength::PERCENT_TAG)
                .then(|| style.size.width.value());
            style.size.width = style_helpers::auto();
            cells.push(TableCell {
                node_id,
                row: row.saturating_sub(1),
                column: col,
                column_span: colspan,
                row_span: rowspan,
                column_width_border,
                percent_width,
                style,
            });

            cursor.place(colspan, rowspan);
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

fn share_columns(mins: &[f32], maxes: &[f32], space: f32, is_stretched: bool) -> Vec<f32> {
    let min_sum: f32 = mins.iter().sum();
    let max_sum: f32 = maxes.iter().sum();
    if space <= min_sum {
        return mins.to_vec();
    }
    if space < max_sum {
        let part = (space - min_sum) / (max_sum - min_sum);
        return mins
            .iter()
            .zip(maxes)
            .map(|(min, max)| min + (max - min) * part)
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
) -> Vec<f32> {
    let mut widths = limits.mins.clone();
    let mut rest = space;
    let mut shared = Vec::new();
    for (index, fixed) in fixed.iter().enumerate() {
        let percent = limits.percents[index].filter(|_| is_stretched);
        match (fixed, percent) {
            (Some(_), _) => {}
            (None, Some(percent)) => {
                widths[index] = (percent * space).max(limits.mins[index]);
            }
            (None, None) => {
                shared.push(index);
                continue;
            }
        }
        rest -= widths[index];
    }
    let mins: Vec<f32> = shared.iter().map(|&index| limits.mins[index]).collect();
    let maxes: Vec<f32> = shared.iter().map(|&index| limits.maxes[index]).collect();
    for (index, width) in shared
        .iter()
        .zip(share_columns(&mins, &maxes, rest, is_stretched))
    {
        widths[*index] = width;
    }
    widths
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
        spacing: f32,
    ) -> ColumnLimits {
        let ctx = Arc::clone(&self.ctx);
        let columns = fixed.len();
        let mut mins = vec![0.0_f32; columns];
        let mut maxes = vec![0.0_f32; columns];
        let mut percents: Vec<Option<f32>> = vec![None; columns];
        let mut spanning = Vec::new();
        for cell in &ctx.cells {
            let start = usize::from(cell.column).min(columns);
            let end = (start + usize::from(cell.column_span)).min(columns);
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
        for (index, fixed) in fixed.iter().enumerate() {
            if let Some(fixed) = fixed {
                let width = fixed.max(mins[index]);
                mins[index] = width;
                maxes[index] = width;
            }
        }

        spanning.sort_by_key(|(span, _, _)| span.len());
        for (span, min, max) in spanning {
            if span.clone().all(|index| fixed[index].is_some()) {
                continue;
            }
            let gaps = spacing * (count(span.len()) - 1.0);
            let weights: Vec<f32> = span
                .clone()
                .map(|index| match fixed[index] {
                    Some(_) => 0.0,
                    None => maxes[index].max(1.0),
                })
                .collect();
            grow(&mut mins[span.clone()], &weights, min - gaps);
            grow(&mut maxes[span], &weights, max - gaps);
        }
        for (max, min) in maxes.iter_mut().zip(&mins) {
            *max = max.max(*min);
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
        let fixed: Vec<Option<f32>> = auto.tracks.iter().map(track_length).collect();
        let is_plain = auto
            .tracks
            .iter()
            .zip(&fixed)
            .all(|(track, fixed)| fixed.is_some() || track.max.is_auto());
        if !is_plain || fixed.is_empty() {
            return;
        }
        let limits = self.column_limits(inputs, &fixed, auto.spacing);

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
        let specified = inputs.known_dimensions.width.or_else(|| {
            let width = ctx
                .style
                .size
                .width
                .maybe_resolve(parent_width, resolve_calc_value)?;
            Some(match ctx.style.box_sizing {
                taffy::BoxSizing::BorderBox => width,
                taffy::BoxSizing::ContentBox => width + frame,
            })
        });
        let space = match (specified, inputs.available_space.width) {
            (Some(width), _) | (None, AvailableSpace::Definite(width)) => width - frame - gaps,
            (None, AvailableSpace::MinContent) => 0.0,
            (None, AvailableSpace::MaxContent) => f32::INFINITY,
        };
        let is_stretched = specified.is_some();

        let mut widths = column_widths(&limits, &fixed, space, is_stretched);
        let floor = self.caption_floor(inputs);
        if widths.iter().sum::<f32>() + frame + gaps < floor {
            widths = column_widths(&limits, &fixed, floor - frame - gaps, true);
        }

        let mut style = ctx.style.clone();
        style.grid_template_columns = widths
            .into_iter()
            .map(|width| {
                let track: TrackSizingFunction = style_helpers::length(width);
                track.into()
            })
            .collect();
        self.style = Some(style);
    }

    fn caption_floor(&mut self, inputs: &taffy::tree::LayoutInput) -> f32 {
        let ctx = Arc::clone(&self.ctx);
        let mut floor = 0.0_f32;
        for caption in &ctx.captions {
            let width = self.intrinsic_width(caption.node_id, inputs, AvailableSpace::MinContent);
            floor = floor.max(width);
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
        let caption_inputs = taffy::tree::LayoutInput {
            sizing_mode: taffy::SizingMode::InherentSize,
            axis: taffy::RequestedAxis::Both,
            known_dimensions: taffy::Size {
                width: Some(width),
                height: None,
            },
            parent_size: taffy::Size {
                width: Some(width),
                height: None,
            },
            available_space: taffy::Size {
                width: AvailableSpace::Definite(width),
                height: AvailableSpace::MaxContent,
            },
            ..*inputs
        };
        let sizes: Vec<taffy::Size<f32>> = ctx
            .captions
            .iter()
            .map(|caption| {
                let node_id = crate::taffy_node_id(caption.node_id);
                self.doc.compute_child_layout(node_id, caption_inputs).size
            })
            .collect();
        let side_height = |is_below: bool| -> f32 {
            ctx.captions
                .iter()
                .zip(&sizes)
                .filter(|(caption, _)| caption.is_below == is_below)
                .map(|(_, size)| size.height)
                .sum()
        };
        let above = side_height(false);
        let grid_height = output.size.height;
        output.size.height += above + side_height(true);
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
        for (index, (caption, size)) in ctx.captions.iter().zip(&sizes).enumerate() {
            let edge = if caption.is_below {
                &mut bottom
            } else {
                &mut top
            };
            let mut layout =
                taffy::Layout::with_order(u32::try_from(first_order + index).unwrap_or(u32::MAX));
            layout.location.y = *edge;
            layout.size = *size;
            *edge += size.height;
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
        let cell = &self.ctx.cells[usize::from(node_id)];
        let node_id = crate::taffy_node_id(cell.node_id);
        self.doc.compute_child_layout(node_id, inputs)
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
            share_columns(&[30.0, 50.0], &[60.0, 90.0], 70.0, true),
            [30.0, 50.0]
        );
    }

    #[test]
    fn room_between_the_limits_goes_to_the_columns_that_can_grow() {
        let widths = share_columns(&[40.0, 50.0, 30.0], &[40.0, 450.0, 110.0], 300.0, true);
        assert!(close(&widths, &[40.0, 200.0, 60.0]), "{widths:?}");
    }

    #[test]
    fn spare_room_is_shared_by_widest_content() {
        let widths = share_columns(&[10.0, 20.0], &[50.0, 150.0], 300.0, true);
        assert!(close(&widths, &[75.0, 225.0]), "{widths:?}");
    }

    #[test]
    fn a_table_without_a_width_stops_at_its_content() {
        assert_eq!(
            share_columns(&[10.0, 20.0], &[50.0, 150.0], 300.0, false),
            [50.0, 150.0]
        );
    }
}
