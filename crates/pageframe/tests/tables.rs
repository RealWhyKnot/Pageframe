mod support;

use pageframe::App;
use support::{open, pixel, render};

const STYLE: &str = "html,body{margin:0;background:white}\
table{border-collapse:collapse}\
td{border:2px solid black;padding:5px 10px;background:#0f0}\
.thin td{border-width:1px}\
i{display:inline-block;height:10px}\
.bare td{padding:0;font-size:0}\
caption{background:#00f;font-size:0}";

const BLACK: [u8; 4] = [0, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

fn draw(body: &str) -> Vec<u8> {
    let html = format!("<html><head><style>{STYLE}</style></head><body>{body}</body></html>");
    let mut harness = open(App::new((), &html));
    harness.pump();
    render(&mut harness)
}

fn row(buffer: &[u8], xs: &[usize], y: usize) -> Vec<[u8; 4]> {
    xs.iter().map(|&x| pixel(buffer, x, y)).collect()
}

fn column(buffer: &[u8], x: usize, ys: &[usize]) -> Vec<[u8; 4]> {
    ys.iter().map(|&y| pixel(buffer, x, y)).collect()
}

#[test]
fn collapsed_cells_share_their_borders() {
    let buffer = draw("<table><tr><td></td><td></td></tr><tr><td></td><td></td></tr></table>");
    assert_eq!(
        row(&buffer, &[0, 1, 2, 21, 22, 23, 24, 43, 44, 45, 46], 6),
        [
            BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, WHITE
        ]
    );
    assert_eq!(
        column(&buffer, 6, &[0, 1, 2, 11, 12, 13, 14, 23, 24, 25, 26]),
        [
            BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, WHITE
        ]
    );
}

#[test]
fn thin_collapsed_borders_stay_one_pixel_wide() {
    let buffer = draw("<table class=\"thin\"><tr><td></td><td></td></tr></table>");
    assert_eq!(
        row(&buffer, &[0, 1, 20, 21, 22, 41, 42, 43], 5),
        [BLACK, GREEN, GREEN, BLACK, GREEN, GREEN, BLACK, WHITE]
    );
    assert_eq!(
        column(&buffer, 5, &[0, 1, 10, 11, 12]),
        [BLACK, GREEN, GREEN, BLACK, WHITE]
    );
}

#[test]
fn no_border_crosses_a_cell_that_spans_rows() {
    let buffer = draw("<table><tr><td rowspan=\"2\"></td><td></td></tr><tr><td></td></tr></table>");
    assert_eq!(
        column(&buffer, 10, &[11, 12, 13, 14, 23, 24]),
        [GREEN, GREEN, GREEN, GREEN, GREEN, BLACK]
    );
    assert_eq!(
        column(&buffer, 30, &[11, 12, 13, 14]),
        [GREEN, BLACK, BLACK, GREEN]
    );
}

#[test]
fn a_missing_cell_gets_no_border() {
    let buffer = draw("<table><tr><td></td><td></td></tr><tr><td></td></tr></table>");
    assert_eq!(
        column(&buffer, 30, &[12, 13, 14, 20, 24, 25]),
        [BLACK, BLACK, WHITE, WHITE, WHITE, WHITE]
    );
    assert_eq!(
        row(&buffer, &[22, 23, 24, 44, 45], 20),
        [BLACK, BLACK, WHITE, WHITE, WHITE]
    );
}

#[test]
fn the_first_cell_colours_a_shared_border() {
    let buffer = draw(
        "<table><tr><td style=\"border-color:red\"></td><td></td></tr>\
         <tr><td></td><td></td></tr></table>",
    );
    assert_eq!(
        row(&buffer, &[0, 1, 22, 23, 44, 45], 6),
        [RED, RED, RED, RED, BLACK, BLACK]
    );
    assert_eq!(
        column(&buffer, 6, &[0, 1, 12, 13, 24, 25]),
        [RED, RED, RED, RED, BLACK, BLACK]
    );
}

#[test]
fn a_wide_table_border_pushes_the_outer_cells_in() {
    let buffer = draw("<table style=\"border:6px solid blue\"><tr><td></td><td></td></tr></table>");
    assert_eq!(
        row(&buffer, &[0, 5, 6, 25, 26, 27, 28, 47, 48, 53, 54], 11),
        [
            BLUE, BLUE, GREEN, GREEN, BLACK, BLACK, GREEN, GREEN, BLUE, BLUE, WHITE
        ]
    );
    assert_eq!(
        column(&buffer, 10, &[0, 5, 6, 15, 16, 21, 22]),
        [BLUE, BLUE, GREEN, GREEN, BLUE, BLUE, WHITE]
    );
}

#[test]
fn a_borderless_cell_makes_room_for_its_neighbours_border() {
    let buffer = draw("<table><tr><td style=\"border:none\"></td><td></td></tr></table>");
    assert_eq!(
        row(&buffer, &[0, 19, 20, 21, 22, 41, 42, 43, 44], 6),
        [
            GREEN, GREEN, BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, WHITE
        ]
    );
}

#[test]
fn a_cell_outside_a_table_keeps_its_border() {
    let buffer = draw(
        "<div style=\"border-collapse:collapse\">\
         <div style=\"display:table-cell;border:2px solid black;width:20px;height:10px\"></div>\
         </div>",
    );
    assert_eq!(row(&buffer, &[1, 10], 6), [BLACK, WHITE]);
    assert_eq!(column(&buffer, 10, &[1, 6]), [BLACK, WHITE]);
}

#[test]
fn spare_width_goes_to_the_column_with_more_content() {
    let buffer = draw(
        "<table class=\"bare\" style=\"width:62px\"><tr>\
         <td><i style=\"width:10px\"></i></td>\
         <td><i style=\"width:34px\"></i></td></tr></table>",
    );
    assert_eq!(
        row(&buffer, &[14, 15, 16, 17, 59, 60, 61, 62], 6),
        [GREEN, BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, WHITE]
    );
}

#[test]
fn a_narrow_table_shrinks_the_columns_that_can_wrap() {
    let narrow = "<i style=\"width:10px\"></i> <i style=\"width:10px\"></i>";
    let buffer = draw(&format!(
        "<table class=\"bare\" style=\"width:46px\"><tr>\
         <td>{narrow}</td><td>{narrow} {narrow}</td></tr></table>"
    ));
    assert_eq!(
        row(&buffer, &[16, 17, 18, 19, 43, 44, 45, 46], 6),
        [GREEN, BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, WHITE]
    );
}

#[test]
fn a_cell_under_a_row_span_widens_its_own_column() {
    let buffer = draw(
        "<table><tr><td rowspan=\"2\"></td><td></td></tr>\
         <tr><td><i style=\"width:30px\"></i></td></tr></table>",
    );
    assert_eq!(
        row(&buffer, &[21, 22, 23, 24, 73, 74, 75, 76], 6),
        [GREEN, BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, WHITE]
    );
}

#[test]
fn a_percent_cell_sets_its_column_width() {
    let buffer = draw(
        "<table style=\"width:102px\"><tr><td style=\"width:25%\"></td><td></td></tr></table>",
    );
    assert_eq!(
        row(&buffer, &[24, 25, 26, 27, 99, 100, 101, 102], 6),
        [GREEN, BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, WHITE]
    );
}

#[test]
fn a_caption_sits_above_the_cells() {
    let buffer = draw("<table><caption style=\"height:10px\"></caption><tr><td></td></tr></table>");
    assert_eq!(
        column(&buffer, 6, &[0, 9, 10, 11, 12, 21, 22, 23, 24]),
        [BLUE, BLUE, BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, WHITE]
    );
    assert_eq!(row(&buffer, &[0, 23, 24], 5), [BLUE, BLUE, WHITE]);
}

#[test]
fn a_bottom_caption_sits_below_the_cells() {
    let buffer = draw(
        "<table><caption style=\"height:10px;caption-side:bottom\"></caption>\
         <tr><td></td></tr></table>",
    );
    assert_eq!(
        column(&buffer, 6, &[0, 1, 2, 11, 12, 13, 14, 23, 24]),
        [BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, BLUE, BLUE, WHITE]
    );
}

#[test]
fn a_wide_caption_widens_its_table() {
    let buffer =
        draw("<table><caption><i style=\"width:60px\"></i></caption><tr><td></td></tr></table>");
    assert_eq!(row(&buffer, &[0, 59, 60], 5), [BLUE, BLUE, WHITE]);
    assert_eq!(
        row(&buffer, &[0, 1, 2, 57, 58, 59, 60], 16),
        [BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, WHITE]
    );
}

#[test]
fn a_table_border_goes_around_the_cells_not_the_caption() {
    let buffer = draw(
        "<table style=\"border:4px solid red;border-collapse:separate\">\
         <caption style=\"height:10px\"></caption><tr><td></td></tr></table>",
    );
    assert_eq!(
        column(&buffer, 1, &[0, 9, 10, 35, 36]),
        [BLUE, BLUE, RED, RED, WHITE]
    );
    assert_eq!(row(&buffer, &[0, 35, 36], 12), [RED, RED, WHITE]);
}

#[test]
fn a_caption_widens_a_table_narrower_than_it() {
    let buffer = draw(
        "<table style=\"width:50px\"><caption><i style=\"width:60px\"></i></caption>\
         <tr><td></td></tr></table>\
         <table><caption><i style=\"width:70px\"></i></caption></table>",
    );
    assert_eq!(row(&buffer, &[0, 59, 60], 5), [BLUE, BLUE, WHITE]);
    assert_eq!(
        row(&buffer, &[0, 1, 2, 57, 58, 59, 60], 16),
        [BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, WHITE]
    );
    assert_eq!(row(&buffer, &[0, 69, 70], 29), [BLUE, BLUE, WHITE]);
}

#[test]
fn a_spanning_cell_with_a_height_fills_its_rows() {
    let cell = "style=\"width:20px;height:10px\"";
    let buffer = draw(&format!(
        "<table class=\"bare\"><tr><td rowspan=\"2\" {cell}></td><td {cell}></td></tr>\
         <tr><td {cell}></td></tr></table>"
    ));
    assert_eq!(
        column(&buffer, 6, &[0, 1, 2, 12, 13, 23, 24, 25, 26]),
        [
            BLACK, BLACK, GREEN, GREEN, GREEN, GREEN, BLACK, BLACK, WHITE
        ]
    );
}

#[test]
fn cells_without_a_table_sit_in_one_row() {
    let cell = "display:table-cell;width:20px;height:10px";
    let buffer = draw(&format!(
        "<div><div style=\"{cell};background:#0f0\"></div>\
         <div style=\"{cell};background:#00f\"></div></div>"
    ));
    assert_eq!(
        row(&buffer, &[0, 19, 20, 39, 40], 5),
        [GREEN, GREEN, BLUE, BLUE, WHITE]
    );
    assert_eq!(column(&buffer, 5, &[9, 10]), [GREEN, WHITE]);
}

#[test]
fn whitespace_between_stray_cells_keeps_them_in_one_row() {
    let cell = "display:table-cell;width:20px;height:10px";
    let buffer = draw(&format!(
        "<div><div style=\"{cell};background:#0f0\"></div> \n \
         <div style=\"{cell};background:#00f\"></div></div>"
    ));
    assert_eq!(
        row(&buffer, &[0, 19, 20, 39, 40], 5),
        [GREEN, GREEN, BLUE, BLUE, WHITE]
    );
    assert_eq!(column(&buffer, 25, &[9, 10]), [BLUE, WHITE]);
}

#[test]
fn a_percent_cell_widens_a_table_without_a_width() {
    let buffer = draw(
        "<table class=\"bare\"><tr><td style=\"width:50%\"></td>\
         <td><i style=\"width:40px\"></i></td></tr></table>",
    );
    assert_eq!(
        row(&buffer, &[1, 2, 41, 42, 43, 44, 83, 84, 85, 86], 5),
        [
            BLACK, GREEN, GREEN, BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, WHITE
        ]
    );
}

#[test]
fn a_full_width_percent_cell_leaves_room_for_its_neighbour() {
    let buffer = draw(
        "<table class=\"bare\"><tr><td style=\"width:100%\"></td>\
         <td><i style=\"width:20px\"></i></td></tr></table>",
    );
    assert_eq!(
        row(&buffer, &[1, 2, 95, 96, 97, 98, 117, 118, 119], 5),
        [
            BLACK, GREEN, GREEN, BLACK, BLACK, GREEN, GREEN, BLACK, BLACK
        ]
    );
}

#[test]
fn a_percent_cell_height_sizes_its_row() {
    let buffer = draw(
        "<table class=\"bare\" style=\"height:62px\">\
         <tr><td style=\"width:10px;height:25%\"></td></tr><tr><td></td></tr></table>",
    );
    assert_eq!(
        column(&buffer, 5, &[1, 2, 14, 15, 16, 17, 59, 60, 61, 62]),
        [
            BLACK, GREEN, GREEN, BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, WHITE
        ]
    );
}

#[test]
fn percent_rows_alone_share_the_whole_table_height() {
    let buffer = draw(
        "<table class=\"bare\" style=\"height:52px\">\
         <tr><td style=\"width:10px;height:30%\"></td></tr>\
         <tr><td style=\"height:20%\"></td></tr></table>",
    );
    assert_eq!(
        column(&buffer, 5, &[2, 29, 30, 31, 32, 49, 50, 51, 52]),
        [
            GREEN, GREEN, BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, WHITE
        ]
    );
}

#[test]
fn content_taller_than_a_percent_row_grows_it() {
    let buffer = draw(
        "<table class=\"bare\" style=\"height:62px\"><tr><td style=\"height:10%\">\
         <i style=\"width:10px;height:30px\"></i></td></tr><tr><td></td></tr></table>",
    );
    assert_eq!(
        column(&buffer, 5, &[2, 31, 32, 33, 34, 59, 60, 61, 62]),
        [
            GREEN, GREEN, BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, WHITE
        ]
    );
}

#[test]
fn percent_cells_past_the_whole_width_get_what_is_left() {
    let buffer = draw(
        "<table class=\"bare\"><tr><td style=\"width:60%\"><i style=\"width:20px\"></i></td>\
         <td style=\"width:60%\"><i style=\"width:20px\"></i></td></tr></table>",
    );
    assert_eq!(
        row(&buffer, &[1, 2, 32, 33, 34, 35, 54, 55, 56, 57], 5),
        [
            BLACK, GREEN, GREEN, BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, WHITE
        ]
    );
}

#[test]
fn line_breaks_in_a_cell_keep_their_line_height() {
    let buffer = draw(
        "<table class=\"bare\"><tr>\
         <td style=\"font:10px/20px sans-serif;color:#0f0\">a<br>b<br>c</td></tr></table>",
    );
    assert_eq!(
        column(&buffer, 3, &[0, 1, 2, 61, 62, 63, 64]),
        [BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, WHITE]
    );
}

#[test]
fn content_taller_than_a_cell_height_grows_the_cell() {
    let buffer = draw(
        "<table class=\"bare\"><tr><td style=\"height:10px\">\
         <i style=\"width:10px;height:30px\"></i></td></tr></table>",
    );
    assert_eq!(
        column(&buffer, 5, &[0, 1, 2, 31, 32, 33, 34]),
        [BLACK, BLACK, GREEN, GREEN, BLACK, BLACK, WHITE]
    );
}

fn check_row_heights(body: &str, expected: &[(&str, f32, f32)]) {
    let html = format!(
        "<html><head><style>{STYLE}table{{height:122px}}\
         td{{padding:0;width:10px;font-size:0}}\
         i{{width:6px;vertical-align:top}}\
         .z{{border-collapse:separate;border-spacing:0;height:120px}}\
         .z td{{border:0}}</style></head><body>{body}</body></html>"
    );
    let mut harness = open(App::new((), &html));
    harness.pump();
    for &(id, top, height) in expected {
        let doc = harness.base_mut();
        let node_id = doc
            .query_selector_all(&format!("#{id}"))
            .expect("row selector is valid")[0];
        let node = doc.get_node(node_id).expect("row exists");
        let actual_top = node.absolute_position(0.0, 0.0).y;
        let actual_height = node.final_layout().size.height;
        drop(doc);
        assert!(
            (actual_top - top).abs() < 0.95,
            "{id}: top {actual_top}, expected {top}"
        );
        assert!(
            (actual_height - height).abs() < 0.95,
            "{id}: height {actual_height}, expected {height}"
        );
    }
}

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

fn check_table_boxes(body: &str, expected: &[(&str, [f32; 4])]) {
    let html = format!(
        "<html><head><style>{STYLE}td{{padding:0;width:10px;font-size:0}}\
         i{{width:6px;vertical-align:top}}</style></head><body>{body}</body></html>"
    );
    let mut harness = open(App::new((), &html));
    harness.pump();
    for &(id, expected) in expected {
        let doc = harness.base_mut();
        let node_id = doc
            .query_selector_all(&format!("#{id}"))
            .expect("box selector is valid")[0];
        let node = doc.get_node(node_id).expect("box exists");
        let position = node.absolute_position(0.0, 0.0);
        let size = node.final_layout().size;
        let actual = [position.x, position.y, size.width, size.height];
        drop(doc);
        assert!(
            actual
                .iter()
                .zip(expected)
                .all(|(actual, expected)| (actual - expected).abs() < 0.95),
            "{id}: {actual:?}, expected {expected:?}"
        );
    }
}

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

const SPAN_STYLE: &str = "<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x{visibility:collapse}div{position:absolute;left:0;top:0}</style>";

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
