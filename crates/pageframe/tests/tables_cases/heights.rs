use super::*;

#[test]
fn spare_height_is_shared_in_proportion_to_row_content() {
    check_row_heights(
        "<table id=\"a\">\
         <tr><td id=\"a1\"><i style=\"height:10px\"></i></td></tr>\
         <tr><td id=\"a2\"><i style=\"height:30px\"></i></td></tr></table>",
        &[
            ("a", 0.0, 122.0),
            ("a1", 1.0, 32.71875),
            ("a2", 33.71875, 87.28125),
        ],
    );
}

#[test]
fn empty_rows_share_spare_height_evenly() {
    check_row_heights(
        "<table id=\"k\" class=\"z\">\
         <tr><td id=\"k1\"></td></tr>\
         <tr><td id=\"k2\"></td></tr>\
         <tr><td id=\"k3\"></td></tr></table>",
        &[
            ("k", 0.0, 120.0),
            ("k1", 0.0, 40.0),
            ("k2", 40.0, 40.0),
            ("k3", 80.0, 40.0),
        ],
    );
}

#[test]
fn rows_with_heights_leave_spare_height_to_plain_rows() {
    check_row_heights(
        "<table id=\"d\">\
         <tr><td id=\"d1\" style=\"height:20px\"></td></tr>\
         <tr><td id=\"d2\"><i style=\"height:10px\"></i></td></tr>\
         <tr><td id=\"d3\"><i style=\"height:30px\"></i></td></tr></table>",
        &[
            ("d", 0.0, 122.0),
            ("d1", 1.0, 22.0),
            ("d2", 23.0, 26.71875),
            ("d3", 49.71875, 71.28125),
        ],
    );
}

#[test]
fn rows_with_heights_share_spare_height_when_every_row_has_one() {
    check_row_heights(
        "<table id=\"e\">\
         <tr><td id=\"e1\" style=\"height:20px\"></td></tr>\
         <tr><td id=\"e2\" style=\"height:40px\"></td></tr></table>",
        &[("e", 0.0, 122.0), ("e1", 1.0, 41.25), ("e2", 42.25, 78.75)],
    );
}

#[test]
fn the_body_takes_spare_height_after_the_header_and_footer() {
    check_row_heights(
        "<table id=\"n\">\
         <thead>\
         <tr><td id=\"n1\"><i style=\"height:10px\"></i></td></tr></thead>\
         <tbody>\
         <tr><td id=\"n2\"><i style=\"height:30px\"></i></td></tr></tbody>\
         <tfoot>\
         <tr><td id=\"n3\"><i style=\"height:20px\"></i></td></tr></tfoot></table>",
        &[
            ("n", 0.0, 122.0),
            ("n1", 1.0, 12.0),
            ("n2", 13.0, 86.0),
            ("n3", 99.0, 22.0),
        ],
    );
}

#[test]
fn two_body_sections_share_spare_height_in_proportion() {
    check_row_heights(
        "<table id=\"d\">\
         <thead>\
         <tr><td id=\"d1\"><i style=\"height:10px\"></i></td></tr></thead>\
         <tbody>\
         <tr><td id=\"d2\"><i style=\"height:10px\"></i></td></tr></tbody>\
         <tbody>\
         <tr><td id=\"d3\"><i style=\"height:30px\"></i></td></tr></tbody></table>",
        &[
            ("d", 0.0, 122.0),
            ("d1", 1.0, 12.0),
            ("d2", 13.0, 29.453125),
            ("d3", 42.453125, 78.546875),
        ],
    );
}

#[test]
fn a_table_min_height_spreads_rows() {
    check_row_heights(
        "<table id=\"l\" style=\"height:auto;min-height:122px\">\
         <tr><td id=\"l1\"><i style=\"height:10px\"></i></td></tr>\
         <tr><td id=\"l2\"><i style=\"height:30px\"></i></td></tr></table>",
        &[
            ("l", 0.0, 122.0),
            ("l1", 1.0, 32.71875),
            ("l2", 33.71875, 87.28125),
        ],
    );
}

#[test]
fn a_table_max_height_limits_spare_row_height() {
    check_row_heights(
        "<table id=\"h\" style=\"max-height:80px\">\
         <tr><td id=\"h1\"><i style=\"height:10px\"></i></td></tr>\
         <tr><td id=\"h2\"><i style=\"height:30px\"></i></td></tr></table>",
        &[
            ("h", 0.0, 80.0),
            ("h1", 1.0, 21.265625),
            ("h2", 22.265625, 56.734375),
        ],
    );
}

#[test]
fn table_padding_and_borders_are_outside_spare_row_height() {
    check_row_heights(
        "<table id=\"k\" style=\"border:6px solid blue;padding:3px;border-collapse:separate;border-spacing:0\">\
         <tr><td id=\"k1\"><i style=\"height:10px\"></i></td></tr>\
         <tr><td id=\"k2\"><i style=\"height:30px\"></i></td></tr></table>",
        &[
            ("k", 0.0, 122.0),
            ("k1", 9.0, 30.328125),
            ("k2", 39.328125, 73.671875),
        ],
    );
}

#[test]
fn a_caption_is_outside_spare_row_height() {
    check_row_heights(
        "<table id=\"f\">\
         <tr><td id=\"f1\"><i style=\"height:10px\"></i></td></tr>\
         <tr><td id=\"f2\"><i style=\"height:30px\"></i></td></tr>\
         <caption id=\"fc\" style=\"height:20px\"></caption></table>",
        &[
            ("f", 0.0, 142.0),
            ("f1", 21.0, 32.71875),
            ("f2", 53.71875, 87.28125),
            ("fc", 0.0, 20.0),
        ],
    );
}

#[test]
fn percent_rows_use_the_final_height_of_their_section() {
    check_row_heights(
        "<table id=\"c\">\
         <thead>\
         <tr><td id=\"c1\"><i style=\"height:10px\"></i></td></tr></thead>\
         <tbody>\
         <tr><td id=\"c2\" style=\"height:50%\"></td></tr>\
         <tr><td id=\"c3\"><i style=\"height:10px\"></i></td></tr></tbody></table>",
        &[
            ("c", 0.0, 122.0),
            ("c1", 1.0, 12.0),
            ("c2", 13.0, 54.0),
            ("c3", 67.0, 54.0),
        ],
    );
}

#[test]
fn percent_rows_share_the_space_left_after_plain_rows() {
    check_row_heights(
        "<table id=\"d\">\
         <tr><td id=\"d1\" style=\"height:60%\"></td></tr>\
         <tr><td id=\"d2\" style=\"height:60%\"></td></tr>\
         <tr><td id=\"d3\"><i style=\"height:10px\"></i></td></tr></table>",
        &[
            ("d", 0.0, 122.0),
            ("d1", 1.0, 64.75),
            ("d2", 65.75, 43.25),
            ("d3", 109.0, 12.0),
        ],
    );
}

#[test]
fn a_percent_header_row_keeps_its_minimum_with_a_body() {
    check_row_heights(
        "<table id=\"g\">\
         <thead>\
         <tr><td id=\"g1\" style=\"height:25%\"></td></tr></thead>\
         <tbody>\
         <tr><td id=\"g2\"><i style=\"height:10px\"></i></td></tr></tbody>\
         <tfoot>\
         <tr><td id=\"g3\"><i style=\"height:10px\"></i></td></tr></tfoot></table>",
        &[
            ("g", 0.0, 122.0),
            ("g1", 1.0, 2.0),
            ("g2", 3.0, 106.0),
            ("g3", 109.0, 12.0),
        ],
    );
}

#[test]
fn a_larger_cell_percent_height_sizes_its_row() {
    check_row_heights(
        "<table id=\"a\">\
         <tr style=\"height:20%\"><td id=\"a1\" style=\"height:40%\"></td></tr>\
         <tr><td id=\"a2\"><i style=\"height:10px\"></i></td></tr></table>",
        &[("a", 0.0, 122.0), ("a1", 1.0, 48.0), ("a2", 49.0, 72.0)],
    );
}

#[test]
fn a_larger_row_percent_height_sizes_its_row() {
    check_row_heights(
        "<table id=\"b\">\
         <tr style=\"height:40%\"><td id=\"b1\" style=\"height:20%\"></td></tr>\
         <tr><td id=\"b2\"><i style=\"height:10px\"></i></td></tr></table>",
        &[("b", 0.0, 122.0), ("b1", 1.0, 48.0), ("b2", 49.0, 72.0)],
    );
}

#[test]
fn a_rowspan_percent_height_does_not_size_individual_rows() {
    check_row_heights(
        "<table id=\"e\">\
         <tr><td id=\"e1\" rowspan=\"2\" style=\"height:50%\"></td><td id=\"e2\"><i style=\"height:10px\"></i></td></tr>\
         <tr><td id=\"e3\"><i style=\"height:10px\"></i></td></tr>\
         <tr><td id=\"e4\"><i style=\"height:10px\"></i></td><td id=\"e5\"></td></tr></table>",
        &[
            ("e", 0.0, 122.0),
            ("e1", 1.0, 80.0),
            ("e2", 1.0, 40.0),
            ("e3", 41.0, 40.0),
            ("e4", 81.0, 40.0),
            ("e5", 81.0, 40.0),
        ],
    );
}

#[test]
fn percent_rows_resolve_separately_in_two_body_sections() {
    check_row_heights(
        "<table id=\"b\">\
         <tbody>\
         <tr><td id=\"b1\" style=\"height:25%\"></td></tr>\
         <tr><td id=\"b2\"><i style=\"height:10px\"></i></td></tr></tbody>\
         <tbody>\
         <tr><td id=\"b3\" style=\"height:50%\"></td></tr>\
         <tr><td id=\"b4\"><i style=\"height:30px\"></i></td></tr></tbody></table>",
        &[
            ("b", 0.0, 122.0),
            ("b1", 1.0, 8.75),
            ("b2", 9.75, 26.25),
            ("b3", 36.0, 42.5),
            ("b4", 78.5, 42.5),
        ],
    );
}

#[test]
fn a_percent_header_without_a_body_uses_its_section_height() {
    check_row_heights(
        "<table id=\"g\">\
         <thead>\
         <tr><td id=\"g1\" style=\"height:25%\"></td></tr></thead>\
         <tfoot>\
         <tr><td id=\"g2\"><i style=\"height:10px\"></i></td></tr></tfoot></table>",
        &[
            ("g", 0.0, 122.0),
            ("g1", 1.0, 17.140625),
            ("g2", 18.140625, 102.859375),
        ],
    );
}

#[test]
fn a_percent_footer_row_keeps_its_minimum_with_a_body() {
    check_row_heights(
        "<table id=\"j\">\
         <tbody>\
         <tr><td id=\"j1\"><i style=\"height:10px\"></i></td></tr></tbody>\
         <tfoot>\
         <tr><td id=\"j2\" style=\"height:25%\"></td></tr></tfoot></table>",
        &[("j", 0.0, 122.0), ("j1", 1.0, 118.0), ("j2", 119.0, 2.0)],
    );
}

#[test]
fn a_percent_footer_alone_uses_its_section_height() {
    check_row_heights(
        "<table id=\"l\">\
         <tfoot>\
         <tr><td id=\"l1\" style=\"height:25%\"></td></tr>\
         <tr><td id=\"l2\"><i style=\"height:10px\"></i></td></tr></tfoot></table>",
        &[("l", 0.0, 122.0), ("l1", 1.0, 30.0), ("l2", 31.0, 90.0)],
    );
}

#[test]
fn overflowing_percent_rows_scale_their_growth_above_minimum() {
    check_row_heights(
        "<table id=\"d\">\
         <tr><td id=\"d1\" style=\"height:80%\"></td></tr>\
         <tr><td id=\"d2\" style=\"height:80%\"></td></tr>\
         <tr><td id=\"d3\"><i style=\"height:10px\"></i></td></tr></table>",
        &[
            ("d", 0.0, 122.0),
            ("d1", 1.0, 86.265625),
            ("d2", 87.265625, 21.734375),
            ("d3", 109.0, 12.0),
        ],
    );
}

#[test]
fn percent_rows_include_spacing_inside_their_section() {
    check_row_heights(
        "<table id=\"a\" style=\"height:100px;border-collapse:separate;border-spacing:4px\">\
         <tr><td id=\"a1\" style=\"height:30%\"></td></tr>\
         <tr><td id=\"a2\"></td></tr></table>",
        &[
            ("a", 0.0, 100.0),
            ("a1", 4.0, 27.59375),
            ("a2", 35.59375, 60.40625),
        ],
    );
}

#[test]
fn overflowing_percent_rows_leave_room_for_border_spacing() {
    check_row_heights(
        "<table id=\"b\" style=\"height:100px;border-collapse:separate;border-spacing:4px\">\
         <tr><td id=\"b1\" style=\"height:70%\"></td></tr>\
         <tr><td id=\"b2\" style=\"height:70%\"></td></tr></table>",
        &[
            ("b", 0.0, 100.0),
            ("b1", 4.0, 61.515625),
            ("b2", 69.515625, 26.484375),
        ],
    );
}

#[test]
fn body_percent_rows_include_only_their_own_spacing() {
    check_row_heights(
        "<table id=\"c\" style=\"height:100px;border-collapse:separate;border-spacing:4px\">\
         <thead>\
         <tr><td id=\"c1\"><i style=\"height:10px\"></i></td></tr></thead>\
         <tbody>\
         <tr><td id=\"c2\" style=\"height:50%\"></td></tr>\
         <tr><td id=\"c3\"><i style=\"height:10px\"></i></td></tr></tbody></table>",
        &[
            ("c", 0.0, 100.0),
            ("c1", 4.0, 14.0),
            ("c2", 22.0, 37.0),
            ("c3", 63.0, 33.0),
        ],
    );
}

#[test]
fn body_sections_share_spare_height_including_their_spacing() {
    check_row_heights(
        "<table id=\"d\" style=\"height:100px;border-collapse:separate;border-spacing:4px\">\
         <tbody>\
         <tr><td id=\"d1\" style=\"height:25%\"></td></tr>\
         <tr><td id=\"d2\"><i style=\"height:10px\"></i></td></tr></tbody>\
         <tbody>\
         <tr><td id=\"d3\" style=\"height:50%\"></td></tr>\
         <tr><td id=\"d4\"><i style=\"height:20px\"></i></td></tr></tbody></table>",
        &[
            ("d", 0.0, 100.0),
            ("d1", 4.0, 8.953125),
            ("d2", 16.953125, 22.890625),
            ("d3", 43.84375, 24.15625),
            ("d4", 72.0, 24.0),
        ],
    );
}
