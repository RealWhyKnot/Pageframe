mod scaled;
mod support;

use pageframe::App;
use scaled::render_at_scale;
use support::{WIDTH, open, pixel, render};

const BLUE: [u8; 4] = [0, 0, 255, 255];
const LIME: [u8; 4] = [0, 255, 0, 255];

#[test]
fn a_scale_change_recascades_fractional_double_border_widths() {
    let mut harness = open(App::new(
        (),
        "<style>html,body{margin:0}div{width:40px;height:24px;background:lime;border:2.5px double blue}</style><div id='box'></div>",
    ));
    let identifier = harness
        .base_mut()
        .query_selector_all("#box")
        .expect("box selector")[0];
    assert_eq!(
        harness
            .base_mut()
            .get_node(identifier)
            .unwrap()
            .final_layout()
            .size
            .width,
        44.0
    );
    assert_eq!(pixel(&render(&mut harness), 20, 1), BLUE);
    let buffer = render_at_scale(&mut harness, 2);
    let width = usize::try_from(WIDTH).expect("test width fits in usize") * 2;
    assert_eq!(
        harness
            .base_mut()
            .get_node(identifier)
            .unwrap()
            .final_layout()
            .size
            .width,
        45.0
    );
    for (row, expected) in [BLUE, BLUE, LIME, BLUE, BLUE, LIME].into_iter().enumerate() {
        let offset = (row * width + 20) * 4;
        assert_eq!(&buffer[offset..offset + 4], &expected, "row {row}");
    }
}

#[test]
fn one_device_pixel_groove_and_ridge_borders_use_the_border_color() {
    for style in ["groove", "ridge"] {
        let html = format!(
            "<style>html,body{{margin:0}}div{{width:40px;height:30px;background:lime;border:1px {style} blue}}</style><div></div>"
        );
        let mut harness = open(App::new((), &html));
        let buffer = render(&mut harness);
        for (horizontal, vertical) in [(20, 0), (0, 15), (41, 15), (20, 31)] {
            assert_eq!(pixel(&buffer, horizontal, vertical), BLUE, "{style}");
        }
    }
}

#[test]
fn one_device_pixel_groove_and_ridge_outlines_use_the_outline_color() {
    for style in ["groove", "ridge"] {
        let html = format!(
            "<style>html,body{{margin:0;background:white}}div{{margin:12px;width:40px;height:30px;background:lime;outline:1px {style} blue}}</style><div></div>"
        );
        let mut harness = open(App::new((), &html));
        let buffer = render(&mut harness);
        for (horizontal, vertical) in [(25, 11), (11, 25), (52, 25), (25, 42)] {
            assert_eq!(pixel(&buffer, horizontal, vertical), BLUE, "{style}");
        }
    }
}

#[test]
fn rounded_grooves_cover_the_background_between_shades() {
    let mut harness = open(App::new(
        (),
        "<style>html,body{margin:0;background:white}div{width:40px;height:30px;background:lime;border:4px groove blue;border-radius:8px}</style><div></div>",
    ));
    let buffer = render(&mut harness);
    for (horizontal, vertical, blue) in [(2, 5, 203), (5, 2, 201), (45, 5, 223), (5, 35, 224)] {
        let actual = pixel(&buffer, horizontal, vertical);
        for (channel, expected) in actual.into_iter().zip([0, 0, blue, 255]) {
            assert!(channel.abs_diff(expected) <= 2);
        }
    }
}

#[test]
fn mixed_groove_widths_leave_no_background_in_the_corners() {
    let mut harness = open(App::new(
        (),
        "<style>html,body{margin:0}div{width:40px;height:30px;background:lime;border-style:groove;border-color:blue;border-width:3px 2px}</style><div></div>",
    ));
    let buffer = render(&mut harness);
    for (horizontal, vertical, blue) in [(0, 34, 170), (1, 33, 226), (43, 0, 200), (42, 2, 199)] {
        let actual = pixel(&buffer, horizontal, vertical);
        for (channel, expected) in actual.into_iter().zip([0, 0, blue, 255]) {
            assert!(
                channel.abs_diff(expected) <= 6,
                "({horizontal}, {vertical}) {actual:?}"
            );
        }
    }
}

#[test]
fn odd_groove_widths_keep_corner_halves_at_forty_five_degrees() {
    let mut harness = open(App::new(
        (),
        "<style>html,body{margin:0}div{width:40px;height:30px;background:lime;border:3px groove blue}</style><div></div>",
    ));
    let buffer = render(&mut harness);
    for (horizontal, vertical, blue) in [(45, 0, 213), (45, 1, 255), (43, 2, 213), (44, 2, 171)] {
        assert_eq!(pixel(&buffer, horizontal, vertical), [0, 0, blue, 255]);
    }
}
