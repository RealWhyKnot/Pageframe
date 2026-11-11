mod support;

use pageframe::{App, Canvas, Color, LineCap, Page};
use pageframe_test_harness::Harness;
use support::{open, pixel, render};

const WHITE: [u8; 4] = [255, 255, 255, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 128, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

const PAGE: &str = r#"<html><head><style>
body{margin:0;background:white}
canvas{display:block}
#wide{width:100px;height:40px}
</style></head><body>
<canvas id="c" width="100" height="60"></canvas>
</body></html>"#;

const SCALED: &str = r#"<html><head><style>
body{margin:0;background:white}
canvas{display:block;width:100px;height:40px}
</style></head><body>
<canvas id="c" width="50" height="20"></canvas>
</body></html>"#;

fn page(html: &str) -> Harness<Page<()>> {
    open(App::new((), html))
}

fn drawn(html: &str, paint: impl Fn(&mut Canvas)) -> Vec<u8> {
    let mut harness = page(html);
    harness.doc.ui().select("#c").draw(paint);
    harness.pump();
    render(&mut harness)
}

#[test]
fn an_undrawn_canvas_shows_the_page_behind_it() {
    let mut harness = page(PAGE);
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 20, 20), WHITE);
}

#[test]
fn fill_rect_paints_canvas_pixels() {
    let buffer = drawn(PAGE, |canvas| {
        canvas
            .set_fill(Color::rgb(255, 0, 0))
            .fill_rect(10.0, 10.0, 30.0, 20.0);
    });
    assert_eq!(pixel(&buffer, 20, 20), RED);
    assert_eq!(pixel(&buffer, 5, 20), WHITE);
    assert_eq!(pixel(&buffer, 45, 20), WHITE);
    assert_eq!(pixel(&buffer, 20, 35), WHITE);
}

#[test]
fn drawing_scales_from_the_bitmap_to_the_css_box() {
    let buffer = drawn(SCALED, |canvas| {
        canvas
            .set_fill(Color::rgb(0, 0, 255))
            .fill_rect(10.0, 5.0, 20.0, 10.0);
    });
    assert_eq!(pixel(&buffer, 40, 20), BLUE);
    assert_eq!(pixel(&buffer, 15, 20), WHITE);
    assert_eq!(pixel(&buffer, 65, 20), WHITE);
    assert_eq!(pixel(&buffer, 40, 5), WHITE);
    assert_eq!(pixel(&buffer, 40, 35), WHITE);
}

#[test]
fn drawing_outside_the_bitmap_is_clipped() {
    let buffer = drawn(PAGE, |canvas| {
        canvas
            .set_fill(Color::rgb(255, 0, 0))
            .fill_rect(80.0, 40.0, 100.0, 100.0);
    });
    assert_eq!(pixel(&buffer, 90, 50), RED);
    assert_eq!(pixel(&buffer, 110, 50), WHITE);
    assert_eq!(pixel(&buffer, 90, 70), WHITE);
}

#[test]
fn a_filled_path_covers_its_inside_only() {
    let buffer = drawn(PAGE, |canvas| {
        canvas
            .set_fill(Color::rgb(0, 128, 0))
            .begin_path()
            .move_to(10.0, 50.0)
            .line_to(50.0, 50.0)
            .line_to(10.0, 10.0)
            .close_path()
            .fill();
    });
    assert_eq!(pixel(&buffer, 15, 40), GREEN);
    assert_eq!(pixel(&buffer, 45, 15), WHITE);
}

#[test]
fn a_stroke_is_as_wide_as_the_line_width() {
    let buffer = drawn(PAGE, |canvas| {
        canvas
            .set_stroke(Color::rgb(255, 0, 0))
            .set_line_width(10.0)
            .begin_path()
            .move_to(10.0, 30.0)
            .line_to(90.0, 30.0)
            .stroke();
    });
    assert_eq!(pixel(&buffer, 50, 26), RED);
    assert_eq!(pixel(&buffer, 50, 33), RED);
    assert_eq!(pixel(&buffer, 50, 22), WHITE);
    assert_eq!(pixel(&buffer, 50, 37), WHITE);
    assert_eq!(pixel(&buffer, 5, 30), WHITE);
}

#[test]
fn a_square_cap_extends_the_line_by_half_its_width() {
    let buffer = drawn(PAGE, |canvas| {
        canvas
            .set_stroke(Color::rgb(255, 0, 0))
            .set_line_width(10.0)
            .set_line_cap(LineCap::Square)
            .begin_path()
            .move_to(20.0, 30.0)
            .line_to(80.0, 30.0)
            .stroke();
    });
    assert_eq!(pixel(&buffer, 17, 30), RED);
    assert_eq!(pixel(&buffer, 12, 30), WHITE);
}

#[test]
fn transforms_move_and_scale_what_is_drawn_after_them() {
    let buffer = drawn(PAGE, |canvas| {
        canvas
            .set_fill(Color::rgb(0, 0, 255))
            .translate(40.0, 20.0)
            .scale(2.0, 2.0)
            .fill_rect(0.0, 0.0, 10.0, 10.0);
    });
    assert_eq!(pixel(&buffer, 50, 30), BLUE);
    assert_eq!(pixel(&buffer, 58, 38), BLUE);
    assert_eq!(pixel(&buffer, 35, 30), WHITE);
    assert_eq!(pixel(&buffer, 65, 30), WHITE);
}

#[test]
fn restore_brings_back_the_saved_transform_and_colour() {
    let buffer = drawn(PAGE, |canvas| {
        canvas
            .set_fill(Color::rgb(255, 0, 0))
            .save()
            .translate(50.0, 0.0)
            .set_fill(Color::rgb(0, 0, 255))
            .fill_rect(0.0, 0.0, 10.0, 10.0)
            .restore()
            .fill_rect(0.0, 0.0, 10.0, 10.0);
    });
    assert_eq!(pixel(&buffer, 55, 5), BLUE);
    assert_eq!(pixel(&buffer, 5, 5), RED);
}

#[test]
fn a_clip_limits_later_drawing_until_restore() {
    let buffer = drawn(PAGE, |canvas| {
        canvas
            .set_fill(Color::rgb(255, 0, 0))
            .save()
            .begin_path()
            .rect(0.0, 0.0, 20.0, 20.0)
            .clip()
            .fill_rect(0.0, 0.0, 100.0, 60.0)
            .restore()
            .set_fill(Color::rgb(0, 0, 255))
            .fill_rect(60.0, 40.0, 10.0, 10.0);
    });
    assert_eq!(pixel(&buffer, 10, 10), RED);
    assert_eq!(pixel(&buffer, 30, 10), WHITE);
    assert_eq!(pixel(&buffer, 65, 45), BLUE);
}

#[test]
fn a_full_arc_fills_a_circle() {
    let buffer = drawn(PAGE, |canvas| {
        canvas
            .set_fill(Color::rgb(0, 128, 0))
            .begin_path()
            .arc((50.0, 30.0), 20.0, 0.0, std::f64::consts::TAU)
            .fill();
    });
    assert_eq!(pixel(&buffer, 50, 30), GREEN);
    assert_eq!(pixel(&buffer, 50, 12), GREEN);
    assert_eq!(pixel(&buffer, 33, 13), WHITE);
    assert_eq!(pixel(&buffer, 50, 55), WHITE);
}

#[test]
fn alpha_blends_the_fill_with_the_page() {
    let buffer = drawn(PAGE, |canvas| {
        canvas
            .set_fill(Color::rgb(0, 0, 0))
            .set_alpha(0.5)
            .fill_rect(0.0, 0.0, 20.0, 20.0);
    });
    let [r, g, b, _] = pixel(&buffer, 10, 10);
    assert!(
        (126..=129).contains(&r) && r == g && g == b,
        "got {r} {g} {b}"
    );
}

#[test]
fn drawing_again_replaces_the_old_picture() {
    let mut harness = page(PAGE);
    harness.doc.ui().select("#c").draw(|canvas| {
        canvas
            .set_fill(Color::rgb(255, 0, 0))
            .fill_rect(0.0, 0.0, 100.0, 60.0);
    });
    harness.pump();
    harness.doc.ui().select("#c").draw(|canvas| {
        canvas
            .set_fill(Color::rgb(0, 0, 255))
            .fill_rect(0.0, 0.0, 10.0, 10.0);
    });
    harness.pump();
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 5, 5), BLUE);
    assert_eq!(pixel(&buffer, 50, 30), WHITE);
}

#[test]
fn draw_skips_elements_that_are_not_canvases() {
    let mut harness = page(
        r#"<html><body style="margin:0;background:white"><div id="c" style="width:50px;height:50px"></div></body></html>"#,
    );
    harness.doc.ui().select("#c").draw(|canvas| {
        canvas
            .set_fill(Color::rgb(255, 0, 0))
            .fill_rect(0.0, 0.0, 50.0, 50.0);
    });
    harness.pump();
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 10, 10), WHITE);
}

#[test]
fn hostile_numbers_do_not_break_the_canvas() {
    let buffer = drawn(PAGE, |canvas| {
        canvas
            .set_line_width(f64::NAN)
            .set_line_width(-4.0)
            .set_alpha(f32::INFINITY)
            .scale(f64::INFINITY, 1.0)
            .arc((f64::NAN, 0.0), -1.0, 0.0, 1.0)
            .restore()
            .set_fill(Color::rgb(255, 0, 0))
            .fill_rect(0.0, 0.0, 1.0e12, 1.0e12);
    });
    assert_eq!(pixel(&buffer, 50, 30), RED);
}

#[test]
fn hex_colours_parse_in_every_length() {
    assert_eq!(Color::hex("#f00"), Some(Color::rgb(255, 0, 0)));
    assert_eq!(Color::hex("#f008"), Some(Color::rgba(255, 0, 0, 136)));
    assert_eq!(Color::hex("#0000ff"), Some(Color::rgb(0, 0, 255)));
    assert_eq!(Color::hex("#0000ff80"), Some(Color::rgba(0, 0, 255, 128)));
    assert_eq!(Color::hex("0000ff"), None);
    assert_eq!(Color::hex("#12"), None);
    assert_eq!(Color::hex("#ggg"), None);
    assert_eq!(Color::hex("#ééé"), None);
}
