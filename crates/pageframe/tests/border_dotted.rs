mod support;

use pageframe::App;
use support::{open, pixel, render};

const BLUE: [u8; 4] = [0, 0, 255, 255];
const LIME: [u8; 4] = [0, 255, 0, 255];

fn dotted(width: &str) -> Vec<u8> {
    let html = format!(
        "<!DOCTYPE html><html><head><style>html,body{{margin:0;background:white}}</style></head><body><div style='background:lime;width:60px;height:30px;border:{width} dotted blue'></div></body></html>"
    );
    let mut harness = open(App::new((), &html));
    render(&mut harness)
}

#[test]
fn a_three_pixel_dotted_border_has_chromes_square_dots_and_end_dots() {
    let buffer = dotted("3px");
    let expected = [
        (0, BLUE),
        (3, BLUE),
        (4, LIME),
        (6, LIME),
        (7, BLUE),
        (9, BLUE),
        (10, LIME),
        (55, BLUE),
        (57, BLUE),
        (58, LIME),
        (61, LIME),
        (62, BLUE),
        (65, BLUE),
    ];
    for (x, color) in expected {
        assert_eq!(pixel(&buffer, x, 1), color, "({x},1)");
    }
}

#[test]
fn a_one_pixel_dotted_border_alternates_after_a_two_pixel_start_dot() {
    let buffer = dotted("1px");
    for (x, color) in [
        (0, BLUE),
        (1, BLUE),
        (2, LIME),
        (3, BLUE),
        (4, LIME),
        (5, BLUE),
    ] {
        assert_eq!(pixel(&buffer, x, 0), color, "({x},0)");
    }
}

#[test]
fn a_wide_dotted_border_draws_round_dots() {
    let buffer = dotted("6px");
    let mut covered = 0.0;
    for y in 0..6 {
        for x in 10..18 {
            covered += 1.0 - f64::from(pixel(&buffer, x, y)[1]) / 255.0;
        }
    }
    let circle = std::f64::consts::PI * 9.0;
    assert!(
        (covered - circle).abs() < 0.4,
        "dot covers {covered}, circle {circle}"
    );
}
