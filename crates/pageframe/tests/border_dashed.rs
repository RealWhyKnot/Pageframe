mod support;

use pageframe::App;
use support::{open, pixel, render};

const BLUE: [u8; 4] = [0, 0, 255, 255];
const LIME: [u8; 4] = [0, 255, 0, 255];

#[test]
fn a_straight_dashed_side_keeps_its_dash_length_and_stretches_the_gaps_like_chromes() {
    let html = "<!DOCTYPE html><html><head><style>html,body{margin:0;background:white}</style></head><body><div style='background:lime;width:40px;height:24px;border:2px dashed blue'></div></body></html>";
    let mut harness = open(App::new((), html));
    let buffer = render(&mut harness);
    for (x, color) in [
        (5, BLUE),
        (6, LIME),
        (8, LIME),
        (10, BLUE),
        (14, BLUE),
        (16, LIME),
        (19, BLUE),
        (24, BLUE),
        (25, LIME),
    ] {
        assert_eq!(pixel(&buffer, x, 0), color, "({x},0)");
    }
    let half = pixel(&buffer, 9, 0);
    assert!((126..=130).contains(&half[2]), "(9,0) is {half:?}");
}
