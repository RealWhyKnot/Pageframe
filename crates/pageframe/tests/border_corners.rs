mod support;

use pageframe::App;
use support::{open, pixel, render};

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
