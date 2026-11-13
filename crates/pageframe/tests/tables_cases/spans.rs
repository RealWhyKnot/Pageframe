use super::*;

#[test]
fn captions_center_blocks_after_the_table_loses_a_column() {
    for side in ["top", "bottom"] {
        let width = if side == "top" { "" } else { "width:122px" };
        let buffer = draw(&format!(
            "<style>caption{{background:white;caption-side:{side}}}td{{padding:0;width:10px}}i{{display:block;width:10px}}</style><table style=\"{width}\"><caption><i style=\"width:6px;background:blue\"></i></caption><col style=\"visibility:collapse\"><col><col><tr><td><i></i></td><td><i></i></td><td><i></i></td></tr></table>"
        ));
        let y = if side == "top" { 5 } else { 19 };
        assert_eq!(pixel(&buffer, 0, y), WHITE);
        let x = if side == "top" { 10 } else { 38 };
        assert_eq!(pixel(&buffer, x, y), BLUE);
        assert_eq!(pixel(&buffer, x + 6, y), WHITE);
    }
}

#[test]
fn whitespace_does_not_override_a_column_group_span() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="a" style=""><colgroup class="x" span="2">
</colgroup><tr><td id="a1"><b></b></td><td id="a2"><b></b></td><td id="a3"><b></b></td></tr></table></div>"#,
        &[
            ("a", [0.0, 0.0, 14.0, 18.0]),
            ("a1", [2.0, 2.0, 0.0, 14.0]),
            ("a2", [2.0, 2.0, 0.0, 14.0]),
            ("a3", [2.0, 2.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn comments_do_not_override_a_column_group_span() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="b" style=""><colgroup class="x" span="2"><!-- only a comment --></colgroup><tr><td id="b1"><b></b></td><td id="b2"><b></b></td><td id="b3"><b></b></td></tr></table></div>"#,
        &[
            ("b", [0.0, 0.0, 14.0, 18.0]),
            ("b1", [2.0, 2.0, 0.0, 14.0]),
            ("b2", [2.0, 2.0, 0.0, 14.0]),
            ("b3", [2.0, 2.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn column_children_override_a_larger_group_span() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="c" style=""><colgroup class="x" span="3">
<col>
</colgroup><tr><td id="c1"><b></b></td><td id="c2"><b></b></td><td id="c3"><b></b></td></tr></table></div>"#,
        &[
            ("c", [0.0, 0.0, 26.0, 18.0]),
            ("c1", [2.0, 2.0, 0.0, 14.0]),
            ("c2", [2.0, 2.0, 10.0, 14.0]),
            ("c3", [14.0, 2.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn column_children_override_a_smaller_group_span() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="d" style=""><colgroup class="x" span="1"><col><col></colgroup><tr><td id="d1"><b></b></td><td id="d2"><b></b></td><td id="d3"><b></b></td></tr></table></div>"#,
        &[
            ("d", [0.0, 0.0, 14.0, 18.0]),
            ("d1", [2.0, 2.0, 0.0, 14.0]),
            ("d2", [2.0, 2.0, 0.0, 14.0]),
            ("d3", [2.0, 2.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn unused_span_columns_do_not_add_spacing() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="e" style=""><tr><td id="e1" colspan="5"><b style="width:60px"></b></td></tr><tr><td id="e21"><b></b></td><td id="e22"><b></b></td><td id="e23"><b></b></td></tr></table></div>"#,
        &[
            ("e", [0.0, 0.0, 68.0, 34.0]),
            ("e1", [2.0, 2.0, 64.0, 14.0]),
            ("e21", [2.0, 18.0, 20.0, 14.0]),
            ("e22", [24.0, 18.0, 20.0, 14.0]),
            ("e23", [46.0, 18.0, 20.0, 14.0]),
        ],
    );
}

#[test]
fn unused_span_columns_do_not_change_column_collapse() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="f" style=""><col><col class="x"><tr><td id="f1" colspan="5"><b style="width:60px"></b></td></tr><tr><td id="f21"><b></b></td><td id="f22"><b></b></td><td id="f23"><b></b></td></tr></table></div>"#,
        &[
            ("f", [0.0, 0.0, 46.0, 34.0]),
            ("f1", [2.0, 2.0, 42.0, 14.0]),
            ("f21", [2.0, 18.0, 20.0, 14.0]),
            ("f22", [22.0, 18.0, 0.0, 14.0]),
            ("f23", [24.0, 18.0, 20.0, 14.0]),
        ],
    );
}

#[test]
fn auto_columns_do_not_keep_unused_span_tracks() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="g" style=""><col span="5"><tr><td id="g1" colspan="5"><b style="width:60px"></b></td></tr><tr><td id="g21"><b></b></td><td id="g22"><b></b></td><td id="g23"><b></b></td></tr></table></div>"#,
        &[
            ("g", [0.0, 0.0, 68.0, 34.0]),
            ("g1", [2.0, 2.0, 64.0, 14.0]),
            ("g21", [2.0, 18.0, 20.0, 14.0]),
            ("g22", [24.0, 18.0, 20.0, 14.0]),
            ("g23", [46.0, 18.0, 20.0, 14.0]),
        ],
    );
}

#[test]
fn a_large_span_ignores_unused_named_auto_columns() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="h" style=""><col span="5"><tr><td id="h1" colspan="70000"><b></b></td></tr><tr><td id="h21"><b></b></td><td id="h22"><b></b></td><td id="h23"><b></b></td></tr></table></div>"#,
        &[
            ("h", [0.0, 0.0, 38.0, 34.0]),
            ("h1", [2.0, 2.0, 34.0, 14.0]),
            ("h21", [2.0, 18.0, 10.0, 14.0]),
            ("h22", [14.0, 18.0, 10.0, 14.0]),
            ("h23", [26.0, 18.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn unused_tracks_between_cell_starts_do_not_add_spacing() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="i" style=""><tr><td id="i1" colspan="5"><b style="width:60px"></b></td><td id="i2"><b></b></td></tr></table></div>"#,
        &[
            ("i", [0.0, 0.0, 80.0, 18.0]),
            ("i1", [2.0, 2.0, 64.0, 14.0]),
            ("i2", [68.0, 2.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn a_single_large_span_uses_one_column() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="j" style=""><tr><td id="j1" colspan="1000"><b></b></td></tr></table></div>"#,
        &[
            ("j", [0.0, 0.0, 14.0, 18.0]),
            ("j1", [2.0, 2.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn overlapping_spans_share_only_occupied_columns() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="k" style=""><tr><td id="k1" colspan="5"><b style="width:60px"></b></td></tr><tr><td id="k2" colspan="2"><b></b></td><td id="k3" colspan="3"><b></b></td></tr></table></div>"#,
        &[
            ("k", [0.0, 0.0, 68.0, 34.0]),
            ("k1", [2.0, 2.0, 64.0, 14.0]),
            ("k2", [2.0, 18.0, 31.0, 14.0]),
            ("k3", [35.0, 18.0, 31.0, 14.0]),
        ],
    );
}

#[test]
fn rowspans_do_not_keep_unused_interior_columns() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="l" style=""><tr><td id="l1" colspan="3" rowspan="2"><b style="width:30px"></b></td><td id="l2"><b></b></td></tr><tr><td id="l3"><b></b></td></tr></table></div>"#,
        &[
            ("l", [0.0, 0.0, 50.0, 34.0]),
            ("l1", [2.0, 2.0, 34.0, 30.0]),
            ("l2", [38.0, 2.0, 10.0, 14.0]),
            ("l3", [38.0, 18.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn auto_columns_do_not_keep_empty_rowspan_tracks() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="m" style=""><col span="3"><tr><td id="m1" colspan="3" rowspan="2"><b style="width:30px"></b></td><td id="m2"><b></b></td></tr><tr><td id="m3"><b></b></td></tr></table></div>"#,
        &[
            ("m", [0.0, 0.0, 50.0, 34.0]),
            ("m1", [2.0, 2.0, 34.0, 30.0]),
            ("m2", [38.0, 2.0, 10.0, 14.0]),
            ("m3", [38.0, 18.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn a_large_span_after_a_cell_stops_at_occupied_columns() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="n" style=""><tr><td id="n1"><b></b></td><td id="n2" colspan="65535"><b></b></td></tr><tr><td id="n31"><b></b></td><td id="n32"><b></b></td><td id="n33"><b></b></td></tr></table></div>"#,
        &[
            ("n", [0.0, 0.0, 38.0, 34.0]),
            ("n1", [2.0, 2.0, 10.0, 14.0]),
            ("n2", [14.0, 2.0, 22.0, 14.0]),
            ("n31", [2.0, 18.0, 10.0, 14.0]),
            ("n32", [14.0, 18.0, 10.0, 14.0]),
            ("n33", [26.0, 18.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn trailing_auto_columns_do_not_add_spacing() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="o" style=""><col span="5"><tr><td id="o1"><b></b></td><td id="o2"><b></b></td></tr></table></div>"#,
        &[
            ("o", [0.0, 0.0, 26.0, 18.0]),
            ("o1", [2.0, 2.0, 10.0, 14.0]),
            ("o2", [14.0, 2.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn trailing_columns_with_widths_keep_their_spacing() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="p" style=""><col span="5" style="width:10px"><tr><td id="p1"><b></b></td><td id="p2"><b></b></td></tr></table></div>"#,
        &[
            ("p", [0.0, 0.0, 62.0, 18.0]),
            ("p1", [2.0, 2.0, 10.0, 14.0]),
            ("p2", [14.0, 2.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn percentage_tables_keep_visible_columns_at_their_minimum() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="a" style="width:50%"><col class="x" style="width:40%"><tr><td id="a1"><b></b></td><td id="a2"><b></b></td></tr></table></div>"#,
        &[
            ("a", [0.0, 0.0, 14.0, 18.0]),
            ("a1", [2.0, 2.0, 0.0, 14.0]),
            ("a2", [2.0, 2.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn auto_tables_resolve_a_collapsed_column_percentage() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="b" style=""><col class="x" style="width:40%"><tr><td id="b1"><b></b></td><td id="b2"><b></b></td></tr></table></div>"#,
        &[
            ("b", [0.0, 0.0, 19.0, 18.0]),
            ("b1", [2.0, 2.0, 0.0, 14.0]),
            ("b2", [2.0, 2.0, 15.0, 14.0]),
        ],
    );
}

#[test]
fn pixel_width_tables_resolve_a_collapsed_column_percentage() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="c" style="width:100px"><col class="x" style="width:40%"><tr><td id="c1"><b></b></td><td id="c2"><b></b></td></tr></table></div>"#,
        &[
            ("c", [0.0, 0.0, 60.40625, 18.0]),
            ("c1", [2.0, 2.0, 0.0, 14.0]),
            ("c2", [2.0, 2.0, 56.40625, 14.0]),
        ],
    );
}

#[test]
fn visible_percent_columns_beside_collapsed_columns_keep_their_minimum() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="d" style=""><col class="x"><col style="width:40%"><tr><td id="d1"><b></b></td><td id="d2"><b></b></td></tr></table></div>"#,
        &[
            ("d", [0.0, 0.0, 14.0, 18.0]),
            ("d1", [2.0, 2.0, 0.0, 14.0]),
            ("d2", [2.0, 2.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn pixel_width_tables_resolve_visible_column_percentages() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="e" style="width:100px"><col class="x"><col style="width:40%"><tr><td id="e1"><b></b></td><td id="e2"><b></b></td></tr></table></div>"#,
        &[
            ("e", [0.0, 0.0, 41.59375, 18.0]),
            ("e1", [2.0, 2.0, 0.0, 14.0]),
            ("e2", [2.0, 2.0, 37.59375, 14.0]),
        ],
    );
}

#[test]
fn percentage_tables_resolve_visible_column_percentages() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="f" style="width:50%"><col class="x"><col style="width:40%"><tr><td id="f1"><b></b></td><td id="f2"><b></b></td></tr></table></div>"#,
        &[
            ("f", [0.0, 0.0, 14.0, 18.0]),
            ("f1", [2.0, 2.0, 0.0, 14.0]),
            ("f2", [2.0, 2.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn full_width_tables_keep_their_percent_column_space() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="g" style="width:100%"><col class="x" style="width:40%"><tr><td id="g1"><b></b></td><td id="g2"><b></b></td></tr></table></div>"#,
        &[
            ("g", [0.0, 0.0, 19.0, 18.0]),
            ("g1", [2.0, 2.0, 0.0, 14.0]),
            ("g2", [2.0, 2.0, 15.0, 14.0]),
        ],
    );
}

#[test]
fn small_percentage_tables_keep_visible_column_content() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="h" style="width:25%"><col class="x" style="width:80%"><tr><td id="h1"><b></b></td><td id="h2"><b></b></td></tr></table></div>"#,
        &[
            ("h", [0.0, 0.0, 14.0, 18.0]),
            ("h1", [2.0, 2.0, 0.0, 14.0]),
            ("h2", [2.0, 2.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn spanning_content_can_grow_columns_with_widths() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="a" style=""><col span="5" style="width:10px"><tr><td id="a1" colspan="5"><b style="width:60px"></b></td></tr><tr><td id="a2"><b></b></td><td id="a3"><b></b></td><td id="a4"><b></b></td></tr></table></div>"#,
        &[
            ("a", [0.0, 0.0, 68.0, 34.0]),
            ("a1", [2.0, 2.0, 64.0, 14.0]),
            ("a2", [2.0, 18.0, 11.1875, 14.0]),
            ("a3", [15.1875, 18.0, 11.1875, 14.0]),
            ("a4", [28.375, 18.0, 11.1875, 14.0]),
        ],
    );
}

#[test]
fn percent_columns_keep_otherwise_unused_tracks() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="b" style="width:100px"><col><col style="width:50%"><col><tr><td id="b1" colspan="3"><b style="width:60px"></b></td></tr></table></div>"#,
        &[
            ("b", [0.0, 0.0, 100.0, 18.0]),
            ("b1", [2.0, 2.0, 96.0, 14.0]),
        ],
    );
}

#[test]
fn wide_tables_distribute_space_between_occupied_columns() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="c" style="width:100px"><col span="5"><tr><td id="c1" colspan="5"><b style="width:60px"></b></td></tr><tr><td id="c2"><b></b></td><td id="c3"><b></b></td><td id="c4"><b></b></td></tr></table></div>"#,
        &[
            ("c", [0.0, 0.0, 100.0, 34.0]),
            ("c1", [2.0, 2.0, 96.0, 14.0]),
            ("c2", [2.0, 18.0, 30.65625, 14.0]),
            ("c3", [34.65625, 18.0, 30.65625, 14.0]),
            ("c4", [67.3125, 18.0, 30.6875, 14.0]),
        ],
    );
}

#[test]
fn zero_width_columns_do_not_keep_unused_tracks() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="d" style=""><col><col style="width:0px"><col><tr><td id="d1" colspan="3"><b style="width:60px"></b></td></tr></table></div>"#,
        &[
            ("d", [0.0, 0.0, 68.0, 18.0]),
            ("d1", [2.0, 2.0, 64.0, 14.0]),
        ],
    );
}

#[test]
fn auto_columns_under_a_single_span_do_not_add_spacing() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="e" style=""><col span="5"><tr><td id="e1" colspan="5"><b style="width:60px"></b></td></tr></table></div>"#,
        &[
            ("e", [0.0, 0.0, 68.0, 18.0]),
            ("e1", [2.0, 2.0, 64.0, 14.0]),
        ],
    );
}

#[test]
fn widths_on_columns_under_a_single_span_keep_spacing() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x,colgroup.x{visibility:collapse}</style><div style="position:absolute;left:0;top:0"><table id="f" style=""><col span="5" style="width:10px"><tr><td id="f1" colspan="5"><b></b></td></tr></table></div>"#,
        &[
            ("f", [0.0, 0.0, 62.0, 18.0]),
            ("f1", [2.0, 2.0, 58.0, 14.0]),
        ],
    );
}

#[test]
fn a_clamped_large_span_can_cross_a_collapsed_column() {
    check_table_boxes(
        r#"<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}</style><table id="b"><col><col style="visibility:collapse"><tr><td id="b1" colspan="70000"><i></i></td></tr><tr><td id="b2"><i></i></td><td id="b3"><i></i></td><td id="b4"><i></i></td></tr></table>"#,
        &[
            ("b", [0.0, 0.0, 26.0, 34.0]),
            ("b1", [2.0, 2.0, 22.0, 14.0]),
            ("b2", [2.0, 18.0, 10.0, 14.0]),
            ("b3", [12.0, 18.0, 0.0, 14.0]),
            ("b4", [14.0, 18.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn max_width_applies_before_a_collapsed_percent_column_is_removed() {
    check_table_boxes(
        &format!(
            r#"{SPAN_STYLE}<div style="width:200px"><table id="a" style="width:50%;max-width:60px"><col class="x" style="width:40%"><tr><td id="a1"><b></b></td><td id="a2"><b></b></td></tr></table></div>"#
        ),
        &[
            ("a", [0.0, 0.0, 36.40625, 18.0]),
            ("a1", [2.0, 2.0, 0.0, 14.0]),
            ("a2", [2.0, 2.0, 32.40625, 14.0]),
        ],
    );
}

#[test]
fn a_right_to_left_table_drops_its_collapsed_percent_column() {
    check_table_boxes(
        &format!(
            r#"{SPAN_STYLE}<div><table id="c" dir="rtl" style="width:50%"><col class="x" style="width:40%"><tr><td id="c1"><b></b></td><td id="c2"><b></b></td></tr></table></div>"#
        ),
        &[
            ("c", [0.0, 0.0, 14.0, 18.0]),
            ("c1", [12.0, 2.0, 0.0, 14.0]),
            ("c2", [2.0, 2.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn a_wide_spanning_cell_grows_a_table_past_its_width() {
    check_table_boxes(
        &format!(
            r#"{SPAN_STYLE}<div><table id="d" style="width:100px"><tr><td id="d1" colspan="2"><b style="width:150px"></b></td></tr><tr><td id="d2" style="width:20px"><b></b></td><td id="d3" style="width:40px"><b></b></td></tr></table></div>"#
        ),
        &[
            ("d", [0.0, 0.0, 158.0, 34.0]),
            ("d1", [2.0, 2.0, 154.0, 14.0]),
            ("d2", [2.0, 18.0, 53.640625, 14.0]),
            ("d3", [57.640625, 18.0, 98.359375, 14.0]),
        ],
    );
}

#[test]
fn a_wide_spanning_cell_shares_its_width_by_column_width() {
    check_table_boxes(
        &format!(
            r#"{SPAN_STYLE}<div><table id="e"><tr><td id="e1" colspan="2"><b style="width:150px"></b></td></tr><tr><td id="e2" style="width:20px"><b></b></td><td id="e3" style="width:40px"><b></b></td></tr></table></div>"#
        ),
        &[
            ("e", [0.0, 0.0, 158.0, 34.0]),
            ("e1", [2.0, 2.0, 154.0, 14.0]),
            ("e2", [2.0, 18.0, 53.640625, 14.0]),
            ("e3", [57.640625, 18.0, 98.359375, 14.0]),
        ],
    );
}

#[test]
fn many_wide_spans_in_one_row_each_get_a_column() {
    let cells = r#"<td colspan="1000"><b></b></td>"#.repeat(69);
    check_table_boxes(
        &format!(
            r#"{SPAN_STYLE}<table id="a"><tr><td id="a1" colspan="1000"><b></b></td>{cells}</tr><tr><td id="a2"><b></b></td></tr></table>"#
        ),
        &[
            ("a", [0.0, 0.0, 842.0, 34.0]),
            ("a1", [2.0, 2.0, 10.0, 14.0]),
            ("a2", [2.0, 18.0, 10.0, 14.0]),
        ],
    );
}
