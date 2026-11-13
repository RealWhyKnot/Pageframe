use super::*;

#[test]
fn a_nested_table_fits_its_cells_outer_width() {
    check_table_boxes(
        "<style>table{height:122px}</style><table id=\"c\">\
         <tr><td id=\"c1\"><i style=\"height:10px\"></i></td></tr>\
         <tr><td id=\"c2\"><table id=\"ci\" style=\"height:80px\">\
         <tr><td id=\"c3\"><i style=\"height:10px\"></i></td></tr>\
         <tr><td id=\"c4\"><i style=\"height:30px\"></i></td></tr></table></td></tr></table>",
        &[
            ("c", [0.0, 0.0, 14.0, 122.0]),
            ("c1", [1.0, 1.0, 12.0, 15.3125]),
            ("c2", [1.0, 16.3125, 12.0, 104.6875]),
            ("ci", [2.0, 28.65625, 10.0, 80.0]),
            ("c3", [3.0, 29.65625, 8.0, 21.265625]),
            ("c4", [3.0, 50.921875, 8.0, 56.734375]),
        ],
    );
}

#[test]
fn a_nested_table_without_cell_widths_fits_its_parent() {
    check_table_boxes(
        "<table id=\"b\" style=\"\">\
         <tr ><td id=\"b1\"><table id=\"bi\" style=\"\">\
         <tr ><td id=\"b2\" style=\"width:auto\"><i style=\"height:10px\"></i></td></tr></table></td></tr></table>",
        &[
            ("b", [0.0, 0.0, 14.0, 18.0]),
            ("b1", [1.0, 1.0, 12.0, 16.0]),
            ("bi", [2.0, 2.0, 10.0, 14.0]),
            ("b2", [3.0, 3.0, 8.0, 12.0]),
        ],
    );
}

#[test]
fn a_nested_table_with_separate_borders_can_shrink() {
    check_table_boxes(
        "<table id=\"c\" style=\"\">\
         <tr ><td id=\"c1\"><table id=\"ci\" style=\"border-collapse:separate;border-spacing:0\">\
         <tr ><td id=\"c2\" style=\"\"><i style=\"height:10px\"></i></td></tr></table></td></tr></table>",
        &[
            ("c", [0.0, 0.0, 14.0, 18.0]),
            ("c1", [1.0, 1.0, 12.0, 16.0]),
            ("ci", [2.0, 2.0, 10.0, 14.0]),
            ("c2", [2.0, 2.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn a_nested_table_with_two_columns_can_shrink() {
    check_table_boxes(
        "<table id=\"d\" style=\"\">\
         <tr ><td id=\"d1\"><table id=\"di\" style=\"\">\
         <tr ><td id=\"d2\" style=\"\"><i style=\"height:10px\"></i></td><td id=\"d3\" style=\"\"><i style=\"height:10px\"></i></td></tr></table></td></tr></table>",
        &[
            ("d", [0.0, 0.0, 22.0, 18.0]),
            ("d1", [1.0, 1.0, 20.0, 16.0]),
            ("di", [2.0, 2.0, 18.0, 14.0]),
            ("d2", [3.0, 3.0, 8.0, 12.0]),
            ("d3", [11.0, 3.0, 8.0, 12.0]),
        ],
    );
}

#[test]
fn a_nested_percent_width_table_fits_its_parent() {
    check_table_boxes(
        "<table id=\"e\" style=\"\">\
         <tr ><td id=\"e1\"><table id=\"ei\" style=\"width:100%\">\
         <tr ><td id=\"e2\" style=\"\"><i style=\"height:10px\"></i></td></tr></table></td></tr></table>",
        &[
            ("e", [0.0, 0.0, 14.0, 18.0]),
            ("e1", [1.0, 1.0, 12.0, 16.0]),
            ("ei", [2.0, 2.0, 10.0, 14.0]),
            ("e2", [3.0, 3.0, 8.0, 12.0]),
        ],
    );
}

#[test]
fn a_nested_table_keeps_its_border_spacing_when_shrinking() {
    check_table_boxes(
        "<table id=\"g\" style=\"\">\
         <tr ><td id=\"g1\"><table id=\"gi\" style=\"border-collapse:separate;border-spacing:2px\">\
         <tr ><td id=\"g2\" style=\"\"><i style=\"height:10px\"></i></td><td id=\"g3\" style=\"\"><i style=\"height:10px\"></i></td></tr></table></td></tr></table>",
        &[
            ("g", [0.0, 0.0, 30.0, 22.0]),
            ("g1", [1.0, 1.0, 28.0, 20.0]),
            ("gi", [2.0, 2.0, 26.0, 18.0]),
            ("g2", [4.0, 4.0, 10.0, 14.0]),
            ("g3", [16.0, 4.0, 10.0, 14.0]),
        ],
    );
}

#[test]
fn a_nested_cell_width_is_a_preference() {
    check_table_boxes(
        "<table id=\"k\" style=\"\">\
         <tr ><td id=\"k1\"><table id=\"ki\" style=\"\">\
         <tr ><td id=\"k2\" style=\"width:100px\"><i style=\"height:10px\"></i></td></tr></table></td></tr></table>",
        &[
            ("k", [0.0, 0.0, 14.0, 18.0]),
            ("k1", [1.0, 1.0, 12.0, 16.0]),
            ("ki", [2.0, 2.0, 10.0, 14.0]),
            ("k2", [3.0, 3.0, 8.0, 12.0]),
        ],
    );
}

#[test]
fn a_nested_table_keeps_its_own_specified_width() {
    check_table_boxes(
        "<table id=\"l\" style=\"\">\
         <tr ><td id=\"l1\"><table id=\"li\" style=\"width:20px\">\
         <tr ><td id=\"l2\" style=\"\"><i style=\"height:10px\"></i></td></tr></table></td></tr></table>",
        &[
            ("l", [0.0, 0.0, 24.0, 18.0]),
            ("l1", [1.0, 1.0, 22.0, 16.0]),
            ("li", [2.0, 2.0, 20.0, 14.0]),
            ("l2", [3.0, 3.0, 18.0, 12.0]),
        ],
    );
}

#[test]
fn a_table_can_shrink_inside_a_narrow_block() {
    check_table_boxes(
        "<style>td{width:100px}</style><div id=\"a\" style=\"width:10px\"><table id=\"ai\">\
         <tr><td id=\"a2\"><i></i></td></tr></table></div>",
        &[
            ("a", [0.0, 0.0, 10.0, 14.0]),
            ("ai", [0.0, 0.0, 10.0, 14.0]),
            ("a2", [1.0, 1.0, 8.0, 12.0]),
        ],
    );
}

#[test]
fn a_collapsed_row_does_not_paint_visible_children() {
    let buffer = draw(
        "<table class=\"bare\"><tr><td style=\"width:10px\"><i style=\"height:10px\"></i></td></tr>\
         <tr style=\"visibility:collapse;background:red\">\
         <td style=\"visibility:visible;background:red\"><i style=\"height:20px;background:red\"></i></td></tr>\
         <tr><td><i style=\"height:30px\"></i></td></tr></table>",
    );
    assert_eq!(
        column(&buffer, 5, &[2, 11, 12, 13, 14, 43, 44, 45, 46]),
        [
            GREEN, GREEN, BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, WHITE
        ]
    );
}

#[test]
fn a_collapsed_row_keeps_the_winning_border_at_the_join() {
    let buffer = draw(
        "<table class=\"bare\"><tr><td style=\"width:10px\"><i style=\"height:10px\"></i></td></tr>\
         <tr style=\"visibility:collapse\">\
         <td style=\"width:50px;background:red;border:6px solid blue\"><i style=\"height:20px\"></i></td></tr>\
         <tr><td><i style=\"height:30px\"></i></td></tr></table>",
    );
    assert_eq!(
        column(&buffer, 15, &[11, 12, 13, 14, 15, 16, 17, 18]),
        [GREEN, BLUE, BLUE, BLUE, BLUE, BLUE, BLUE, GREEN]
    );
}

#[test]
fn visible_rowspan_content_can_overflow_after_a_row_collapses() {
    let buffer = draw(
        "<table class=\"bare\"><tr><td rowspan=\"3\" style=\"width:10px\">\
         <i style=\"width:6px;height:50px;vertical-align:top;background:#ff0\"></i></td>\
         <td style=\"width:10px\"><i style=\"height:10px\"></i></td></tr>\
         <tr style=\"visibility:collapse\"><td><i style=\"height:20px\"></i></td></tr>\
         <tr><td><i style=\"height:30px\"></i></td></tr></table>",
    );
    assert_eq!(
        column(&buffer, 4, &[2, 43, 44, 45, 50, 51, 52]),
        [
            [255, 255, 0, 255],
            [255, 255, 0, 255],
            [255, 255, 0, 255],
            [255, 255, 0, 255],
            [255, 255, 0, 255],
            [255, 255, 0, 255],
            WHITE,
        ]
    );
}

#[test]
fn nested_cell_borders_fit_inside_the_outer_cell() {
    let buffer = draw(
        "<table class=\"bare\"><tr><td style=\"width:10px\"><table><tr>\
         <td style=\"width:10px\"><i style=\"width:6px;vertical-align:top\"></i></td>\
         </tr></table></td></tr></table>",
    );
    assert_eq!(
        row(&buffer, &[0, 1, 2, 3, 4, 9, 10, 11, 12, 13, 14], 5),
        [
            BLACK, BLACK, BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, BLACK, BLACK, WHITE
        ]
    );
}

#[test]
fn a_header_rowspan_paints_its_border_before_the_body() {
    let buffer = draw(
        "<table class=\"bare\"><thead><tr><td rowspan=\"2\" style=\"width:10px\">\
         <i style=\"height:10px\"></i></td><td style=\"width:10px\"><i></i></td></tr></thead>\
         <tbody><tr><td><i></i></td><td><i style=\"height:30px\"></i></td></tr></tbody></table>",
    );
    assert_eq!(
        column(&buffer, 5, &[11, 12, 13, 14]),
        [GREEN, BLACK, BLACK, GREEN]
    );
}

#[test]
fn specified_cell_widths_leave_spare_width_to_auto_cells() {
    check_table_boxes(
        "<style>td{width:auto}</style><table id=\"a\" style=\"width:122px\">\
         <tr><td id=\"a1\" style=\"width:25%\"><i></i></td><td id=\"a2\"><i></i></td><td id=\"a3\" style=\"width:30px\"><i></i></td></tr></table>",
        &[
            ("a", [0.0, 0.0, 122.0, 14.0]),
            ("a1", [1.0, 1.0, 30.0, 12.0]),
            ("a2", [31.0, 1.0, 58.0, 12.0]),
            ("a3", [89.0, 1.0, 32.0, 12.0]),
        ],
    );
}

#[test]
fn specified_cell_widths_shrink_when_the_table_is_tight() {
    check_table_boxes(
        "<style>td{width:auto}</style><table id=\"b\" style=\"width:62px\">\
         <tr><td id=\"b1\" style=\"width:25%\"><i></i></td><td id=\"b2\"><i></i></td><td id=\"b3\" style=\"width:50px\"><i></i></td></tr></table>",
        &[
            ("b", [0.0, 0.0, 62.0, 14.0]),
            ("b1", [1.0, 1.0, 15.0, 12.0]),
            ("b2", [16.0, 1.0, 8.0, 12.0]),
            ("b3", [24.0, 1.0, 37.0, 12.0]),
        ],
    );
}

#[test]
fn auto_columns_do_not_make_cell_widths_inflexible() {
    check_table_boxes(
        "<style>td{width:auto}</style><table id=\"c\" style=\"width:122px\"><col><col><col>\
         <tr><td id=\"c1\" style=\"width:25%\"><i></i></td><td id=\"c2\"><i></i></td><td id=\"c3\" style=\"width:30px\"><i></i></td></tr></table>",
        &[
            ("c", [0.0, 0.0, 122.0, 14.0]),
            ("c1", [1.0, 1.0, 30.0, 12.0]),
            ("c2", [31.0, 1.0, 58.0, 12.0]),
            ("c3", [89.0, 1.0, 32.0, 12.0]),
        ],
    );
}

#[test]
fn visible_cells_and_children_stay_hidden_in_collapsed_sections() {
    for section in ["thead", "tbody", "tfoot"] {
        for visible in ["visibility:visible", ""] {
            let buffer = draw(&format!(
                "<table class=\"bare\"><{section} style=\"visibility:collapse\">\
                 <tr><td style=\"width:20px;{visible}\"><i style=\"display:block;\
                 visibility:visible;width:10px;height:10px;background:red\"></i></td></tr>\
                 </{section}></table>"
            ));
            assert_eq!(pixel(&buffer, 5, 5), WHITE, "{section}: {visible}");
        }
    }
}

#[test]
fn visible_cells_and_children_stay_hidden_in_collapsed_rows() {
    for visible in ["visibility:visible", ""] {
        let buffer = draw(&format!(
            "<table class=\"bare\"><tr style=\"visibility:collapse\">\
             <td style=\"width:20px;{visible}\"><i style=\"display:block;\
             visibility:visible;width:10px;height:10px;background:red\"></i></td></tr></table>"
        ));
        assert_eq!(pixel(&buffer, 5, 5), WHITE, "{visible}");
    }
}

#[test]
fn positioned_visible_content_can_paint_from_a_collapsed_row() {
    for position in [
        "position:relative",
        "position:relative;z-index:1",
        "transform:translateX(0px)",
    ] {
        for visible_cell in [false, true] {
            let (cell, child) = if visible_cell {
                (position, "")
            } else {
                ("", position)
            };
            let buffer = draw(&format!(
                "<table class=\"bare\"><tr style=\"visibility:collapse\">\
                 <td style=\"width:20px;visibility:visible;{cell}\"><i style=\"display:block;\
                 visibility:visible;width:10px;height:10px;background:red;{child}\"></i></td></tr></table>"
            ));
            assert_eq!(
                pixel(&buffer, 5, 5),
                RED,
                "{position}, cell: {visible_cell}"
            );
        }
    }
}
