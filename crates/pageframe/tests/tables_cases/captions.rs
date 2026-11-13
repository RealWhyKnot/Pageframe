use super::*;

#[test]
fn a_collapsed_row_colours_the_seam_between_its_neighbours() {
    let buffer = seam_table("", "", "");
    assert_eq!(
        column(&buffer, 5, &[11, 12, 13, 14]),
        [GREEN, RED, RED, GREEN]
    );
    assert_eq!(row(&buffer, &[0, 9], 13), [RED, RED]);
}

#[test]
fn a_wide_border_above_a_collapsed_row_stops_at_the_row_below() {
    let buffer = seam_table("border-bottom:6px solid black", "", "");
    assert_eq!(
        column(&buffer, 5, &[11, 12, 13, 14, 15, 16]),
        [GREEN, BLACK, BLACK, RED, RED, BLACK]
    );
    assert_eq!(column(&buffer, 5, &[17, 18]), [BLACK, GREEN]);
}

#[test]
fn a_wide_border_below_a_collapsed_row_stops_at_the_row_above() {
    let buffer = seam_table("", "", "border-top:6px solid black");
    assert_eq!(
        column(&buffer, 5, &[9, 10, 15, 16]),
        [GREEN, BLACK, BLACK, GREEN]
    );
}

#[test]
fn a_wide_border_on_a_collapsed_row_fills_the_seam() {
    let buffer = seam_table("", "border-width:6px", "");
    assert_eq!(
        column(&buffer, 5, &[11, 12, 17, 18]),
        [GREEN, RED, RED, GREEN]
    );
}

#[test]
fn a_table_min_width_holds_when_content_overflows_its_width() {
    check_table_boxes(
        &format!(
            r#"{SPAN_STYLE}<div><table id="a" style="width:50px;min-width:200px"><tr><td id="a1"><b style="width:100px"></b></td></tr></table></div>"#
        ),
        &[
            ("a", [0.0, 0.0, 200.0, 18.0]),
            ("a1", [2.0, 2.0, 196.0, 14.0]),
        ],
    );
    check_table_boxes(
        &format!(
            r#"{SPAN_STYLE}<div><table id="b" style="width:50px;min-width:80px"><tr><td id="b1"><b style="width:100px"></b></td></tr></table></div>"#
        ),
        &[
            ("b", [0.0, 0.0, 108.0, 18.0]),
            ("b1", [2.0, 2.0, 104.0, 14.0]),
        ],
    );
    check_table_boxes(
        &format!(
            r#"{SPAN_STYLE}<div><table id="f" style="width:50px;min-width:200px;box-sizing:border-box;border:5px solid blue"><tr><td id="f1"><b style="width:100px"></b></td></tr></table></div>"#
        ),
        &[
            ("f", [0.0, 0.0, 200.0, 28.0]),
            ("f1", [7.0, 7.0, 186.0, 14.0]),
        ],
    );
}

#[test]
fn a_table_min_width_stretches_its_columns() {
    check_table_boxes(
        &format!(
            r#"{SPAN_STYLE}<div><table id="g" style="min-width:200px"><tr><td id="g1"><b style="width:40px"></b></td><td id="g2"><b style="width:80px"></b></td></tr></table></div>"#
        ),
        &[
            ("g", [0.0, 0.0, 200.0, 18.0]),
            ("g1", [2.0, 2.0, 66.6875, 14.0]),
            ("g2", [70.6875, 2.0, 127.3125, 14.0]),
        ],
    );
}

#[test]
fn a_table_max_width_wraps_content_in_a_table_without_a_width() {
    check_table_boxes(
        &format!(
            r#"{SPAN_STYLE}<div style="width:500px"><table id="c" style="max-width:60px"><tr><td id="c1"><b style="width:40px"></b><b style="width:40px"></b><b style="width:40px"></b></td></tr></table></div>"#
        ),
        &[
            ("c", [0.0, 0.0, 60.0, 38.0]),
            ("c1", [2.0, 2.0, 56.0, 34.0]),
        ],
    );
    check_table_boxes(
        &format!(
            r#"{SPAN_STYLE}<div style="width:500px"><table id="e" style="max-width:60px"><tr><td id="e1"><b style="width:40px"></b><b style="width:40px"></b></td><td id="e2"><b style="width:40px"></b><b style="width:40px"></b></td></tr></table></div>"#
        ),
        &[
            ("e", [0.0, 0.0, 94.0, 28.0]),
            ("e1", [2.0, 2.0, 44.0, 24.0]),
            ("e2", [48.0, 2.0, 44.0, 24.0]),
        ],
    );
    check_table_boxes(
        &format!(
            r#"{SPAN_STYLE}<div style="width:500px"><table id="h" style="max-width:60px;min-width:90px"><tr><td id="h1"><b style="width:40px"></b><b style="width:40px"></b><b style="width:40px"></b></td></tr></table></div>"#
        ),
        &[
            ("h", [0.0, 0.0, 90.0, 28.0]),
            ("h1", [2.0, 2.0, 86.0, 24.0]),
        ],
    );
}

#[test]
fn a_caption_margin_insets_the_caption_and_adds_height() {
    check_caption_boxes(
        r#"<table id="a"><caption id="ac" style="margin:5px"><b></b></caption><tr><td id="a1"><b></b><b></b></td></tr></table>"#,
        &[
            ("a", [0.0, 0.0, 88.0, 38.0]),
            ("ac", [5.0, 5.0, 78.0, 10.0]),
            ("a1", [4.0, 24.0, 80.0, 10.0]),
        ],
    );
    check_caption_boxes(
        r#"<table id="d"><caption id="dc" style="caption-side:bottom;margin:7px 3px"><b></b></caption><tr><td id="d1"><b></b><b></b></td></tr></table>"#,
        &[
            ("d", [0.0, 0.0, 88.0, 42.0]),
            ("dc", [3.0, 25.0, 82.0, 10.0]),
            ("d1", [4.0, 4.0, 80.0, 10.0]),
        ],
    );
    check_caption_boxes(
        r#"<table id="h"><caption id="hc" style="margin:-5px"><b></b></caption><tr><td id="h1"><b></b><b></b></td></tr></table>"#,
        &[
            ("h", [0.0, 0.0, 88.0, 18.0]),
            ("hc", [-5.0, -5.0, 98.0, 10.0]),
            ("h1", [4.0, 4.0, 80.0, 10.0]),
        ],
    );
}

#[test]
fn auto_caption_margins_place_a_sized_caption() {
    check_caption_boxes(
        r#"<table id="f"><caption id="fc" style="margin:0 auto;width:50px"><b></b></caption><tr><td id="f1"><b></b><b></b></td></tr></table>"#,
        &[
            ("f", [0.0, 0.0, 88.0, 28.0]),
            ("fc", [19.0, 0.0, 50.0, 10.0]),
        ],
    );
    check_caption_boxes(
        r#"<table id="c"><caption id="cc" style="margin-left:30px;margin-right:auto"><b></b></caption><tr><td id="c1"><b></b><b></b></td></tr></table>"#,
        &[
            ("c", [0.0, 0.0, 88.0, 28.0]),
            ("cc", [30.0, 0.0, 58.0, 10.0]),
        ],
    );
    check_caption_boxes(
        r#"<table id="o"><caption id="oc" style="margin:5px;box-sizing:border-box;width:60px;padding:5px"><b></b></caption><tr><td id="o1"><b></b><b></b></td></tr></table>"#,
        &[
            ("o", [0.0, 0.0, 88.0, 48.0]),
            ("oc", [5.0, 5.0, 60.0, 20.0]),
            ("o1", [4.0, 34.0, 80.0, 10.0]),
        ],
    );
}

#[test]
fn a_caption_width_and_margin_widen_the_table() {
    check_caption_boxes(
        r#"<table id="g"><caption id="gc" style="margin:5px;width:120px"><b></b></caption><tr><td id="g1"><b></b></td></tr></table>"#,
        &[
            ("g", [0.0, 0.0, 130.0, 38.0]),
            ("gc", [5.0, 5.0, 120.0, 10.0]),
            ("g1", [4.0, 24.0, 122.0, 10.0]),
        ],
    );
}

#[test]
fn margins_of_stacked_captions_add_up() {
    check_caption_boxes(
        r#"<table id="i"><caption id="ic" style="margin:5px"><b></b></caption><caption id="ic2" style="margin:9px"><b></b></caption><tr><td id="i1"><b></b><b></b></td></tr></table>"#,
        &[
            ("i", [0.0, 0.0, 88.0, 66.0]),
            ("ic", [5.0, 5.0, 78.0, 10.0]),
            ("ic2", [9.0, 29.0, 70.0, 10.0]),
            ("i1", [4.0, 52.0, 80.0, 10.0]),
        ],
    );
    check_caption_boxes(
        r#"<table id="q"><caption id="qc" style="caption-side:bottom;margin:5px"><b></b></caption><caption id="qc2" style="margin:3px"><b></b></caption><tr><td id="q1"><b></b><b></b></td></tr></table>"#,
        &[
            ("q", [0.0, 0.0, 88.0, 54.0]),
            ("qc", [5.0, 39.0, 78.0, 10.0]),
            ("qc2", [3.0, 3.0, 82.0, 10.0]),
            ("q1", [4.0, 20.0, 80.0, 10.0]),
        ],
    );
}

#[test]
fn the_table_frame_starts_below_overlapping_captions() {
    let buffer = draw(&format!(
        "{CAPTION_PAINT_STYLE}<table><caption style=\"height:20px\"></caption><caption style=\"margin-top:-15px\"><b></b></caption><tr><td><b></b><b></b></td></tr></table>"
    ));
    assert_eq!(
        column(&buffer, 30, &[0, 14, 15, 18, 19, 28, 29, 30, 31, 32, 33]),
        [
            BLUE, BLUE, BLUE, BLUE, GREEN, GREEN, WHITE, WHITE, BLACK, BLACK, WHITE
        ]
    );
    assert_eq!(
        column(&buffer, 0, &[15, 19, 20, 32, 33]),
        [BLUE, BLUE, BLACK, BLACK, WHITE]
    );
}

#[test]
fn cells_paint_over_a_caption_pulled_onto_the_table() {
    let buffer = draw(&format!(
        "{CAPTION_PAINT_STYLE}<table><caption style=\"margin:-5px\"><b></b></caption><tr><td><b></b><b></b></td></tr></table>"
    ));
    assert_eq!(
        column(&buffer, 50, &[0, 3, 4, 13, 14, 15, 16, 17, 18]),
        [BLUE, BLUE, GREEN, GREEN, WHITE, WHITE, BLACK, BLACK, WHITE]
    );
    assert_eq!(
        row(&buffer, &[0, 1, 2, 3, 4], 4),
        [BLUE, BLUE, BLUE, BLUE, GREEN]
    );
    assert_eq!(row(&buffer, &[0, 1, 2], 8), [BLACK, BLACK, WHITE]);
}

#[test]
fn cells_paint_over_a_bottom_caption_pulled_onto_the_table() {
    let buffer = draw(&format!(
        "{CAPTION_PAINT_STYLE}<table><caption style=\"caption-side:bottom;margin:-5px\"><b></b></caption><tr><td><b></b><b></b></td></tr></table>"
    ));
    assert_eq!(
        column(&buffer, 50, &[0, 1, 3, 4, 13, 14, 16, 22, 23]),
        [BLACK, BLACK, WHITE, GREEN, GREEN, BLUE, BLUE, BLUE, WHITE]
    );
    assert_eq!(column(&buffer, 0, &[8, 14]), [BLACK, BLUE]);
}
