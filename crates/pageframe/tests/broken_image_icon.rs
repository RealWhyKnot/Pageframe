mod support;

use pageframe::App;
use support::{open, pixel, render};

const PAGE: &str = "<!doctype html><style>html,body{margin:0;background:white;font:16px/20px Arial}</style><img src='data:,bad' alt='Hello' width='80' height='30' style='display:inline-block'>";

#[test]
fn broken_image_icon_matches_chromes_pixels() {
    let mut harness = open(App::new((), PAGE));
    let buffer = render(&mut harness);
    for (x, y, chrome) in [
        (5, 0, 163),
        (0, 10, 149),
        (15, 12, 146),
        (3, 8, 197),
        (5, 4, 255),
        (12, 4, 145),
        (6, 12, 88),
        (13, 10, 255),
        (4, 15, 131),
        (14, 15, 131),
    ] {
        let red = pixel(&buffer, x, y)[0];
        assert!(
            red.abs_diff(chrome) <= 2,
            "({x},{y}) red {red}, Chrome {chrome}"
        );
    }
}
