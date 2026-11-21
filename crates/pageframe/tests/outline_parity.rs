mod support;

use pageframe::App;
use support::{open, pixel, render};

const PAGE: &str = "<!DOCTYPE html><html><head><style>html,body{margin:0;background:white}div{position:absolute;width:80px;height:40px;outline:3px dashed blue;outline-offset:5px}</style></head><body><div style='left:20px;top:16px;outline-width:2px;height:41px'></div></body></html>";

fn assert_red_near(buffer: &[u8], x: usize, y: usize, chrome: u8) {
    let red = pixel(buffer, x, y)[0];
    assert!(
        red.abs_diff(chrome) <= 3,
        "({x},{y}) red {red}, Chrome {chrome}"
    );
}

#[test]
fn vertical_dash_ends_land_on_quarter_pixels_like_chromes() {
    let mut harness = open(App::new((), PAGE));
    let buffer = render(&mut harness);
    for (y, chrome) in [
        (18, 191),
        (24, 63),
        (28, 127),
        (34, 127),
        (44, 127),
        (48, 63),
        (54, 191),
    ] {
        assert_red_near(&buffer, 13, y, chrome);
    }
}

#[test]
fn horizontal_dash_ends_keep_exact_coverage_like_chromes() {
    let mut harness = open(App::new((), PAGE));
    let buffer = render(&mut harness);
    for (x, chrome) in [
        (22, 198),
        (28, 56),
        (32, 141),
        (42, 84),
        (52, 27),
        (61, 226),
    ] {
        assert_red_near(&buffer, x, 9, chrome);
    }
}
