mod support;

use pageframe::{App, Canvas, Color, Gradient};
use support::{open, pixel, render};

const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];
const RED: Color = Color::rgb(255, 0, 0);
const BLUE: Color = Color::rgb(0, 0, 255);

const PAGE: &str = r#"<html><head><style>
body{margin:0;background:white}
canvas{display:block}
</style></head><body>
<canvas id="c" width="100" height="60"></canvas>
</body></html>"#;

fn drawn(paint: impl Fn(&mut Canvas)) -> Vec<u8> {
    let mut harness = open(App::new((), PAGE));
    harness.doc.ui().select("#c").draw(paint);
    harness.pump();
    render(&mut harness)
}

fn near(actual: [u8; 4], wanted: [u8; 3]) -> bool {
    actual[..3]
        .iter()
        .zip(wanted)
        .all(|(&channel, target)| channel.abs_diff(target) <= 3)
}

fn across() -> Gradient {
    Gradient::linear((0.0, 0.0), (100.0, 0.0))
}

fn black_to_white(from: f64, to: f64) -> Gradient {
    Gradient::linear((from, 0.0), (to, 0.0))
        .stop(0.0, Color::BLACK)
        .stop(1.0, Color::WHITE)
}

#[test]
fn a_linear_gradient_blends_between_its_stops() {
    let buffer = drawn(|canvas| {
        canvas
            .set_fill_gradient(&across().stop(0.0, RED).stop(1.0, BLUE))
            .fill_rect(0.0, 0.0, 100.0, 60.0);
    });
    assert!(near(pixel(&buffer, 0, 30), [254, 0, 1]));
    assert!(near(pixel(&buffer, 50, 30), [126, 0, 129]));
    assert!(near(pixel(&buffer, 99, 5), [1, 0, 254]));
}

#[test]
fn colour_holds_past_the_first_and_last_stop() {
    let buffer = drawn(|canvas| {
        canvas
            .set_fill_gradient(&black_to_white(40.0, 60.0))
            .fill_rect(0.0, 0.0, 100.0, 60.0);
    });
    assert_eq!(pixel(&buffer, 10, 30), BLACK);
    assert_eq!(pixel(&buffer, 90, 30), WHITE);
    assert!(near(pixel(&buffer, 50, 30), [134, 134, 134]));
}

#[test]
fn stops_are_ordered_by_offset_whatever_order_they_were_added_in() {
    let green = Color::rgb(0, 128, 0);
    let ordered = drawn(|canvas| {
        let gradient = across().stop(0.0, RED).stop(0.5, green).stop(1.0, BLUE);
        canvas
            .set_fill_gradient(&gradient)
            .fill_rect(0.0, 0.0, 100.0, 60.0);
    });
    let shuffled = drawn(|canvas| {
        let gradient = across().stop(1.0, BLUE).stop(0.0, RED).stop(0.5, green);
        canvas
            .set_fill_gradient(&gradient)
            .fill_rect(0.0, 0.0, 100.0, 60.0);
    });
    assert_eq!(ordered, shuffled);
    assert!(near(pixel(&ordered, 50, 30), [3, 127, 3]));
}

#[test]
fn one_stop_is_a_solid_colour_and_no_stops_paint_nothing() {
    let buffer = drawn(|canvas| {
        canvas
            .set_fill_gradient(&across().stop(0.3, RED))
            .fill_rect(0.0, 0.0, 50.0, 60.0)
            .set_fill_gradient(&across())
            .fill_rect(50.0, 0.0, 50.0, 60.0);
    });
    assert_eq!(pixel(&buffer, 5, 30), [255, 0, 0, 255]);
    assert_eq!(pixel(&buffer, 45, 30), [255, 0, 0, 255]);
    assert_eq!(pixel(&buffer, 75, 30), WHITE);
}

#[test]
fn bad_gradients_and_bad_stops_paint_nothing() {
    let buffer = drawn(|canvas| {
        let point = Gradient::linear((10.0, 10.0), (10.0, 10.0))
            .stop(0.0, RED)
            .stop(1.0, BLUE);
        let endless = Gradient::linear((0.0, f64::NAN), (100.0, 0.0))
            .stop(0.0, RED)
            .stop(1.0, BLUE);
        let same = Gradient::radial((50.0, 30.0), 10.0, (50.0, 30.0), 10.0)
            .stop(0.0, RED)
            .stop(1.0, BLUE);
        let inside_out = Gradient::radial((50.0, 30.0), -1.0, (50.0, 30.0), 10.0)
            .stop(0.0, RED)
            .stop(1.0, BLUE);
        let stray = across().stop(-0.1, RED).stop(1.1, BLUE).stop(f32::NAN, RED);
        for gradient in [point, endless, same, inside_out, stray] {
            canvas
                .set_fill_gradient(&gradient)
                .fill_rect(0.0, 0.0, 100.0, 60.0)
                .set_stroke_gradient(&gradient)
                .set_line_width(10.0)
                .stroke_rect(20.0, 20.0, 60.0, 20.0);
        }
    });
    assert_eq!(pixel(&buffer, 50, 30), WHITE);
    assert_eq!(pixel(&buffer, 20, 30), WHITE);
}

#[test]
fn a_radial_gradient_runs_from_the_inner_circle_to_the_outer() {
    let buffer = drawn(|canvas| {
        let gradient = Gradient::radial((50.0, 30.0), 5.0, (50.0, 30.0), 25.0)
            .stop(0.0, Color::WHITE)
            .stop(1.0, RED);
        canvas
            .set_fill_gradient(&gradient)
            .fill_rect(0.0, 0.0, 100.0, 60.0);
    });
    assert_eq!(pixel(&buffer, 50, 30), WHITE);
    assert_eq!(pixel(&buffer, 52, 31), WHITE);
    assert!(near(pixel(&buffer, 65, 30), [255, 122, 122]));
    assert_eq!(pixel(&buffer, 90, 30), [255, 0, 0, 255]);
    assert_eq!(pixel(&buffer, 50, 58), [255, 0, 0, 255]);
}

#[test]
fn a_see_through_stop_keeps_its_own_colour_while_fading() {
    let buffer = drawn(|canvas| {
        let gradient = across().stop(0.0, RED).stop(1.0, Color::rgba(0, 0, 255, 0));
        canvas
            .set_fill_gradient(&gradient)
            .fill_rect(0.0, 0.0, 100.0, 60.0);
    });
    assert!(near(pixel(&buffer, 50, 30), [191, 128, 192]));
    assert!(near(pixel(&buffer, 99, 30), [254, 253, 255]));
}

#[test]
fn a_gradient_follows_the_transform_for_rects_paths_and_strokes() {
    let buffer = drawn(|canvas| {
        canvas
            .translate(50.0, 0.0)
            .set_fill_gradient(&black_to_white(0.0, 50.0))
            .fill_rect(0.0, 0.0, 50.0, 20.0)
            .begin_path()
            .rect(0.0, 20.0, 50.0, 20.0)
            .fill()
            .set_stroke_gradient(&black_to_white(0.0, 50.0))
            .set_line_width(10.0)
            .begin_path()
            .move_to(0.0, 50.0)
            .line_to(50.0, 50.0)
            .stroke();
    });
    for row in [10, 30, 50] {
        assert_eq!(pixel(&buffer, 40, row), WHITE);
        assert!(near(pixel(&buffer, 50, row), [3, 3, 3]));
        assert!(near(pixel(&buffer, 75, row), [130, 130, 130]));
        assert!(near(pixel(&buffer, 99, row), [252, 252, 252]));
    }
}

#[test]
fn alpha_fades_a_gradient_and_a_plain_colour_replaces_it() {
    let buffer = drawn(|canvas| {
        let dark = across().stop(0.0, Color::BLACK).stop(1.0, Color::BLACK);
        canvas
            .save()
            .set_alpha(0.5)
            .set_fill_gradient(&dark)
            .fill_rect(0.0, 0.0, 30.0, 60.0)
            .restore()
            .fill_rect(30.0, 0.0, 30.0, 60.0)
            .set_fill_gradient(&dark)
            .set_fill(RED)
            .fill_rect(60.0, 0.0, 40.0, 60.0);
    });
    assert!(near(pixel(&buffer, 15, 30), [128, 128, 128]));
    assert_eq!(pixel(&buffer, 45, 30), BLACK);
    assert_eq!(pixel(&buffer, 80, 30), [255, 0, 0, 255]);
}

#[test]
fn oversized_gradients_still_paint() {
    let buffer = drawn(|canvas| {
        let crowded = (0..5_000_u16).fold(across(), |gradient, index| {
            let offset = f32::from(index) / 5_000.0;
            gradient.stop(offset, if index % 2 == 0 { RED } else { BLUE })
        });
        let vast = Gradient::radial((50.0, 30.0), 0.0, (50.0, 30.0), 1e300)
            .stop(0.0, RED)
            .stop(1.0, BLUE);
        canvas
            .set_fill_gradient(&crowded)
            .fill_rect(0.0, 0.0, 100.0, 30.0)
            .set_fill_gradient(&vast)
            .fill_rect(0.0, 30.0, 100.0, 30.0);
    });
    assert_ne!(pixel(&buffer, 80, 10), WHITE);
    assert_eq!(pixel(&buffer, 50, 45), [255, 0, 0, 255]);
}
