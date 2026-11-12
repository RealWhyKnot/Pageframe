mod scaled;
mod support;

use pageframe::App;
use scaled::render_at_scale;
use support::{open, pixel, render};

const BLUE: [u8; 4] = [0, 0, 255, 255];
const LIME: [u8; 4] = [0, 255, 0, 255];

fn page(width: &str) -> String {
    format!(
        "<!DOCTYPE html><html><head><style>html,body{{margin:0;background:white}}</style></head><body><div style='background:lime;width:40px;height:24px;border:{width} double blue'></div></body></html>"
    )
}

#[test]
fn a_four_pixel_double_border_has_chromes_whole_pixel_stripes() {
    let mut harness = open(App::new((), &page("4px")));
    let buffer = render(&mut harness);
    for (row, expected) in [BLUE, LIME, LIME, BLUE].into_iter().enumerate() {
        assert_eq!(pixel(&buffer, 20, row), expected, "row {row}");
    }
}

#[test]
fn a_two_pixel_double_border_is_double_at_double_scale_like_chromes() {
    let mut harness = open(App::new((), &page("2px")));
    let buffer = render_at_scale(&mut harness, 2);
    let width = usize::try_from(support::WIDTH).expect("test width fits in usize") * 2;
    for (row, expected) in [BLUE, LIME, LIME, BLUE].into_iter().enumerate() {
        let start = (row * width + 40) * 4;
        assert_eq!(buffer[start..start + 4], expected, "row {row}");
    }
}

#[test]
fn a_double_border_under_three_pixels_is_solid_like_chromes() {
    let mut harness = open(App::new((), &page("2px")));
    let buffer = render(&mut harness);
    for row in 0..2 {
        assert_eq!(pixel(&buffer, 20, row), BLUE, "row {row}");
    }
    assert_eq!(pixel(&buffer, 20, 2), LIME);
}

#[test]
fn a_three_pixel_double_border_has_one_pixel_stripes_like_chromes() {
    let mut harness = open(App::new((), &page("3px")));
    let buffer = render(&mut harness);
    for (row, expected) in [BLUE, LIME, BLUE, LIME].into_iter().enumerate() {
        assert_eq!(pixel(&buffer, 20, row), expected, "row {row}");
    }
}
