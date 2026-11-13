use super::*;

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
