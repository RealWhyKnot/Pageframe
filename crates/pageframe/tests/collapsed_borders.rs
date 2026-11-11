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

#[test]
fn a_table_that_clips_its_overflow_keeps_its_border() {
    let buffer = grid("", "overflow:hidden;border:6px solid blue");
    assert_eq!(pixel(&buffer, 1, 1), BLUE);
    assert_eq!(pixel(&buffer, 10, 4), BLUE);
    assert_eq!(pixel(&buffer, 10, 10), YELLOW);
}

#[test]
fn a_see_through_border_shows_the_cell_colour_under_it() {
    let buffer = grid("border:6px solid rgba(0,0,255,.5);background:red", "");
    let [red, green, blue, _] = pixel(&buffer, 10, 4);
    assert!(
        red > 100 && green == 0 && blue > 100,
        "{red} {green} {blue}"
    );
}

fn parts(table: &str, columns: &str, row: &str) -> Vec<u8> {
    draw(
        "width:20px;height:14px",
        &format!(
            "<table style=\"{table}\">{columns}\
             <tr style=\"{row}\"><td><b></b></td><td><b></b></td></tr>\
             <tr><td><b></b></td><td><b></b></td></tr></table>"
        ),
    )
}

fn line(buffer: &[u8], y: usize, xs: &[usize]) -> Vec<[u8; 4]> {
    xs.iter().map(|x| pixel(buffer, *x, y)).collect()
}

#[test]
fn a_row_border_makes_room_around_the_row() {
    let buffer = parts("", "", "border:6px solid blue");
    assert_eq!(
        line(&buffer, 10, &[0, 5, 6, 25, 26, 27, 28, 47, 48, 53, 54]),
        [
            BLUE, BLUE, YELLOW, YELLOW, BLACK, BLACK, YELLOW, YELLOW, BLUE, BLUE, WHITE
        ]
    );
    assert_eq!(pixel(&buffer, 10, 5), BLUE);
    assert_eq!(pixel(&buffer, 10, 6), YELLOW);
    assert_eq!(pixel(&buffer, 10, 19), YELLOW);
    assert_eq!(pixel(&buffer, 10, 20), BLUE);
    assert_eq!(pixel(&buffer, 10, 25), BLUE);
    assert_eq!(pixel(&buffer, 10, 26), YELLOW);
}

#[test]
fn a_row_border_in_a_right_to_left_table_sits_on_the_table_edges() {
    let buffer = parts("direction:rtl", "", "border:6px solid blue");
    assert_eq!(
        line(&buffer, 10, &[0, 5, 6, 25, 26, 27, 28, 47, 48, 53, 54]),
        [
            BLUE, BLUE, YELLOW, YELLOW, BLACK, BLACK, YELLOW, YELLOW, BLUE, BLUE, WHITE
        ]
    );
}

#[test]
fn a_column_border_makes_room_around_the_column() {
    let buffer = parts("", "<col style=\"border:6px solid blue\"><col>", "");
    assert_eq!(
        line(&buffer, 10, &[0, 5, 6, 25, 26, 31, 32, 51, 52, 53, 54]),
        [
            BLUE, BLUE, YELLOW, YELLOW, BLUE, BLUE, YELLOW, YELLOW, BLACK, BLACK, WHITE
        ]
    );
    assert_eq!(pixel(&buffer, 10, 5), BLUE);
    assert_eq!(pixel(&buffer, 10, 6), YELLOW);
}

#[test]
fn a_hidden_column_border_takes_no_room() {
    let buffer = parts("", "<col style=\"border:6px hidden blue\"><col>", "");
    assert_eq!(
        line(&buffer, 5, &[0, 19, 20, 39, 40, 41, 42]),
        [YELLOW, YELLOW, YELLOW, YELLOW, BLACK, BLACK, WHITE]
    );
    assert_eq!(pixel(&buffer, 5, 0), WHITE);
    assert_eq!(pixel(&buffer, 5, 1), LIME);
    assert_eq!(pixel(&buffer, 5, 2), YELLOW);
    assert_eq!(pixel(&buffer, 25, 0), BLACK);
}

#[test]
fn an_uneven_column_border_in_a_right_to_left_table_keeps_its_sides() {
    let buffer = parts(
        "direction:rtl",
        "<col style=\"border-left:8px solid blue\"><col>",
        "",
    );
    assert_eq!(
        line(&buffer, 10, &[0, 1, 2, 21, 22, 29, 30, 49, 50, 51, 52]),
        [
            BLACK, BLACK, YELLOW, YELLOW, BLUE, BLUE, YELLOW, YELLOW, BLACK, BLACK, WHITE
        ]
    );
}

#[test]
fn a_column_group_border_goes_around_the_group_only() {
    let buffer = parts(
        "",
        "<colgroup span=\"2\" style=\"border:6px solid blue\"></colgroup>",
        "",
    );
    assert_eq!(
        line(&buffer, 10, &[0, 5, 6, 25, 26, 27, 28, 47, 48, 53, 54]),
        [
            BLUE, BLUE, YELLOW, YELLOW, BLACK, BLACK, YELLOW, YELLOW, BLUE, BLUE, WHITE
        ]
    );
}

#[test]
fn a_row_group_border_goes_around_the_group_only() {
    let buffer = draw(
        "width:20px;height:14px",
        "<table><tbody style=\"border:6px solid blue\">\
         <tr><td><b></b></td></tr><tr><td><b></b></td></tr></tbody></table>",
    );
    let column: Vec<_> = [0, 5, 6, 19, 20, 21, 22, 35, 36, 41, 42]
        .iter()
        .map(|y| pixel(&buffer, 10, *y))
        .collect();
    assert_eq!(
        column,
        [
            BLUE, BLUE, YELLOW, YELLOW, BLACK, BLACK, YELLOW, YELLOW, BLUE, BLUE, WHITE
        ]
    );
}

#[test]
fn a_positioned_cell_keeps_its_colour_inside_a_see_through_border() {
    let buffer = grid(
        "position:relative;border:6px solid rgba(0,0,255,.5);background:red",
        "",
    );
    let [red, green, blue, _] = pixel(&buffer, 10, 4);
    assert!(
        red > 100 && green > 100 && blue == 255,
        "{red} {green} {blue}"
    );
}

#[test]
fn a_will_change_cell_keeps_its_colour_inside_a_see_through_border() {
    let buffer = grid(
        "will-change:transform;border:6px solid rgba(0,0,255,.5);background:red",
        "",
    );
    let [red, green, blue, _] = pixel(&buffer, 10, 4);
    assert!(
        red > 100 && green > 100 && blue == 255,
        "{red} {green} {blue}"
    );
}

#[test]
fn a_shifted_cell_moves_without_its_borders() {
    let buffer = grid("position:relative;left:3px;top:3px", "");
    assert_eq!(pixel(&buffer, 1, 10), BLACK);
    assert_eq!(pixel(&buffer, 3, 3), WHITE);
    assert_eq!(pixel(&buffer, 5, 5), YELLOW);
    assert_eq!(pixel(&buffer, 23, 10), YELLOW);
    assert_eq!(pixel(&buffer, 10, 17), YELLOW);
    assert_eq!(pixel(&buffer, 30, 17), BLACK);
}

#[test]
fn a_shifted_caption_moves_over_the_table() {
    let buffer = draw(
        "width:20px;height:14px",
        "<table><caption style=\"position:relative;top:8px;height:4px;background:red\">\
         </caption><tr><td><b></b></td></tr></table>",
    );
    assert_eq!(pixel(&buffer, 10, 1), WHITE);
    assert_eq!(pixel(&buffer, 10, 5), BLACK);
    assert_eq!(pixel(&buffer, 10, 7), YELLOW);
    assert_eq!(pixel(&buffer, 10, 9), RED);
    assert_eq!(pixel(&buffer, 10, 13), YELLOW);
}

#[test]
fn a_cell_in_a_positioned_row_keeps_its_colour_inside_a_see_through_border() {
    let buffer = draw(
        "width:20px;height:14px",
        "<table><tr style=\"position:relative\">\
         <td style=\"border:6px solid rgba(0,0,255,.5);background:red\"><b></b></td>\
         </tr></table>",
    );
    let [red, green, blue, _] = pixel(&buffer, 10, 4);
    assert!(
        red > 100 && green > 100 && blue == 255,
        "{red} {green} {blue}"
    );
}

const SHIFTED: &str = "<td style=\"position:relative;left:3px;top:3px\"><b></b></td>";
const PLAIN: &str = "<td><b></b></td>";

#[test]
fn a_shifted_cell_keeps_its_offset_beside_a_collapsed_column() {
    let buffer = draw(
        "width:20px;height:14px",
        &format!(
            "<table><col><col style=\"visibility:collapse\"><col>\
             <tr>{SHIFTED}{PLAIN}{PLAIN}</tr></table>"
        ),
    );
    assert_eq!(pixel(&buffer, 3, 3), WHITE);
    assert_eq!(pixel(&buffer, 5, 5), YELLOW);
}

#[test]
fn a_shifted_cell_keeps_its_offset_above_a_collapsed_row() {
    let buffer = draw(
        "width:20px;height:14px",
        &format!(
            "<table><tr>{SHIFTED}</tr>\
             <tr style=\"visibility:collapse\">{PLAIN}</tr><tr>{PLAIN}</tr></table>"
        ),
    );
    assert_eq!(pixel(&buffer, 3, 3), WHITE);
    assert_eq!(pixel(&buffer, 5, 5), YELLOW);
}

#[test]
fn a_caption_in_a_right_to_left_table_follows_right_over_left() {
    let buffer = draw(
        "width:20px;height:14px",
        &format!(
            "<table style=\"direction:rtl;margin-left:20px\">\
             <caption style=\"position:relative;left:5px;right:10px;height:4px;\
             background:red\"></caption><tr>{PLAIN}</tr></table>"
        ),
    );
    assert_eq!(pixel(&buffer, 12, 2), RED);
    assert_eq!(pixel(&buffer, 40, 2), WHITE);
}

#[test]
fn a_percent_offset_on_a_cell_uses_the_row_width_only() {
    let buffer = draw(
        "width:20px;height:14px",
        &format!(
            "<table><tr><td style=\"position:relative;left:50%;top:50%\"><b></b></td>\
             {PLAIN}</tr><tr>{PLAIN}{PLAIN}</tr></table>"
        ),
    );
    assert_eq!(pixel(&buffer, 10, 10), WHITE);
    assert_eq!(pixel(&buffer, 30, 10), YELLOW);
    assert_eq!(pixel(&buffer, 45, 10), BLACK);
    assert_eq!(pixel(&buffer, 30, 16), BLACK);
}

#[test]
fn a_percent_offset_on_a_caption_uses_the_table_content_width() {
    let buffer = draw(
        "width:20px;height:14px",
        &format!(
            "<table><caption style=\"position:relative;left:50%;top:50%;height:4px;\
             background:red\"></caption><tr>{PLAIN}</tr></table>"
        ),
    );
    assert_eq!(pixel(&buffer, 10, 2), WHITE);
    assert_eq!(pixel(&buffer, 11, 2), RED);
    assert_eq!(pixel(&buffer, 34, 2), RED);
    assert_eq!(pixel(&buffer, 35, 2), WHITE);
}

#[test]
fn a_row_colour_under_a_shifted_cell_stays_inside_its_borders() {
    let buffer = draw(
        "width:20px;height:14px",
        "<table><tr style=\"background:red\">\
         <td style=\"position:relative;left:3px;top:3px;background:none;padding:3px\">\
         <b></b></td></tr></table>",
    );
    assert_eq!(line(&buffer, 10, &[3, 4, 5]), [WHITE, WHITE, RED]);
    assert_eq!(pixel(&buffer, 10, 4), WHITE);
    assert_eq!(pixel(&buffer, 10, 5), RED);
    assert_eq!(pixel(&buffer, 8, 8), YELLOW);
}

#[test]
fn a_shifted_row_moves_its_cells_over_the_borders() {
    let buffer = draw(
        "width:20px;height:14px",
        &format!(
            "<table><tr style=\"position:relative;top:3px;left:5px\">{PLAIN}{PLAIN}</tr>\
             <tr>{PLAIN}{PLAIN}</tr></table>"
        ),
    );
    assert_eq!(
        line(&buffer, 10, &[1, 3, 6, 7, 23]),
        [BLACK, WHITE, WHITE, YELLOW, YELLOW]
    );
    assert_eq!(line(&buffer, 3, &[10, 23]), [WHITE, BLACK]);
    assert_eq!(pixel(&buffer, 10, 17), YELLOW);
    assert_eq!(pixel(&buffer, 3, 17), BLACK);
}

#[test]
fn a_faded_row_fades_its_cells_and_its_own_colour_together() {
    let buffer = draw(
        "width:20px;height:14px",
        "<table><tr style=\"opacity:.5;background:red\">\
         <td style=\"background:none;padding:3px\"><b></b></td>\
         <td><b></b></td></tr></table>",
    );
    let is_half = |pixel: [u8; 4], full: [u8; 4]| {
        pixel
            .iter()
            .zip(full)
            .all(|(got, full)| got.abs_diff(u8::midpoint(full, 255)) <= 1)
    };
    assert!(is_half(pixel(&buffer, 3, 3), RED));
    assert!(is_half(pixel(&buffer, 10, 10), YELLOW));
    assert!(is_half(pixel(&buffer, 40, 10), YELLOW));
    assert_eq!(pixel(&buffer, 0, 0), BLACK);
    assert_eq!(pixel(&buffer, 29, 10), BLACK);
}

#[test]
fn a_shifted_row_group_takes_its_borders_along() {
    let buffer = draw(
        "width:20px;height:14px",
        &format!(
            "<table><tbody style=\"position:relative;top:3px;left:5px\">\
             <tr>{PLAIN}{PLAIN}</tr></tbody>\
             <tbody><tr>{PLAIN}{PLAIN}</tr></tbody></table>"
        ),
    );
    assert_eq!(pixel(&buffer, 1, 1), WHITE);
    assert_eq!(pixel(&buffer, 6, 4), BLACK);
    assert_eq!(pixel(&buffer, 48, 20), BLACK);
    assert_eq!(pixel(&buffer, 1, 17), WHITE);
    assert_eq!(pixel(&buffer, 1, 20), BLACK);
    assert_eq!(pixel(&buffer, 10, 10), YELLOW);
}

#[test]
fn a_shifted_row_paints_its_colour_over_the_borders() {
    let buffer = draw(
        "width:20px;height:14px",
        "<table><tr style=\"position:relative;left:5px;background:red\">\
         <td style=\"background:none;padding:3px\"><b></b></td>\
         <td><b></b></td></tr></table>",
    );
    assert_eq!(line(&buffer, 10, &[6, 7, 9, 10]), [WHITE, RED, RED, YELLOW]);
    assert_eq!(pixel(&buffer, 29, 3), RED);
    assert_eq!(pixel(&buffer, 29, 1), BLACK);
}
