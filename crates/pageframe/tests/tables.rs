mod support;

use pageframe::App;
use support::{open, pixel, render};

const STYLE: &str = "html,body{margin:0;background:white}\
table{border-collapse:collapse}\
td{border:2px solid black;padding:5px 10px;background:#0f0}\
.thin td{border-width:1px}";

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
