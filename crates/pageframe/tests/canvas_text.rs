mod support;

use std::cell::RefCell;

use pageframe::{App, Canvas, Color, TextAlign, TextBaseline};
use support::{HEIGHT, WIDTH, open, pixel, render};

const WHITE: [u8; 4] = [255, 255, 255, 255];

const PAGE: &str = r#"<html><head><style>
body{margin:0;background:white}
canvas{display:block}
</style></head><body>
<canvas id="c" width="120" height="80"></canvas>
</body></html>"#;

struct Ink {
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
}

fn drawn(paint: impl Fn(&mut Canvas)) -> Vec<u8> {
    let mut harness = open(App::new((), PAGE));
    harness.doc.ui().select("#c").draw(paint);
    harness.pump();
    render(&mut harness)
}

fn ink(buffer: &[u8]) -> Option<Ink> {
    let width = usize::try_from(WIDTH).expect("test width fits in usize");
    let height = usize::try_from(HEIGHT).expect("test height fits in usize");
    let mut found: Option<Ink> = None;
    for y in 0..height {
        for x in 0..width {
            if pixel(buffer, x, y) == WHITE {
                continue;
            }
            let ink = found.get_or_insert(Ink {
                left: x,
                top: y,
                right: x,
                bottom: y,
            });
            ink.left = ink.left.min(x);
            ink.top = ink.top.min(y);
            ink.right = ink.right.max(x);
            ink.bottom = ink.bottom.max(y);
        }
    }
    found
}

fn measured(paint: impl Fn(&mut Canvas) -> f64) -> f64 {
    let width = RefCell::new(0.0);
    drawn(|canvas| {
        *width.borrow_mut() = paint(canvas);
    });
    width.into_inner()
}

#[test]
fn text_is_drawn_from_its_start_on_the_baseline() {
    let buffer = drawn(|canvas| {
        canvas.set_font("Arial", 20.0).fill_text("HHH", 30.0, 50.0);
    });
    let ink = ink(&buffer).expect("text leaves ink");
    assert!((30..=33).contains(&ink.left), "left {}", ink.left);
    assert!((34..=37).contains(&ink.top), "top {}", ink.top);
    assert!((48..=50).contains(&ink.bottom), "bottom {}", ink.bottom);
}

#[test]
fn text_takes_the_fill_colour() {
    let buffer = drawn(|canvas| {
        canvas
            .set_fill(Color::rgb(255, 0, 0))
            .set_font("Arial", 60.0)
            .set_font_weight(700.0)
            .fill_text("I", 40.0, 60.0);
    });
    let ink = ink(&buffer).expect("text leaves ink");
    let middle = pixel(
        &buffer,
        (ink.left + ink.right) / 2,
        (ink.top + ink.bottom) / 2,
    );
    assert_eq!(middle, [255, 0, 0, 255]);
}

#[test]
fn alignment_moves_text_around_its_anchor() {
    let right = drawn(|canvas| {
        canvas
            .set_font("Arial", 20.0)
            .set_text_align(TextAlign::Right)
            .fill_text("HHH", 60.0, 50.0);
    });
    let right = ink(&right).expect("text leaves ink");
    assert!((57..=60).contains(&right.right), "right {}", right.right);

    let center = drawn(|canvas| {
        canvas
            .set_font("Arial", 20.0)
            .set_text_align(TextAlign::Center)
            .fill_text("HHH", 60.0, 50.0);
    });
    let center = ink(&center).expect("text leaves ink");
    let middle = (center.left + center.right) / 2;
    assert!((59..=61).contains(&middle), "middle {middle}");
}

#[test]
fn baselines_move_text_around_its_anchor() {
    let top = drawn(|canvas| {
        canvas
            .set_font("Arial", 20.0)
            .set_text_baseline(TextBaseline::Top)
            .fill_text("Hg", 30.0, 30.0);
    });
    let top = ink(&top).expect("text leaves ink");
    assert!((30..=34).contains(&top.top), "top {}", top.top);

    let bottom = drawn(|canvas| {
        canvas
            .set_font("Arial", 20.0)
            .set_text_baseline(TextBaseline::Bottom)
            .fill_text("Hg", 30.0, 60.0);
    });
    let bottom = ink(&bottom).expect("text leaves ink");
    assert!(
        (57..=60).contains(&bottom.bottom),
        "bottom {}",
        bottom.bottom
    );

    let middle = drawn(|canvas| {
        canvas
            .set_font("Arial", 20.0)
            .set_text_baseline(TextBaseline::Middle)
            .fill_text("H", 30.0, 40.0);
    });
    let middle = ink(&middle).expect("text leaves ink");
    assert!(middle.top < 40 && middle.bottom > 40);
}

#[test]
fn text_follows_the_transform() {
    let buffer = drawn(|canvas| {
        canvas
            .translate(40.0, 20.0)
            .scale(2.0, 2.0)
            .set_font("Arial", 10.0)
            .fill_text("H", 0.0, 10.0);
    });
    let ink = ink(&buffer).expect("text leaves ink");
    assert!((40..=44).contains(&ink.left), "left {}", ink.left);
    assert!((38..=40).contains(&ink.bottom), "bottom {}", ink.bottom);
    assert!(
        ink.bottom - ink.top >= 12,
        "height {}",
        ink.bottom - ink.top
    );
}

#[test]
fn measured_width_grows_with_text_and_size() {
    let short = measured(|canvas| canvas.set_font("Arial", 10.0).measure_text("abc"));
    let long = measured(|canvas| canvas.set_font("Arial", 10.0).measure_text("abcabc"));
    let large = measured(|canvas| canvas.set_font("Arial", 20.0).measure_text("abc"));
    let empty = measured(|canvas| canvas.measure_text(""));
    assert!(short > 0.0);
    assert!((long - short * 2.0).abs() < 0.01, "{short} {long}");
    assert!((large - short * 2.0).abs() < 0.01, "{short} {large}");
    assert!(empty.abs() < f64::EPSILON);
}

#[test]
fn line_breaks_and_tabs_count_as_spaces() {
    let spaced = measured(|canvas| canvas.measure_text("a b c"));
    let broken = measured(|canvas| canvas.measure_text("a\nb\tc"));
    let exotic = measured(|canvas| canvas.measure_text("a\u{2028}b\u{b}c"));
    assert!(spaced > 0.0);
    assert!((spaced - broken).abs() < f64::EPSILON, "{spaced} {broken}");
    assert!((spaced - exotic).abs() < f64::EPSILON, "{spaced} {exotic}");
    let buffer = drawn(|canvas| {
        canvas
            .set_font("Arial", 12.0)
            .fill_text("a\nb\u{2029}c\u{85}d", 10.0, 20.0);
    });
    let ink = ink(&buffer).expect("text leaves ink");
    assert!(ink.bottom <= 21, "bottom {}", ink.bottom);
}

#[test]
fn restore_brings_back_the_font() {
    let small = measured(|canvas| canvas.set_font("Arial", 10.0).measure_text("abc"));
    let restored = measured(|canvas| {
        canvas.set_font("Arial", 10.0).save();
        canvas
            .set_font("Arial", 30.0)
            .set_font_weight(700.0)
            .restore();
        canvas.measure_text("abc")
    });
    assert!(small > 0.0);
    assert!(
        (small - restored).abs() < f64::EPSILON,
        "{small} {restored}"
    );
}

#[test]
fn bad_font_settings_keep_the_last_good_ones() {
    let good = measured(|canvas| canvas.set_font("Arial", 10.0).measure_text("abc"));
    let after = measured(|canvas| {
        canvas
            .set_font("Arial", 10.0)
            .set_font("Arial", f32::NAN)
            .set_font("Arial", -5.0)
            .set_font("Arial", 0.0)
            .set_font_weight(f32::INFINITY)
            .set_font_weight(0.0)
            .measure_text("abc")
    });
    assert!(good > 0.0);
    assert!((good - after).abs() < f64::EPSILON, "{good} {after}");
}

#[test]
fn unknown_fonts_and_hostile_text_still_draw() {
    let long = "W".repeat(20_000);
    let buffer = drawn(|canvas| {
        canvas
            .set_font("No Such Font 12345", 16.0)
            .fill_text("H", f64::NAN, 20.0)
            .fill_text("H", 10.0, f64::INFINITY)
            .fill_text(&long, 0.0, 40.0)
            .fill_text("\u{202e}\u{0}\u{feff}e\u{301}\u{1f600}", 10.0, 70.0);
    });
    let ink = ink(&buffer).expect("text leaves ink");
    assert!(ink.right >= 118, "right {}", ink.right);
    assert!(ink.top >= 22, "top {}", ink.top);
}
