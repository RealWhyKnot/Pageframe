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
