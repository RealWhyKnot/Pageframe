mod support;

use pageframe::App;
use support::{open, pixel, render};

const STYLE: &str = "html,body{margin:0;background:white}\
td{border:2px solid black;padding:0;background:lime;font-size:0}\
b{display:inline-block;width:6px;height:10px;background:yellow}\
.r{background:red}.v b{background:blue}";

const BLACK: [u8; 4] = [0, 0, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const YELLOW: [u8; 4] = [255, 255, 0, 255];

fn draw(body: &str) -> Vec<u8> {
    let html = format!("<html><head><style>{STYLE}</style></head><body>{body}</body></html>");
    let mut harness = open(App::new((), &html));
    harness.pump();
    render(&mut harness)
}

fn column_table(cell: &str, child: &str) -> String {
    format!(
        "<table><col style=\"visibility:collapse\"><col><tr>\
         <td style=\"{cell}\"><b></b><b class=\"r\" style=\"{child}\"></b></td>\
         <td class=\"v\"><b></b></td></tr></table>"
    )
}

fn row_table(cell: &str, child: &str) -> String {
    format!(
        "<table><tr style=\"visibility:collapse\">\
         <td style=\"{cell}\"><b></b><b class=\"r\" style=\"{child}\"></b></td>\
         <td class=\"v\"><b></b></td></tr></table>"
    )
}

fn check_wrapped_child(buffer: &[u8]) {
    assert_eq!(pixel(buffer, 6, 8), BLUE);
    assert_eq!(pixel(buffer, 13, 8), WHITE);
    assert_eq!(pixel(buffer, 3, 15), BLACK);
    assert_eq!(pixel(buffer, 6, 15), RED);
    assert_eq!(pixel(buffer, 10, 15), BLACK);
    assert_eq!(pixel(buffer, 3, 20), WHITE);
    assert_eq!(pixel(buffer, 6, 20), RED);
    assert_eq!(pixel(buffer, 10, 20), WHITE);
    assert_eq!(pixel(buffer, 6, 24), WHITE);
}

fn check_children_without_a_cell(buffer: &[u8]) {
    assert_eq!(pixel(buffer, 6, 3), WHITE);
    assert_eq!(pixel(buffer, 3, 8), WHITE);
    assert_eq!(pixel(buffer, 6, 8), YELLOW);
    assert_eq!(pixel(buffer, 12, 8), RED);
    assert_eq!(pixel(buffer, 17, 8), WHITE);
    assert_eq!(pixel(buffer, 6, 15), WHITE);
}

#[test]
fn a_relative_child_of_a_cell_in_a_collapsed_column_is_painted() {
    check_wrapped_child(&draw(&column_table("", "position:relative")));
}

#[test]
fn a_child_with_a_z_index_in_a_collapsed_column_is_painted() {
    check_wrapped_child(&draw(&column_table("", "position:relative;z-index:1")));
}

#[test]
fn a_transformed_child_of_a_cell_in_a_collapsed_column_is_painted() {
    check_wrapped_child(&draw(&column_table("", "transform:translateX(0px)")));
}

#[test]
fn an_absolute_child_of_a_cell_in_a_collapsed_column_wraps_to_the_next_line() {
    check_wrapped_child(&draw(&column_table("", "position:absolute")));
}

#[test]
fn a_fixed_child_of_a_cell_in_a_collapsed_column_wraps_to_the_next_line() {
    check_wrapped_child(&draw(&column_table("", "position:fixed")));
}

#[test]
fn a_relative_cell_in_a_collapsed_column_paints_its_relative_child() {
    check_wrapped_child(&draw(&column_table(
        "position:relative;visibility:visible",
        "position:relative",
    )));
}

#[test]
fn a_relative_cell_in_a_collapsed_column_places_its_fixed_child() {
    check_wrapped_child(&draw(&column_table(
        "position:relative;visibility:visible",
        "position:fixed",
    )));
}

#[test]
fn a_relative_cell_in_a_collapsed_row_paints_its_children_without_its_box() {
    check_children_without_a_cell(&draw(&row_table(
        "position:relative;visibility:visible",
        "position:relative",
    )));
}

#[test]
fn a_relative_cell_in_a_collapsed_row_paints_its_absolute_child_without_its_box() {
    check_children_without_a_cell(&draw(&row_table(
        "position:relative;visibility:visible",
        "position:absolute",
    )));
}

#[test]
fn a_cell_with_a_z_index_in_a_collapsed_column_paints_its_relative_child() {
    check_wrapped_child(&draw(&column_table(
        "position:relative;z-index:1",
        "position:relative",
    )));
}

#[test]
fn a_transformed_cell_in_a_collapsed_column_paints_its_relative_child() {
    check_wrapped_child(&draw(&column_table(
        "transform:translateX(0px)",
        "position:relative",
    )));
}

#[test]
fn an_absolute_child_below_a_collapsed_row_moves_up_with_its_cell() {
    let buffer = draw(
        "<table><tr><td><b></b></td></tr>\
         <tr style=\"visibility:collapse\"><td><b style=\"height:20px\"></b></td></tr>\
         <tr><td><b></b><b class=\"r\" style=\"position:absolute\"></b></td></tr></table>",
    );
    assert_eq!(pixel(&buffer, 6, 24), YELLOW);
    assert_eq!(pixel(&buffer, 12, 24), RED);
    assert_eq!(pixel(&buffer, 12, 50), WHITE);
}

#[test]
fn a_faded_cell_in_a_collapsed_column_fades_its_relative_child() {
    let buffer = draw(&column_table("opacity:.5", "position:relative"));
    let [red, green, blue, _] = pixel(&buffer, 5, 17);
    assert_eq!(red, 255);
    assert!(green.abs_diff(127) <= 1 && blue.abs_diff(127) <= 1);
    let [red, green, blue, _] = pixel(&buffer, 5, 14);
    assert!(red.abs_diff(127) <= 1 && green == 0 && blue == 0);
}

fn wrapped_table(wrapper: &str) -> String {
    format!(
        "<table><col style=\"visibility:collapse\"><col><tr>\
         <td style=\"opacity:.5\"><b></b><div style=\"{wrapper}\">\
         <b class=\"r\" style=\"position:relative;z-index:1\"></b></div></td>\
         <td class=\"v\"><b></b></td></tr></table>"
    )
}

fn check_half_red_over_lime(buffer: &[u8]) {
    let [red, green, blue, _] = pixel(buffer, 5, 20);
    assert!(red.abs_diff(128) <= 1 && green.abs_diff(127) <= 1 && blue == 0);
}

#[test]
fn a_faded_cell_in_a_collapsed_column_fades_a_child_that_passes_a_relative_box() {
    check_half_red_over_lime(&draw(&wrapped_table("position:relative")));
}

#[test]
fn a_faded_row_fades_the_children_of_its_cell_in_a_collapsed_column() {
    let buffer = draw(
        "<table><col style=\"visibility:collapse\"><col><tr style=\"opacity:.5\">\
         <td style=\"opacity:.5\"><b></b>\
         <b class=\"r\" style=\"position:relative;top:30px\"></b></td>\
         <td class=\"v\"><b></b></td></tr></table>",
    );
    let [red, green, blue, _] = pixel(&buffer, 6, 48);
    assert_eq!(red, 255);
    assert!(green.abs_diff(191) <= 1 && blue.abs_diff(191) <= 1);
}

#[test]
fn a_faded_cell_in_a_collapsed_column_fades_an_absolute_child_of_a_clipping_box() {
    let buffer = draw(
        "<table><col style=\"visibility:collapse\"><col><tr>\
         <td style=\"opacity:.5\"><b></b><div style=\"overflow:hidden\">\
         <b class=\"r\" style=\"position:absolute\"></b></div></td>\
         <td class=\"v\"><b></b></td></tr></table>",
    );
    let [red, green, blue, _] = pixel(&buffer, 5, 20);
    assert_eq!(red, 255);
    assert!(green.abs_diff(127) <= 1 && blue.abs_diff(127) <= 1);
}

const MARKER: &str = "<i style=\"display:inline-block;width:5px;height:1px;background:red\"></i>";
const CELLS: &str = "<tr><td><b></b></td><td><b></b></td><td><b></b></td></tr>";

#[test]
fn an_inline_block_holding_only_a_table_sits_on_the_baseline() {
    let buffer = draw(&format!(
        "<div><span style=\"display:inline-block\"><table>{CELLS}</table></span>{MARKER}</div>"
    ));
    assert_eq!(pixel(&buffer, 40, 17), RED);
    assert_eq!(pixel(&buffer, 40, 13), WHITE);
}

#[test]
fn an_inline_block_holding_a_table_in_a_block_sits_on_the_baseline() {
    let buffer = draw(&format!(
        "<div><span style=\"display:inline-block\"><div><table>{CELLS}</table></div></span>{MARKER}</div>"
    ));
    assert_eq!(pixel(&buffer, 40, 17), RED);
    assert_eq!(pixel(&buffer, 40, 13), WHITE);
}

#[test]
fn an_inline_table_keeps_its_first_row_baseline() {
    let buffer = draw(&format!(
        "<div><table style=\"display:inline-table\">{CELLS}{CELLS}</table>{MARKER}</div>"
    ));
    assert_eq!(pixel(&buffer, 40, 13), RED);
}

const LINE: &str = "<div style=\"font-size:0\"><b></b></div>";

#[test]
fn an_inline_block_uses_the_line_before_a_table_as_its_baseline() {
    let buffer = draw(&format!(
        "<div><span style=\"display:inline-block\">{LINE}<table>{CELLS}</table></span>{MARKER}</div>"
    ));
    assert_eq!(pixel(&buffer, 40, 9), RED);
    assert_eq!(pixel(&buffer, 40, 13), WHITE);
    assert_eq!(pixel(&buffer, 3, 3), YELLOW);
}

#[test]
fn an_inline_block_uses_its_last_line_as_its_baseline() {
    let buffer = draw(&format!(
        "<div><span style=\"display:inline-block\">{LINE}<table>{CELLS}</table>{LINE}</span>{MARKER}</div>"
    ));
    assert_eq!(pixel(&buffer, 40, 37), RED);
    assert_eq!(pixel(&buffer, 40, 9), WHITE);
}

#[test]
fn an_inline_block_holding_an_inline_table_keeps_its_baseline() {
    let buffer = draw(&format!(
        "<div><span style=\"display:inline-block;font-size:0\">\
         <table style=\"display:inline-table\">{CELLS}{CELLS}</table></span>{MARKER}</div>"
    ));
    assert_eq!(pixel(&buffer, 40, 13), RED);
}

#[test]
fn an_inline_block_holding_an_inline_block_with_a_table_keeps_its_line() {
    let buffer = draw(&format!(
        "<div><span style=\"display:inline-block;font-size:0\">\
         <span style=\"display:inline-block\"><table>{CELLS}</table></span></span>{MARKER}</div>"
    ));
    assert_eq!(pixel(&buffer, 40, 17), RED);
}

#[test]
fn a_shrink_wrapped_line_fits_an_inline_table_by_its_baseline() {
    let buffer = draw(&format!(
        "<div style=\"position:absolute;background:blue\">\
         <table style=\"display:inline-table\">{CELLS}{CELLS}</table>{MARKER}</div>"
    ));
    assert_eq!(pixel(&buffer, 40, 33), BLUE);
    assert_eq!(pixel(&buffer, 40, 35), WHITE);
}

#[test]
fn a_hidden_child_after_the_last_line_does_not_change_the_baseline() {
    let buffer = draw(&format!(
        "<div><span style=\"display:inline-block\">{LINE}<table>{CELLS}</table>{LINE}\
         <div style=\"display:none\"></div></span>{MARKER}</div>"
    ));
    assert_eq!(pixel(&buffer, 40, 37), RED);
}

#[test]
fn a_faded_box_in_a_faded_collapsed_cell_fades_its_absolute_child_once() {
    let buffer = draw(
        "<table><col style=\"visibility:collapse\"><col><tr>\
         <td style=\"opacity:.5\"><b></b><div style=\"opacity:.5\">\
         <b class=\"r\" style=\"position:absolute\"></b></div></td>\
         <td class=\"v\"><b></b></td></tr></table>",
    );
    let [red, green, blue, _] = pixel(&buffer, 5, 20);
    assert_eq!(red, 255);
    assert!(green.abs_diff(191) <= 1 && blue.abs_diff(191) <= 1);
}
