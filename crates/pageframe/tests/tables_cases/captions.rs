use super::*;

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
