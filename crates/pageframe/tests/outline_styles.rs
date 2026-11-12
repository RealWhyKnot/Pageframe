mod support;

use pageframe::App;
use support::{open, pixel, render};

const BLUE: [u8; 4] = [0, 0, 255, 255];
const DARK: [u8; 4] = [0, 0, 171, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

fn outlined(style: &str) -> Vec<u8> {
    let html = format!(
        "<!DOCTYPE html><html><head><style>html,body{{margin:0;background:white}}</style></head><body><div style='margin:12px;background:lime;width:60px;height:30px;outline:3px {style} blue'></div></body></html>"
    );
    let mut harness = open(App::new((), &html));
    render(&mut harness)
}

#[test]
fn a_double_outline_has_two_stripes_like_chromes() {
    let buffer = outlined("double");
    for (row, expected) in [BLUE, WHITE, BLUE].into_iter().enumerate() {
        assert_eq!(pixel(&buffer, 40, 9 + row), expected, "row {}", 9 + row);
    }
}

#[test]
fn a_groove_outline_rounds_its_halves_like_chromes() {
    let buffer = outlined("groove");
    for (row, expected) in [DARK, DARK, BLUE].into_iter().enumerate() {
        assert_eq!(pixel(&buffer, 40, 9 + row), expected, "row {}", 9 + row);
    }
    for (column, expected) in [DARK, DARK, BLUE].into_iter().enumerate() {
        assert_eq!(
            pixel(&buffer, 72 + column, 25),
            expected,
            "column {}",
            72 + column
        );
    }
}
