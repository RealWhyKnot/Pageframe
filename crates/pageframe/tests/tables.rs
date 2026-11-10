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
