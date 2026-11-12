mod support;

use pageframe::App;
use support::{open, pixel, render};

const BLUE: [u8; 4] = [0, 0, 255, 255];
const LIME: [u8; 4] = [0, 255, 0, 255];

fn corners_are_solid(style: &str) {
    let html = format!(
        "<!DOCTYPE html><html><head><style>html,body{{margin:0;background:white}}</style></head><body><div style='background:lime;width:40px;height:24px;border:2px {style} blue'></div></body></html>"
    );
    let mut harness = open(App::new((), &html));
    let buffer = render(&mut harness);
    for (left, top) in [(0, 0), (42, 0), (0, 26), (42, 26)] {
        for (x, y) in [
            (left, top),
            (left + 1, top),
            (left, top + 1),
            (left + 1, top + 1),
        ] {
            let found = pixel(&buffer, x, y);
            assert_eq!(found, [0, 0, 255, 255], "{style} corner ({x},{y})");
        }
    }
}

#[test]
fn a_double_border_has_solid_corners_like_chromes() {
    corners_are_solid("double");
}

#[test]
fn a_dashed_border_has_solid_corners_like_chromes() {
    corners_are_solid("dashed");
}

fn render_body(body: &str) -> Vec<u8> {
    let html = format!(
        "<!DOCTYPE html><html><head><style>html,body{{margin:0;background:white}}</style></head><body>{body}</body></html>"
    );
    let mut harness = open(App::new((), &html));
    render(&mut harness)
}

#[test]
fn a_see_through_dashed_border_paints_its_corners_once_like_chromes() {
    let buffer = render_body(
        "<div style='background:lime;width:40px;height:30px;border:4px dashed rgba(0,0,255,0.5)'></div>",
    );
    for (x, y) in [(44, 0), (47, 0), (45, 1), (44, 3), (47, 3), (0, 0), (3, 3)] {
        let [red, green, blue, _] = pixel(&buffer, x, y);
        assert!(
            red == 0 && green.abs_diff(126) <= 2 && blue.abs_diff(128) <= 2,
            "corner ({x},{y}) is {red},{green},{blue}, Chrome has 0,126,128"
        );
    }
}

#[test]
fn a_thin_side_of_a_double_border_leaves_the_wide_sides_stripes_whole() {
    let buffer = render_body(
        "<div style='background:lime;width:40px;height:30px;border:1px double blue;border-left-width:6px'></div>",
    );
    for (x, expected) in [(0, BLUE), (1, BLUE), (2, LIME), (3, BLUE)] {
        assert_eq!(pixel(&buffer, x, 0), expected, "({x},0)");
    }
    for (x, expected) in [
        (0, BLUE),
        (1, BLUE),
        (2, LIME),
        (3, LIME),
        (4, BLUE),
        (5, BLUE),
    ] {
        assert_eq!(pixel(&buffer, x, 5), expected, "({x},5)");
    }
}
