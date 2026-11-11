mod support;

use pageframe::App;
use support::{open, pixel, render};

const BLACK: [u8; 4] = [0, 0, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const DARK_RED: [u8; 4] = [171, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const LIME: [u8; 4] = [0, 255, 0, 255];
const YELLOW: [u8; 4] = [255, 255, 0, 255];

fn draw(child: &str, body: &str) -> Vec<u8> {
    let html = format!(
        "<html><head><style>html,body{{margin:0;background:white}}\
         table{{border-collapse:collapse}}\
         td{{border:2px solid black;padding:0;background:lime;font-size:0}}\
         b{{display:inline-block;{child};background:yellow}}</style></head>\
         <body>{body}</body></html>"
    );
    let mut harness = open(App::new((), &html));
    harness.pump();
    render(&mut harness)
}

fn grid(cell: &str, table: &str) -> Vec<u8> {
    draw(
        "width:20px;height:14px",
        &format!(
            "<table style=\"{table}\"><tr><td style=\"{cell}\"><b></b></td><td><b></b></td></tr>\
             <tr><td><b></b></td><td><b></b></td></tr></table>"
        ),
    )
}

fn collapsed_row(cell: &str) -> Vec<u8> {
    draw(
        "width:6px;height:10px",
        &format!(
            "<table><col><col style=\"visibility:collapse\">\
             <tr><td><b></b></td><td><b></b></td></tr>\
             <tr style=\"visibility:collapse\">\
             <td style=\"position:relative;{cell}\"><b></b></td>\
             <td style=\"border-color:blue\"><b></b></td></tr>\
             <tr><td><b></b></td><td><b></b></td></tr></table>"
        ),
    )
}

#[test]
fn a_double_border_is_two_stripes_with_the_cells_showing_between() {
    let buffer = grid("border:6px double red", "");
    assert_eq!(pixel(&buffer, 0, 0), RED);
    assert_eq!(pixel(&buffer, 10, 1), RED);
    assert_eq!(pixel(&buffer, 10, 2), WHITE);
    assert_eq!(pixel(&buffer, 10, 3), LIME);
    assert_eq!(pixel(&buffer, 10, 4), RED);
    assert_eq!(pixel(&buffer, 2, 10), WHITE);
    assert_eq!(pixel(&buffer, 3, 10), LIME);
    assert_eq!(pixel(&buffer, 4, 10), RED);
    assert_eq!(pixel(&buffer, 27, 10), RED);
    assert_eq!(pixel(&buffer, 28, 10), LIME);
    assert_eq!(pixel(&buffer, 29, 10), LIME);
    assert_eq!(pixel(&buffer, 30, 10), RED);
    assert_eq!(pixel(&buffer, 10, 10), YELLOW);
}

#[test]
fn a_groove_border_is_dark_outside_and_plain_inside() {
    let buffer = grid("border:6px groove red", "");
    assert_eq!(pixel(&buffer, 10, 0), DARK_RED);
    assert_eq!(pixel(&buffer, 10, 2), DARK_RED);
    assert_eq!(pixel(&buffer, 10, 3), RED);
    assert_eq!(pixel(&buffer, 10, 5), RED);
    assert_eq!(pixel(&buffer, 1, 10), DARK_RED);
    assert_eq!(pixel(&buffer, 4, 10), RED);
    assert_eq!(pixel(&buffer, 27, 10), DARK_RED);
    assert_eq!(pixel(&buffer, 30, 10), RED);
    assert_eq!(pixel(&buffer, 10, 21), DARK_RED);
    assert_eq!(pixel(&buffer, 10, 24), RED);
}

#[test]
fn a_ridge_border_is_plain_outside_and_dark_inside() {
    let buffer = grid("border:6px ridge red", "");
    assert_eq!(pixel(&buffer, 10, 1), RED);
    assert_eq!(pixel(&buffer, 10, 4), DARK_RED);
    assert_eq!(pixel(&buffer, 1, 10), RED);
    assert_eq!(pixel(&buffer, 4, 10), DARK_RED);
}

#[test]
fn a_dashed_border_leaves_gaps_that_show_the_cell_behind() {
    let buffer = grid("border:6px dashed red", "");
    assert_eq!(pixel(&buffer, 5, 1), RED);
    assert_eq!(pixel(&buffer, 15, 1), WHITE);
    assert_eq!(pixel(&buffer, 15, 4), LIME);
    assert_eq!(pixel(&buffer, 25, 1), RED);
    assert_eq!(pixel(&buffer, 1, 5), RED);
    assert_eq!(pixel(&buffer, 1, 12), WHITE);
    assert_eq!(pixel(&buffer, 4, 12), LIME);
    assert_eq!(pixel(&buffer, 1, 20), RED);
}

#[test]
fn a_thick_dotted_border_is_a_row_of_round_dots() {
    let buffer = grid("border:6px dotted red", "");
    assert_eq!(pixel(&buffer, 3, 3), RED);
    assert_eq!(pixel(&buffer, 15, 2), RED);
    assert_eq!(pixel(&buffer, 28, 2), RED);
    assert_eq!(pixel(&buffer, 9, 1), WHITE);
    assert_eq!(pixel(&buffer, 9, 4), LIME);
    assert_eq!(pixel(&buffer, 2, 12), RED);
    assert_eq!(pixel(&buffer, 1, 7), WHITE);
    assert_eq!(pixel(&buffer, 4, 7), LIME);
    assert_ne!(pixel(&buffer, 13, 0), RED);
}

#[test]
fn a_cell_filled_by_its_child_leaves_the_gaps_of_a_table_border_empty() {
    let buffer = grid("", "border:6px dashed blue");
    assert_eq!(pixel(&buffer, 5, 1), BLUE);
    assert_eq!(pixel(&buffer, 5, 4), BLUE);
    assert_eq!(pixel(&buffer, 14, 1), WHITE);
    assert_eq!(pixel(&buffer, 14, 4), WHITE);
    assert_eq!(pixel(&buffer, 10, 10), YELLOW);
}

#[test]
fn a_hidden_border_in_a_collapsed_row_removes_the_line_between_rows() {
    let buffer = collapsed_row("border:4px hidden red");
    assert_eq!(pixel(&buffer, 0, 12), BLACK);
    assert_eq!(pixel(&buffer, 4, 10), YELLOW);
    assert_ne!(pixel(&buffer, 4, 12), BLACK);
    assert_ne!(pixel(&buffer, 4, 13), BLACK);
    assert_eq!(pixel(&buffer, 4, 15), YELLOW);
    assert_eq!(pixel(&buffer, 8, 12), BLUE);
    assert_eq!(pixel(&buffer, 8, 14), BLACK);
}

#[test]
fn a_double_border_in_a_collapsed_row_is_drawn_as_two_stripes() {
    let buffer = collapsed_row("border:4px double red");
    assert_eq!(pixel(&buffer, 5, 12), RED);
    assert_eq!(pixel(&buffer, 5, 13), LIME);
    assert_eq!(pixel(&buffer, 5, 14), LIME);
    assert_eq!(pixel(&buffer, 5, 15), RED);
    assert_eq!(pixel(&buffer, 0, 13), RED);
    assert_eq!(pixel(&buffer, 1, 13), WHITE);
    assert_eq!(pixel(&buffer, 2, 13), LIME);
    assert_eq!(pixel(&buffer, 3, 13), RED);
    assert_eq!(pixel(&buffer, 11, 13), BLACK);
    assert_eq!(pixel(&buffer, 13, 13), RED);
}
