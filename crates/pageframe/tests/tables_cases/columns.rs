use super::*;

#[test]
fn a_collapsed_column_is_removed_after_auto_sizing() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="a" style=""><col style="visibility:collapse"><col><col><tr ><td id="a1"><i></i></td><td id="a2"><i></i></td><td id="a3"><i></i></td></tr></table>"#,
        &[
            ("a", [0.0, 0.0, 26.0, 14.0]),
            ("a1", [1.0, 1.0, 0.0, 12.0]),
            ("a2", [1.0, 1.0, 12.0, 12.0]),
            ("a3", [13.0, 1.0, 12.0, 12.0]),
        ],
    );
}

#[test]
fn a_collapsed_column_is_removed_after_width_distribution() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="b" style="width:122px"><col style="visibility:collapse"><col><col><tr ><td id="b1"><i></i></td><td id="b2"><i></i></td><td id="b3"><i></i></td></tr></table>"#,
        &[
            ("b", [0.0, 0.0, 82.0, 14.0]),
            ("b1", [1.0, 1.0, 0.0, 12.0]),
            ("b2", [1.0, 1.0, 40.0, 12.0]),
            ("b3", [41.0, 1.0, 40.0, 12.0]),
        ],
    );
}

#[test]
fn a_collapsed_column_removes_one_spacing_gap() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="c" style="border-collapse:separate;border-spacing:4px 3px"><col style="visibility:collapse"><col><col><tr ><td id="c1"><i></i></td><td id="c2"><i></i></td><td id="c3"><i></i></td></tr></table>"#,
        &[
            ("c", [0.0, 0.0, 40.0, 20.0]),
            ("c1", [4.0, 3.0, 0.0, 14.0]),
            ("c2", [4.0, 3.0, 14.0, 14.0]),
            ("c3", [22.0, 3.0, 14.0, 14.0]),
        ],
    );
}

#[test]
fn collapsed_column_groups_remove_each_column() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="d" style=""><colgroup style="visibility:collapse"><col><col></colgroup><col><tr ><td id="d1"><i></i></td><td id="d2"><i></i></td><td id="d3"><i></i></td></tr></table>"#,
        &[
            ("d", [0.0, 0.0, 14.0, 14.0]),
            ("d1", [1.0, 1.0, 0.0, 12.0]),
            ("d2", [1.0, 1.0, 0.0, 12.0]),
            ("d3", [1.0, 1.0, 12.0, 12.0]),
        ],
    );
}

#[test]
fn a_colspan_shrinks_across_a_collapsed_column() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="e" style=""><col><col style="visibility:collapse"><col><tr><td id="e1" colspan="2"><i style="width:30px"></i></td><td id="e2"><i></i></td></tr><tr ><td id="e31"><i></i></td><td id="e32"><i></i></td><td id="e33"><i></i></td></tr></table>"#,
        &[
            ("e", [0.0, 0.0, 30.0, 26.0]),
            ("e1", [1.0, 1.0, 16.0, 12.0]),
            ("e2", [17.0, 1.0, 12.0, 12.0]),
            ("e31", [1.0, 13.0, 16.0, 12.0]),
            ("e32", [17.0, 13.0, 0.0, 12.0]),
            ("e33", [17.0, 13.0, 12.0, 12.0]),
        ],
    );
}

#[test]
fn a_colspan_survives_a_collapsed_first_column() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="f" style=""><col style="visibility:collapse"><col><col><tr><td id="f1" colspan="2"><i style="width:30px"></i></td><td id="f2"><i></i></td></tr><tr ><td id="f31"><i></i></td><td id="f32"><i></i></td><td id="f33"><i></i></td></tr></table>"#,
        &[
            ("f", [0.0, 0.0, 30.0, 26.0]),
            ("f1", [1.0, 1.0, 16.0, 12.0]),
            ("f2", [17.0, 1.0, 12.0, 12.0]),
            ("f31", [1.0, 13.0, 0.0, 12.0]),
            ("f32", [1.0, 13.0, 16.0, 12.0]),
            ("f33", [17.0, 13.0, 12.0, 12.0]),
        ],
    );
}

#[test]
fn columns_and_rows_can_collapse_together() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="g" style=""><col style="visibility:collapse"><col><col><tr style="visibility:collapse"><td id="g1"><i></i></td><td id="g2"><i></i></td><td id="g3"><i></i></td></tr><tr ><td id="g31"><i></i></td><td id="g32"><i></i></td><td id="g33"><i></i></td></tr></table>"#,
        &[
            ("g", [0.0, 0.0, 26.0, 14.0]),
            ("g1", [1.0, 1.0, 0.0, 0.0]),
            ("g2", [1.0, 1.0, 12.0, 0.0]),
            ("g3", [13.0, 1.0, 12.0, 0.0]),
            ("g31", [1.0, 1.0, 0.0, 12.0]),
            ("g32", [1.0, 1.0, 12.0, 12.0]),
            ("g33", [13.0, 1.0, 12.0, 12.0]),
        ],
    );
}

#[test]
fn a_collapsed_column_keeps_the_row_height() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="h" style=""><col style="visibility:collapse"><col><col><tr ><td id="h1"><i style="height:40px"></i></td><td id="h2"><i></i></td><td id="h3"><i></i></td></tr></table>"#,
        &[
            ("h", [0.0, 0.0, 26.0, 44.0]),
            ("h1", [1.0, 1.0, 0.0, 42.0]),
            ("h2", [1.0, 1.0, 12.0, 42.0]),
            ("h3", [13.0, 1.0, 12.0, 42.0]),
        ],
    );
}

#[test]
fn captions_keep_their_width_when_columns_collapse() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="i" style=""><caption id="icap"><i style="width:40px"></i></caption><col style="visibility:collapse"><col><col><tr ><td id="i1"><i></i></td><td id="i2"><i></i></td><td id="i3"><i></i></td></tr></table>"#,
        &[
            ("i", [0.0, 0.0, 40.0, 24.0]),
            ("icap", [0.0, 0.0, 40.0, 10.0]),
            ("i1", [1.0, 11.0, 0.0, 12.0]),
            ("i2", [1.0, 11.0, 12.65625, 12.0]),
            ("i3", [13.65625, 11.0, 12.6875, 12.0]),
        ],
    );
}

#[test]
fn percent_columns_are_sized_before_they_collapse() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="j" style="width:122px"><col style="visibility:collapse;width:50%"><col><col><tr ><td id="j1"><i></i></td><td id="j2"><i></i></td><td id="j3"><i></i></td></tr></table>"#,
        &[
            ("j", [0.0, 0.0, 62.0, 14.0]),
            ("j1", [1.0, 1.0, 0.0, 12.0]),
            ("j2", [1.0, 1.0, 30.0, 12.0]),
            ("j3", [31.0, 1.0, 30.0, 12.0]),
        ],
    );
}

#[test]
fn fixed_columns_are_sized_before_they_collapse() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="k" style=""><col style="visibility:collapse;width:30px"><col><col><tr ><td id="k1"><i></i></td><td id="k2"><i></i></td><td id="k3"><i></i></td></tr></table>"#,
        &[
            ("k", [0.0, 0.0, 26.0, 14.0]),
            ("k1", [1.0, 1.0, 0.0, 12.0]),
            ("k2", [1.0, 1.0, 12.0, 12.0]),
            ("k3", [13.0, 1.0, 12.0, 12.0]),
        ],
    );
}

#[test]
fn a_collapsed_cell_keeps_its_size() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="l" style=""><tr><td id="l1" style="visibility:collapse"><i style="height:30px"></i></td><td id="l2"><i></i></td></tr></table>"#,
        &[
            ("l", [0.0, 0.0, 26.0, 34.0]),
            ("l1", [1.0, 1.0, 12.0, 32.0]),
            ("l2", [13.0, 1.0, 12.0, 32.0]),
        ],
    );
}

#[test]
fn a_column_group_span_can_collapse() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="m" style=""><colgroup span="2" style="visibility:collapse"></colgroup><col><tr ><td id="m1"><i></i></td><td id="m2"><i></i></td><td id="m3"><i></i></td></tr></table>"#,
        &[
            ("m", [0.0, 0.0, 14.0, 14.0]),
            ("m1", [1.0, 1.0, 0.0, 12.0]),
            ("m2", [1.0, 1.0, 0.0, 12.0]),
            ("m3", [1.0, 1.0, 12.0, 12.0]),
        ],
    );
}

#[test]
fn a_middle_column_removes_its_spacing_gap() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="n" style="border-collapse:separate;border-spacing:4px 3px;width:122px"><col><col style="visibility:collapse"><col><tr ><td id="n1"><i></i></td><td id="n2"><i></i></td><td id="n3"><i></i></td></tr></table>"#,
        &[
            ("n", [0.0, 0.0, 82.67188, 20.0]),
            ("n1", [4.0, 3.0, 35.32812, 14.0]),
            ("n2", [39.32812, 3.0, 0.0, 14.0]),
            ("n3", [43.32812, 3.0, 35.34375, 14.0]),
        ],
    );
}

#[test]
fn visible_columns_override_a_collapsed_group() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="o" style=""><colgroup style="visibility:collapse"><col style="visibility:visible"><col></colgroup><col><tr ><td id="o1"><i></i></td><td id="o2"><i></i></td><td id="o3"><i></i></td></tr></table>"#,
        &[
            ("o", [0.0, 0.0, 26.0, 14.0]),
            ("o1", [1.0, 1.0, 12.0, 12.0]),
            ("o2", [13.0, 1.0, 0.0, 12.0]),
            ("o3", [13.0, 1.0, 12.0, 12.0]),
        ],
    );
}

#[test]
fn all_columns_can_collapse() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="p" style=""><col span="3" style="visibility:collapse"><tr ><td id="p1"><i></i></td><td id="p2"><i></i></td><td id="p3"><i></i></td></tr></table>"#,
        &[
            ("p", [0.0, 0.0, 2.0, 14.0]),
            ("p1", [1.0, 1.0, 0.0, 12.0]),
            ("p2", [1.0, 1.0, 0.0, 12.0]),
            ("p3", [1.0, 1.0, 0.0, 12.0]),
        ],
    );
}

#[test]
fn separate_border_colspans_shrink_across_collapsed_columns() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="a" style="border-collapse:separate;border-spacing:4px 3px"><col><col style="visibility:collapse"><col><tr><td id="a1" colspan="2"><i style="width:40px;background:blue"></i></td><td id="a2"><i></i></td></tr><tr><td id="a31"><i></i></td><td id="a32"><i style="height:30px"></i></td><td id="a33"><i></i></td></tr></table>"#,
        &[
            ("a", [0.0, 0.0, 46.0, 57.0]),
            ("a1", [4.0, 3.0, 20.0, 14.0]),
            ("a2", [28.0, 3.0, 14.0, 14.0]),
            ("a31", [4.0, 20.0, 20.0, 34.0]),
            ("a32", [24.0, 20.0, 0.0, 34.0]),
            ("a33", [28.0, 20.0, 14.0, 34.0]),
        ],
    );
}

#[test]
fn removed_columns_keep_collapsed_border_widths() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="b" style=""><col><col style="visibility:collapse"><col><tr><td id="b1" style="border-right:6px solid blue"><i></i></td><td id="b2" style="border:8px solid red"><i></i></td><td id="b3" style="border-left:4px solid green"><i></i></td></tr></table>"#,
        &[
            ("b", [0.0, 0.0, 32.0, 26.0]),
            ("b1", [1.0, 4.0, 15.0, 18.0]),
            ("b2", [16.0, 4.0, 0.0, 18.0]),
            ("b3", [16.0, 4.0, 15.0, 18.0]),
        ],
    );
}

#[test]
fn positioned_cells_in_collapsed_columns_have_zero_width() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="c" style=""><col style="visibility:collapse"><col><tr><td id="c1" style="visibility:visible;position:relative"><i></i></td><td id="c2"><i style="background:blue"></i></td></tr></table>"#,
        &[
            ("c", [0.0, 0.0, 14.0, 14.0]),
            ("c1", [1.0, 1.0, 0.0, 12.0]),
            ("c2", [1.0, 1.0, 12.0, 12.0]),
        ],
    );
}

#[test]
fn separate_tables_keep_outer_spacing_when_all_columns_collapse() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="d" style="border-collapse:separate;border-spacing:4px 3px"><col span="2" style="visibility:collapse"><tr><td id="d1"><i></i></td><td id="d2"><i></i></td></tr></table>"#,
        &[
            ("d", [0.0, 0.0, 8.0, 20.0]),
            ("d1", [4.0, 3.0, 0.0, 14.0]),
            ("d2", [4.0, 3.0, 0.0, 14.0]),
        ],
    );
}

#[test]
fn a_collapsed_cell_keeps_visible_child_geometry() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="e" style=""><tr><td id="e1" style="visibility:collapse"><i id="ei" style="visibility:visible"></i></td><td id="e2"><i style="background:blue"></i></td></tr></table>"#,
        &[
            ("e", [0.0, 0.0, 26.0, 14.0]),
            ("e1", [1.0, 1.0, 12.0, 12.0]),
            ("ei", [2.0, 2.0, 10.0, 10.0]),
            ("e2", [13.0, 1.0, 12.0, 12.0]),
        ],
    );
}

#[test]
fn fixed_layout_distributes_width_before_columns_collapse() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="f" style="width:122px;table-layout:fixed"><colgroup style="visibility:collapse" span="2"></colgroup><col><tr><td id="f1"><i></i></td><td id="f2"><i style="height:30px"></i></td><td id="f3"><i></i></td></tr></table>"#,
        &[
            ("f", [0.0, 0.0, 42.0, 34.0]),
            ("f1", [1.0, 1.0, 0.0, 32.0]),
            ("f2", [1.0, 1.0, 0.0, 32.0]),
            ("f3", [1.0, 1.0, 40.0, 32.0]),
        ],
    );
}

#[test]
fn fixed_column_widths_grow_before_collapsing() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="g" style="width:122px"><col style="visibility:collapse;width:30px"><col><col><tr><td id="g1"><i></i></td><td id="g2"><i style="height:30px"></i></td><td id="g3"><i></i></td></tr></table>"#,
        &[
            ("g", [0.0, 0.0, 55.34375, 34.0]),
            ("g1", [1.0, 1.0, 0.0, 32.0]),
            ("g2", [1.0, 1.0, 26.65625, 32.0]),
            ("g3", [27.65625, 1.0, 26.6875, 32.0]),
        ],
    );
}

#[test]
fn a_colspan_can_cross_a_collapsed_middle_column() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="h" style=""><col><col style="visibility:collapse"><col><tr><td id="h1" colspan="3"><i style="width:50px;background:blue"></i></td></tr><tr><td id="h31"><i></i></td><td id="h32"><i style="height:30px"></i></td><td id="h33"><i></i></td></tr></table>"#,
        &[
            ("h", [0.0, 0.0, 36.67188, 46.0]),
            ("h1", [1.0, 1.0, 34.67188, 12.0]),
            ("h31", [1.0, 13.0, 17.32812, 32.0]),
            ("h32", [18.32812, 13.0, 0.0, 32.0]),
            ("h33", [18.32812, 13.0, 17.34375, 32.0]),
        ],
    );
}

#[test]
fn a_last_column_removes_its_spacing_gap() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="i" style="border-collapse:separate;border-spacing:4px 3px"><col><col><col style="visibility:collapse"><tr><td id="i1"><i></i></td><td id="i2"><i style="height:30px"></i></td><td id="i3"><i></i></td></tr></table>"#,
        &[
            ("i", [0.0, 0.0, 40.0, 40.0]),
            ("i1", [4.0, 3.0, 14.0, 34.0]),
            ("i2", [22.0, 3.0, 14.0, 34.0]),
            ("i3", [36.0, 3.0, 0.0, 34.0]),
        ],
    );
}

#[test]
fn auto_tables_resolve_percent_columns_before_collapsing() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="j" style=""><col style="visibility:collapse;width:25%"><col><col><tr><td id="j1"><i></i></td><td id="j2"><i style="height:30px"></i></td><td id="j3"><i></i></td></tr></table>"#,
        &[
            ("j", [0.0, 0.0, 38.0, 34.0]),
            ("j1", [1.0, 1.0, 0.0, 32.0]),
            ("j2", [1.0, 1.0, 18.0, 32.0]),
            ("j3", [19.0, 1.0, 18.0, 32.0]),
        ],
    );
}

#[test]
fn explicit_table_width_is_shared_before_middle_columns_collapse() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="k" style="width:122px"><col><col style="visibility:collapse"><col><tr><td id="k1"><i></i></td><td id="k2"><i style="height:30px"></i></td><td id="k3"><i></i></td></tr></table>"#,
        &[
            ("k", [0.0, 0.0, 82.0, 34.0]),
            ("k1", [1.0, 1.0, 40.0, 32.0]),
            ("k2", [41.0, 1.0, 0.0, 32.0]),
            ("k3", [41.0, 1.0, 40.0, 32.0]),
        ],
    );
}

#[test]
fn right_to_left_columns_collapse_towards_the_right() {
    check_table_boxes(
        r#"<style>i{display:block;width:10px}</style><table id="l" style="direction:rtl"><col><col style="visibility:collapse"><col><tr><td id="l1"><i></i></td><td id="l2"><i style="height:30px"></i></td><td id="l3"><i></i></td></tr></table>"#,
        &[
            ("l", [0.0, 0.0, 26.0, 34.0]),
            ("l1", [13.0, 1.0, 12.0, 32.0]),
            ("l2", [13.0, 1.0, 0.0, 32.0]),
            ("l3", [1.0, 1.0, 12.0, 32.0]),
        ],
    );
}

#[test]
fn positioned_cells_in_collapsed_columns_do_not_paint() {
    let buffer = draw(
        r#"<style>td{padding:0;width:10px}i{display:block;width:10px;background:red}</style><table id="c" style=""><col style="visibility:collapse"><col><tr><td id="c1" style="visibility:visible;position:relative"><i></i></td><td id="c2"><i style="background:blue"></i></td></tr></table>"#,
    );
    assert_eq!(pixel(&buffer, 5, 5), BLUE);
    assert_eq!(pixel(&buffer, 15, 5), WHITE);
}

#[test]
fn visible_children_of_a_collapsed_cell_can_paint() {
    let buffer = draw(
        r#"<style>td{padding:0;width:10px}i{display:block;width:10px;background:red}</style><table id="e" style=""><tr><td id="e1" style="visibility:collapse"><i id="ei" style="visibility:visible"></i></td><td id="e2"><i style="background:blue"></i></td></tr></table>"#,
    );
    assert_eq!(pixel(&buffer, 5, 5), RED);
}

#[test]
fn a_removed_column_can_win_a_shared_border() {
    let buffer = draw(
        r#"<style>td{padding:0;width:10px}i{display:block;width:10px;background:red}</style><table id="b" style=""><col><col style="visibility:collapse"><col><tr><td id="b1" style="border-right:6px solid blue"><i></i></td><td id="b2" style="border:8px solid red"><i></i></td><td id="b3" style="border-left:4px solid green"><i></i></td></tr></table>"#,
    );
    assert_eq!(pixel(&buffer, 13, 3), RED);
    assert_eq!(pixel(&buffer, 15, 3), RED);
}

#[test]
fn collapsed_column_borders_paint_over_overflowing_spans() {
    let buffer = draw(
        r#"<style>td{padding:0;width:10px}i{display:block;width:10px;background:red}</style><table id="h" style=""><col><col style="visibility:collapse"><col><tr><td id="h1" colspan="3"><i style="width:50px;background:blue"></i></td></tr><tr><td id="h31"><i></i></td><td id="h32"><i style="height:30px"></i></td><td id="h33"><i></i></td></tr></table>"#,
    );
    assert_eq!(pixel(&buffer, 35, 3), BLACK);
    assert_eq!(pixel(&buffer, 40, 3), BLUE);
}

#[test]
fn positioned_cells_in_collapsed_rows_paint_borders_above_the_background() {
    for position in [
        "position:relative",
        "position:relative;z-index:1",
        "transform:translateX(0px)",
    ] {
        let buffer = draw(&format!(
            "<table class=\"bare\"><tr style=\"visibility:collapse\"><td style=\"width:20px;visibility:visible;{position}\"><i style=\"display:block;width:10px;height:10px;background:red\"></i></td></tr></table>"
        ));
        assert_eq!(pixel(&buffer, 5, 1), BLACK);
        assert_eq!(pixel(&buffer, 5, 5), RED);
    }
}
