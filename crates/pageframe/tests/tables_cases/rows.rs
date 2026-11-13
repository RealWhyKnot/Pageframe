use super::*;

#[test]
fn a_rowspan_stops_at_the_end_of_its_header() {
    check_table_boxes(
        "<style>table{height:122px}</style><table id=\"e\">\
         <thead>\
         <tr><td id=\"e1\" rowspan=\"2\"><i style=\"height:10px\"></i></td><td id=\"e2\"><i style=\"height:10px\"></i></td></tr></thead>\
         <tbody>\
         <tr><td id=\"e3\"><i style=\"height:10px\"></i></td><td id=\"e4\"><i style=\"height:30px\"></i></td></tr></tbody></table>",
        &[
            ("e", [0.0, 0.0, 26.0, 122.0]),
            ("e1", [1.0, 1.0, 12.0, 12.0]),
            ("e2", [13.0, 1.0, 12.0, 12.0]),
            ("e3", [1.0, 13.0, 12.0, 108.0]),
            ("e4", [13.0, 13.0, 12.0, 108.0]),
        ],
    );
}

#[test]
fn a_body_rowspan_does_not_enter_the_footer() {
    check_table_boxes(
        "<table id=\"b\" style=\"\">\
         <tbody>\
         <tr ><td id=\"b1\" rowspan=\"3\"><i style=\"height:10px\"></i></td><td id=\"b2\" ><i style=\"height:10px\"></i></td></tr></tbody>\
         <tfoot>\
         <tr ><td id=\"b4\" ><i style=\"height:10px\"></i></td><td id=\"b5\" ><i style=\"height:30px\"></i></td></tr></tfoot></table>",
        &[
            ("b", [0.0, 0.0, 26.0, 46.0]),
            ("b1", [1.0, 1.0, 12.0, 12.0]),
            ("b2", [13.0, 1.0, 12.0, 12.0]),
            ("b4", [1.0, 13.0, 12.0, 32.0]),
            ("b5", [13.0, 13.0, 12.0, 32.0]),
        ],
    );
}

#[test]
fn a_body_rowspan_does_not_enter_the_next_body() {
    check_table_boxes(
        "<table id=\"c\" style=\"\">\
         <tbody>\
         <tr ><td id=\"c1\" rowspan=\"3\"><i style=\"height:10px\"></i></td><td id=\"c2\" ><i style=\"height:10px\"></i></td></tr></tbody>\
         <tbody>\
         <tr ><td id=\"c4\" ><i style=\"height:10px\"></i></td><td id=\"c5\" ><i style=\"height:30px\"></i></td></tr></tbody></table>",
        &[
            ("c", [0.0, 0.0, 26.0, 46.0]),
            ("c1", [1.0, 1.0, 12.0, 12.0]),
            ("c2", [13.0, 1.0, 12.0, 12.0]),
            ("c4", [1.0, 13.0, 12.0, 32.0]),
            ("c5", [13.0, 13.0, 12.0, 32.0]),
        ],
    );
}

#[test]
fn a_zero_rowspan_ends_with_its_body() {
    check_table_boxes(
        "<table id=\"d\" style=\"\">\
         <tbody>\
         <tr ><td id=\"d1\" rowspan=\"0\"><i style=\"height:10px\"></i></td><td id=\"d2\" ><i style=\"height:10px\"></i></td></tr>\
         <tr ><td id=\"d3\" ><i style=\"height:20px\"></i></td></tr></tbody>\
         <tbody>\
         <tr ><td id=\"d4\" ><i style=\"height:10px\"></i></td><td id=\"d5\" ><i style=\"height:30px\"></i></td></tr></tbody></table>",
        &[
            ("d", [0.0, 0.0, 26.0, 68.0]),
            ("d1", [1.0, 1.0, 12.0, 34.0]),
            ("d2", [13.0, 1.0, 12.0, 12.0]),
            ("d3", [13.0, 13.0, 12.0, 22.0]),
            ("d4", [1.0, 35.0, 12.0, 32.0]),
            ("d5", [13.0, 35.0, 12.0, 32.0]),
        ],
    );
}

#[test]
fn a_rowspan_can_end_exactly_at_its_section_end() {
    check_table_boxes(
        "<table id=\"e\" style=\"\">\
         <tbody>\
         <tr ><td id=\"e1\" rowspan=\"2\"><i style=\"height:10px\"></i></td><td id=\"e2\" ><i style=\"height:10px\"></i></td></tr>\
         <tr ><td id=\"e3\" ><i style=\"height:20px\"></i></td></tr></tbody>\
         <tfoot>\
         <tr ><td id=\"e4\" ><i style=\"height:10px\"></i></td><td id=\"e5\" ><i style=\"height:30px\"></i></td></tr></tfoot></table>",
        &[
            ("e", [0.0, 0.0, 26.0, 68.0]),
            ("e1", [1.0, 1.0, 12.0, 34.0]),
            ("e2", [13.0, 1.0, 12.0, 12.0]),
            ("e3", [13.0, 13.0, 12.0, 22.0]),
            ("e4", [1.0, 35.0, 12.0, 32.0]),
            ("e5", [13.0, 35.0, 12.0, 32.0]),
        ],
    );
}

#[test]
fn a_zero_rowspan_ends_with_its_header() {
    check_table_boxes(
        "<table id=\"f\" style=\"\">\
         <thead>\
         <tr ><td id=\"f1\" rowspan=\"0\"><i style=\"height:10px\"></i></td><td id=\"f2\" ><i style=\"height:10px\"></i></td></tr>\
         <tr ><td id=\"f3\" ><i style=\"height:20px\"></i></td></tr></thead>\
         <tbody>\
         <tr ><td id=\"f4\" ><i style=\"height:10px\"></i></td><td id=\"f5\" ><i style=\"height:30px\"></i></td></tr></tbody></table>",
        &[
            ("f", [0.0, 0.0, 26.0, 68.0]),
            ("f1", [1.0, 1.0, 12.0, 34.0]),
            ("f2", [13.0, 1.0, 12.0, 12.0]),
            ("f3", [13.0, 13.0, 12.0, 22.0]),
            ("f4", [1.0, 35.0, 12.0, 32.0]),
            ("f5", [13.0, 35.0, 12.0, 32.0]),
        ],
    );
}

#[test]
fn rows_without_explicit_sections_keep_their_spans_separate() {
    check_table_boxes(
        "<table id=\"h\">\
         <tr><td id=\"h1\" rowspan=\"3\"><i style=\"height:10px\"></i></td><td id=\"h2\"></td></tr>\
         <tbody>\
         <tr><td id=\"h3\"></td><td id=\"h4\"></td></tr></tbody>\
         <tr><td id=\"h5\"></td></tr></table>",
        &[
            ("h", [0.0, 0.0, 26.0, 18.0]),
            ("h1", [1.0, 1.0, 12.0, 12.0]),
            ("h2", [13.0, 1.0, 12.0, 12.0]),
            ("h3", [1.0, 13.0, 12.0, 2.0]),
            ("h4", [13.0, 13.0, 12.0, 2.0]),
            ("h5", [1.0, 15.0, 12.0, 2.0]),
        ],
    );
}

#[test]
fn a_zero_rowspan_covers_the_remaining_rows() {
    check_table_boxes(
        "<table id=\"j\" style=\"\">\
         <tr ><td id=\"j1\" rowspan=\"0\"><i style=\"height:10px\"></i></td><td id=\"j2\" ><i style=\"height:10px\"></i></td></tr>\
         <tr ><td id=\"j3\" ><i style=\"height:30px\"></i></td></tr></table>",
        &[
            ("j", [0.0, 0.0, 26.0, 46.0]),
            ("j1", [1.0, 1.0, 12.0, 44.0]),
            ("j2", [13.0, 1.0, 12.0, 12.0]),
            ("j3", [13.0, 13.0, 12.0, 32.0]),
        ],
    );
}

#[test]
fn a_collapsed_row_loses_its_share_of_table_height() {
    check_table_boxes(
        "<style>table{height:122px}</style><table id=\"f\">\
         <tr><td id=\"f1\"><i style=\"height:10px\"></i></td></tr>\
         <tr style=\"visibility:collapse\"><td id=\"f2\"><i style=\"height:20px\"></i></td></tr>\
         <tr><td id=\"f3\"><i style=\"height:30px\"></i></td></tr></table>",
        &[
            ("f", [0.0, 0.0, 14.0, 82.0]),
            ("f1", [1.0, 1.0, 12.0, 21.8125]),
            ("f2", [1.0, 22.8125, 12.0, 0.0]),
            ("f3", [1.0, 22.8125, 12.0, 58.1875]),
        ],
    );
}

#[test]
fn a_collapsed_row_is_removed_from_an_auto_height_table() {
    check_table_boxes(
        "<table id=\"a\" style=\"\">\
         <tr ><td id=\"a1\" ><i style=\"height:10px\"></i></td></tr>\
         <tr style=\"visibility:collapse\"><td id=\"a2\" ><i style=\"height:20px\"></i></td></tr>\
         <tr ><td id=\"a3\" ><i style=\"height:30px\"></i></td></tr></table>",
        &[
            ("a", [0.0, 0.0, 14.0, 46.0]),
            ("a1", [1.0, 1.0, 12.0, 12.0]),
            ("a2", [1.0, 13.0, 12.0, 0.0]),
            ("a3", [1.0, 13.0, 12.0, 32.0]),
        ],
    );
}

#[test]
fn a_collapsed_row_removes_one_border_spacing_gap() {
    check_table_boxes(
        "<table id=\"b\" style=\"border-collapse:separate;border-spacing:4px\">\
         <tr ><td id=\"b1\" ><i style=\"height:10px\"></i></td></tr>\
         <tr style=\"visibility:collapse\"><td id=\"b2\" ><i style=\"height:20px\"></i></td></tr>\
         <tr ><td id=\"b3\" ><i style=\"height:30px\"></i></td></tr></table>",
        &[
            ("b", [0.0, 0.0, 22.0, 60.0]),
            ("b1", [4.0, 4.0, 14.0, 14.0]),
            ("b2", [4.0, 18.0, 14.0, 0.0]),
            ("b3", [4.0, 22.0, 14.0, 34.0]),
        ],
    );
}

#[test]
fn a_collapsed_row_loses_its_spare_height_and_spacing() {
    check_table_boxes(
        "<table id=\"c\" style=\"height:122px;border-collapse:separate;border-spacing:4px\">\
         <tr ><td id=\"c1\" ><i style=\"height:10px\"></i></td></tr>\
         <tr style=\"visibility:collapse\"><td id=\"c2\" ><i style=\"height:20px\"></i></td></tr>\
         <tr ><td id=\"c3\" ><i style=\"height:30px\"></i></td></tr></table>",
        &[
            ("c", [0.0, 0.0, 22.0, 82.671875]),
            ("c1", [4.0, 4.0, 14.0, 20.609375]),
            ("c2", [4.0, 24.609375, 14.0, 0.0]),
            ("c3", [4.0, 28.609375, 14.0, 50.0625]),
        ],
    );
}

#[test]
fn a_collapsed_header_loses_its_height() {
    check_table_boxes(
        "<table id=\"f\" style=\"height:122px\">\
         <thead style=\"visibility:collapse\">\
         <tr ><td id=\"f1\" ><i style=\"height:10px\"></i></td></tr></thead>\
         <tbody>\
         <tr ><td id=\"f2\" ><i style=\"height:20px\"></i></td></tr></tbody>\
         <tfoot>\
         <tr ><td id=\"f3\" ><i style=\"height:30px\"></i></td></tr></tfoot></table>",
        &[
            ("f", [0.0, 0.0, 14.0, 110.0]),
            ("f1", [1.0, 1.0, 12.0, 0.0]),
            ("f2", [1.0, 1.0, 12.0, 76.0]),
            ("f3", [1.0, 77.0, 12.0, 32.0]),
        ],
    );
}

#[test]
fn a_collapsed_body_loses_its_height() {
    check_table_boxes(
        "<table id=\"g\" style=\"height:122px\">\
         <thead>\
         <tr ><td id=\"g1\" ><i style=\"height:10px\"></i></td></tr></thead>\
         <tbody style=\"visibility:collapse\">\
         <tr ><td id=\"g2\" ><i style=\"height:20px\"></i></td></tr></tbody>\
         <tfoot>\
         <tr ><td id=\"g3\" ><i style=\"height:30px\"></i></td></tr></tfoot></table>",
        &[
            ("g", [0.0, 0.0, 14.0, 46.0]),
            ("g1", [1.0, 1.0, 12.0, 12.0]),
            ("g2", [1.0, 13.0, 12.0, 0.0]),
            ("g3", [1.0, 13.0, 12.0, 32.0]),
        ],
    );
}

#[test]
fn a_collapsed_footer_loses_its_height() {
    check_table_boxes(
        "<table id=\"h\" style=\"height:122px\">\
         <thead>\
         <tr ><td id=\"h1\" ><i style=\"height:10px\"></i></td></tr></thead>\
         <tbody>\
         <tr ><td id=\"h2\" ><i style=\"height:20px\"></i></td></tr></tbody>\
         <tfoot style=\"visibility:collapse\">\
         <tr ><td id=\"h3\" ><i style=\"height:30px\"></i></td></tr></tfoot></table>",
        &[
            ("h", [0.0, 0.0, 14.0, 90.0]),
            ("h1", [1.0, 1.0, 12.0, 12.0]),
            ("h2", [1.0, 13.0, 12.0, 76.0]),
            ("h3", [1.0, 89.0, 12.0, 0.0]),
        ],
    );
}

#[test]
fn a_rowspan_shrinks_across_a_collapsed_row() {
    check_table_boxes(
        "<table id=\"j\" style=\"\">\
         <tr ><td id=\"j1\" rowspan=\"3\"><i id=\"j1i\" style=\"height:50px\"></i></td><td id=\"j2\" ><i style=\"height:10px\"></i></td></tr>\
         <tr style=\"visibility:collapse\"><td id=\"j3\" ><i style=\"height:20px\"></i></td></tr>\
         <tr ><td id=\"j4\" ><i style=\"height:30px\"></i></td></tr></table>",
        &[
            ("j", [0.0, 0.0, 26.0, 46.0]),
            ("j1", [1.0, 1.0, 12.0, 44.0]),
            ("j1i", [2.0, 2.0, 6.0, 50.0]),
            ("j2", [13.0, 1.0, 12.0, 12.0]),
            ("j3", [13.0, 13.0, 12.0, 0.0]),
            ("j4", [13.0, 13.0, 12.0, 32.0]),
        ],
    );
}

#[test]
fn a_collapsed_row_keeps_its_column_width() {
    check_table_boxes(
        "<table id=\"k\" style=\"\">\
         <tr ><td id=\"k1\" ><i style=\"height:10px\"></i></td></tr>\
         <tr style=\"visibility:collapse;background:red\"><td id=\"k2\" style=\"width:50px;background:red;border:6px solid blue\"><i style=\"height:20px\"></i></td></tr>\
         <tr ><td id=\"k3\" ><i style=\"height:30px\"></i></td></tr></table>",
        &[
            ("k", [0.0, 0.0, 62.0, 50.0]),
            ("k1", [3.0, 1.0, 56.0, 14.0]),
            ("k2", [3.0, 15.0, 56.0, 0.0]),
            ("k3", [3.0, 15.0, 56.0, 34.0]),
        ],
    );
}

#[test]
fn a_rowspan_starting_in_a_collapsed_row_has_zero_height() {
    check_table_boxes(
        "<table id=\"n\" style=\"\">\
         <tr style=\"visibility:collapse\"><td id=\"n1\" rowspan=\"2\"><i id=\"n1i\" style=\"height:50px\"></i></td><td id=\"n2\" ><i style=\"height:10px\"></i></td></tr>\
         <tr ><td id=\"n3\" ><i style=\"height:30px\"></i></td></tr>\
         <tr ><td id=\"n4\" ><i style=\"height:10px\"></i></td><td id=\"n5\" ><i style=\"height:10px\"></i></td></tr></table>",
        &[
            ("n", [0.0, 0.0, 26.0, 51.828125]),
            ("n1", [1.0, 1.0, 12.0, 0.0]),
            ("n1i", [2.0, 2.0, 6.0, 50.0]),
            ("n2", [13.0, 1.0, 12.0, 0.0]),
            ("n3", [13.0, 1.0, 12.0, 37.828125]),
            ("n4", [1.0, 38.828125, 12.0, 12.0]),
            ("n5", [13.0, 38.828125, 12.0, 12.0]),
        ],
    );
}

#[test]
fn collapsing_the_first_row_preserves_outer_spacing() {
    check_table_boxes(
        "<table id=\"o\" style=\"border-collapse:separate;border-spacing:4px\">\
         <tr style=\"visibility:collapse\"><td id=\"o1\"><i style=\"height:10px\"></i></td></tr>\
         <tr><td id=\"o2\"><i style=\"height:20px\"></i></td></tr></table>",
        &[
            ("o", [0.0, 0.0, 22.0, 32.0]),
            ("o1", [4.0, 4.0, 14.0, 0.0]),
            ("o2", [4.0, 4.0, 14.0, 24.0]),
        ],
    );
}

#[test]
fn collapsing_every_row_preserves_outer_spacing() {
    check_table_boxes(
        "<table id=\"p\" style=\"border-collapse:separate;border-spacing:4px\">\
         <tr style=\"visibility:collapse\"><td id=\"p1\"><i style=\"height:10px\"></i></td></tr>\
         <tr style=\"visibility:collapse\"><td id=\"p2\"><i style=\"height:20px\"></i></td></tr></table>",
        &[
            ("p", [0.0, 0.0, 22.0, 8.0]),
            ("p1", [4.0, 4.0, 14.0, 0.0]),
            ("p2", [4.0, 4.0, 14.0, 0.0]),
        ],
    );
}
