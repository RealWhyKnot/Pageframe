mod support;

use std::f64::consts::{PI, TAU};

use pageframe::{App, Canvas, Color, LineCap};
use support::{open, pixel, render};

const WHITE: [u8; 4] = [255, 255, 255, 255];
const RED: [u8; 4] = [255, 0, 0, 255];

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

fn red_line(canvas: &mut Canvas) {
    canvas
        .set_stroke(Color::rgb(255, 0, 0))
        .set_line_width(4.0)
        .begin_path()
        .move_to(0.0, 30.0)
        .line_to(100.0, 30.0)
        .stroke();
}

fn nested_rects(canvas: &mut Canvas) {
    canvas
        .begin_path()
        .rect(10.0, 10.0, 80.0, 40.0)
        .rect(30.0, 20.0, 40.0, 20.0);
}

#[test]
fn a_counterclockwise_arc_takes_the_other_way_round() {
    let half = |canvas: &mut Canvas| {
        canvas
            .set_stroke(Color::rgb(255, 0, 0))
            .set_line_width(6.0)
            .begin_path();
    };
    let buffer = drawn(|canvas| {
        half(canvas);
        canvas
            .arc_counterclockwise((50.0, 30.0), 20.0, 0.0, PI)
            .stroke();
    });
    assert_eq!(pixel(&buffer, 50, 10), RED);
    assert_eq!(pixel(&buffer, 50, 50), WHITE);
    let buffer = drawn(|canvas| {
        half(canvas);
        canvas.arc((50.0, 30.0), 20.0, 0.0, PI).stroke();
    });
    assert_eq!(pixel(&buffer, 50, 10), WHITE);
    assert_eq!(pixel(&buffer, 50, 50), RED);
}

#[test]
fn a_counterclockwise_ellipse_takes_the_other_way_round() {
    let half = |canvas: &mut Canvas| {
        canvas
            .set_stroke(Color::rgb(255, 0, 0))
            .set_line_width(6.0)
            .begin_path();
    };
    let buffer = drawn(|canvas| {
        half(canvas);
        canvas
            .ellipse_counterclockwise((50.0, 30.0), (40.0, 20.0), 0.0, 0.0, PI)
            .stroke();
    });
    assert_eq!(pixel(&buffer, 50, 10), RED);
    assert_eq!(pixel(&buffer, 50, 50), WHITE);
    let buffer = drawn(|canvas| {
        half(canvas);
        canvas
            .ellipse((50.0, 30.0), (40.0, 20.0), 0.0, 0.0, PI)
            .stroke();
    });
    assert_eq!(pixel(&buffer, 50, 10), WHITE);
    assert_eq!(pixel(&buffer, 50, 50), RED);
    let buffer = drawn(|canvas| {
        half(canvas);
        canvas
            .ellipse_counterclockwise((50.0, 30.0), (20.0, 10.0), PI / 2.0, 0.0, PI)
            .stroke();
    });
    assert_eq!(pixel(&buffer, 60, 30), RED);
    assert_eq!(pixel(&buffer, 40, 30), WHITE);
}

#[test]
fn zero_length_dashes_with_caps_draw_dots() {
    let dotted = |cap: LineCap| {
        drawn(move |canvas| {
            canvas
                .set_stroke(Color::rgb(255, 0, 0))
                .set_line_width(10.0)
                .set_line_cap(cap)
                .set_line_dash(&[0.0, 20.0])
                .begin_path()
                .move_to(10.0, 30.0)
                .line_to(90.0, 30.0)
                .stroke();
        })
    };
    for cap in [LineCap::Round, LineCap::Square] {
        let buffer = dotted(cap);
        for column in [10, 30, 50, 70] {
            assert_eq!(pixel(&buffer, column, 30), RED);
        }
        for column in [20, 40, 60, 80] {
            assert_eq!(pixel(&buffer, column, 30), WHITE);
        }
    }
    let buffer = dotted(LineCap::Butt);
    for column in [10, 20, 30, 50] {
        assert_eq!(pixel(&buffer, column, 30), WHITE);
    }
}

#[test]
fn a_stroked_curve_keeps_its_outer_edge() {
    let buffer = drawn(|canvas| {
        canvas
            .translate(-450.0, 0.0)
            .set_stroke(Color::rgb(255, 0, 0))
            .set_line_width(8.0)
            .begin_path()
            .ellipse_counterclockwise((500.0, 100.0), (40.0, 80.0), 0.3, 5.0, 0.5)
            .stroke();
    });
    for (column, row) in [(45, 27), (37, 34), (35, 36)] {
        let [red, green, blue, _] = pixel(&buffer, column, row);
        assert!(red == 255 && green < 128 && blue < 128, "{column},{row}");
    }
}

#[test]
fn a_counterclockwise_arc_past_a_full_turn_closes_the_circle() {
    let buffer = drawn(|canvas| {
        canvas
            .set_fill(Color::rgb(255, 0, 0))
            .begin_path()
            .arc_counterclockwise((50.0, 30.0), 20.0, 7.0, 0.0)
            .fill();
    });
    for (column, row) in [(50, 14), (50, 46), (34, 30), (66, 30)] {
        assert_eq!(pixel(&buffer, column, row), RED);
    }
    assert_eq!(pixel(&buffer, 50, 5), WHITE);
}

#[test]
fn an_ellipse_has_two_radii_and_a_rotation() {
    let buffer = drawn(|canvas| {
        canvas
            .set_fill(Color::rgb(255, 0, 0))
            .begin_path()
            .ellipse((50.0, 30.0), (40.0, 10.0), 0.0, 0.0, TAU)
            .fill();
    });
    assert_eq!(pixel(&buffer, 15, 30), RED);
    assert_eq!(pixel(&buffer, 85, 30), RED);
    assert_eq!(pixel(&buffer, 50, 45), WHITE);
    let buffer = drawn(|canvas| {
        canvas
            .set_fill(Color::rgb(255, 0, 0))
            .begin_path()
            .ellipse((50.0, 30.0), (25.0, 10.0), PI / 2.0, 0.0, TAU)
            .fill();
    });
    assert_eq!(pixel(&buffer, 50, 50), RED);
    assert_eq!(pixel(&buffer, 50, 10), RED);
    assert_eq!(pixel(&buffer, 30, 30), WHITE);
}

#[test]
fn bad_arcs_and_ellipses_draw_nothing() {
    let buffer = drawn(|canvas| {
        canvas
            .set_fill(Color::rgb(255, 0, 0))
            .begin_path()
            .ellipse((50.0, 30.0), (-40.0, 10.0), 0.0, 0.0, TAU)
            .ellipse((50.0, 30.0), (40.0, f64::NAN), 0.0, 0.0, TAU)
            .ellipse((50.0, 30.0), (40.0, 10.0), f64::INFINITY, 0.0, TAU)
            .arc_counterclockwise((50.0, 30.0), -1.0, 0.0, TAU)
            .arc_counterclockwise((50.0, 30.0), 20.0, f64::NAN, TAU)
            .fill();
    });
    assert_eq!(pixel(&buffer, 50, 30), WHITE);
}

#[test]
fn enormous_arcs_stay_cheap_and_scaled_arcs_stay_round() {
    let buffer = drawn(|canvas| {
        canvas
            .set_fill(Color::rgb(255, 0, 0))
            .begin_path()
            .arc((0.0, 0.0), 1e50, 0.0, TAU)
            .ellipse((0.0, 0.0), (1e200, 1e120), 0.3, 0.0, TAU)
            .fill();
    });
    assert_eq!(pixel(&buffer, 50, 30), RED);
    let buffer = drawn(|canvas| {
        canvas
            .set_fill(Color::rgb(255, 0, 0))
            .scale(1000.0, 1000.0)
            .begin_path()
            .arc((0.05, 0.03), 0.02, 0.0, TAU)
            .fill();
    });
    let inside = [
        (68, 30),
        (32, 30),
        (50, 48),
        (50, 12),
        (63, 43),
        (37, 17),
        (63, 17),
        (37, 43),
    ];
    let outside = [
        (72, 30),
        (28, 30),
        (50, 52),
        (50, 8),
        (66, 46),
        (34, 14),
        (66, 14),
        (34, 46),
    ];
    for (column, row) in inside {
        assert_eq!(pixel(&buffer, column, row), RED);
    }
    for (column, row) in outside {
        assert_eq!(pixel(&buffer, column, row), WHITE);
    }
}

#[test]
fn dashes_leave_gaps_along_a_line() {
    let buffer = drawn(|canvas| {
        canvas.set_line_dash(&[10.0, 10.0]);
        red_line(canvas);
    });
    assert_eq!(pixel(&buffer, 5, 30), RED);
    assert_eq!(pixel(&buffer, 15, 30), WHITE);
    assert_eq!(pixel(&buffer, 25, 30), RED);
    assert_eq!(pixel(&buffer, 95, 30), WHITE);
}

#[test]
fn a_dash_offset_shifts_the_pattern() {
    let buffer = drawn(|canvas| {
        canvas
            .set_line_dash(&[10.0, 10.0])
            .set_line_dash_offset(10.0);
        red_line(canvas);
    });
    assert_eq!(pixel(&buffer, 5, 30), WHITE);
    assert_eq!(pixel(&buffer, 15, 30), RED);
    let buffer = drawn(|canvas| {
        canvas
            .set_line_dash(&[10.0, 10.0])
            .set_line_dash_offset(-1e300);
        red_line(canvas);
    });
    let row: Vec<bool> = (0..100)
        .map(|column| pixel(&buffer, column, 30) == RED)
        .collect();
    assert_eq!(row.iter().filter(|&&red| red).count(), 50);
}

#[test]
fn an_odd_dash_list_repeats_itself() {
    let buffer = drawn(|canvas| {
        canvas.set_line_dash(&[10.0, 5.0, 20.0]);
        red_line(canvas);
    });
    assert_eq!(pixel(&buffer, 5, 30), RED);
    assert_eq!(pixel(&buffer, 12, 30), WHITE);
    assert_eq!(pixel(&buffer, 25, 30), RED);
    assert_eq!(pixel(&buffer, 40, 30), WHITE);
    assert_eq!(pixel(&buffer, 47, 30), RED);
    assert_eq!(pixel(&buffer, 60, 30), WHITE);
}

#[test]
fn bad_dash_lists_are_ignored_and_an_empty_one_clears() {
    let buffer = drawn(|canvas| {
        canvas
            .set_line_dash(&[10.0, 10.0])
            .set_line_dash(&[5.0, -1.0])
            .set_line_dash(&[f64::NAN, 5.0])
            .set_line_dash(&[3.0; 5_000])
            .set_line_dash_offset(f64::INFINITY);
        red_line(canvas);
    });
    assert_eq!(pixel(&buffer, 5, 30), RED);
    assert_eq!(pixel(&buffer, 15, 30), WHITE);
    let buffer = drawn(|canvas| {
        canvas.set_line_dash(&[10.0, 10.0]).set_line_dash(&[]);
        red_line(canvas);
    });
    assert_eq!(pixel(&buffer, 15, 30), RED);
    let buffer = drawn(|canvas| {
        canvas.set_line_dash(&[0.0, 0.0]);
        red_line(canvas);
    });
    assert_eq!(pixel(&buffer, 15, 30), RED);
}

#[test]
fn restore_brings_back_the_saved_dashes() {
    let buffer = drawn(|canvas| {
        canvas.save().set_line_dash(&[10.0, 10.0]).restore();
        red_line(canvas);
    });
    assert_eq!(pixel(&buffer, 15, 30), RED);
}

#[test]
fn dashes_too_small_to_count_draw_a_solid_line() {
    let buffer = drawn(|canvas| {
        canvas.set_line_dash(&[1e-9, 1e-9]);
        red_line(canvas);
        canvas
            .set_line_dash(&[1e-12])
            .stroke_rect(10.0, 5.0, 80.0, 10.0);
    });
    assert_eq!(pixel(&buffer, 15, 30), RED);
    assert_eq!(pixel(&buffer, 50, 5), RED);
}

#[test]
fn even_odd_fill_leaves_the_inner_shape_empty() {
    let buffer = drawn(|canvas| {
        canvas.set_fill(Color::rgb(255, 0, 0));
        nested_rects(canvas);
        canvas.fill_even_odd();
    });
    assert_eq!(pixel(&buffer, 20, 30), RED);
    assert_eq!(pixel(&buffer, 50, 30), WHITE);
    let buffer = drawn(|canvas| {
        canvas.set_fill(Color::rgb(255, 0, 0));
        nested_rects(canvas);
        canvas.fill();
    });
    assert_eq!(pixel(&buffer, 50, 30), RED);
}

#[test]
fn even_odd_clip_cuts_the_inner_shape_out() {
    let buffer = drawn(|canvas| {
        canvas.save();
        nested_rects(canvas);
        canvas
            .clip_even_odd()
            .set_fill(Color::rgb(255, 0, 0))
            .fill_rect(0.0, 0.0, 100.0, 60.0)
            .restore()
            .fill_rect(0.0, 55.0, 100.0, 5.0);
    });
    assert_eq!(pixel(&buffer, 20, 30), RED);
    assert_eq!(pixel(&buffer, 50, 30), WHITE);
    assert_eq!(pixel(&buffer, 5, 30), WHITE);
    assert_eq!(pixel(&buffer, 50, 57), [0, 0, 0, 255]);
}
