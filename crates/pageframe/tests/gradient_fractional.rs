mod scaled;
mod support;

use pageframe::App;
use scaled::{render_at_ratio, render_at_scale};
use support::{WIDTH, open, pixel, render};

const BLACK: [u8; 4] = [0, 0, 0, 255];
const GOLD: [u8; 4] = [255, 204, 0, 255];

const WHITE: [u8; 4] = [255, 255, 255, 255];
const ROW_WIDTH: u32 = 640;
const ROW: &str = "<style>html,body{margin:0;background:#fff;font:14px Arial}div{width:110px;height:70px;margin:4px;display:inline-block}</style><div></div><div></div><div></div><div style=\"background:repeating-linear-gradient(90deg,#000 0 6px,#fc0 6px 12px)\"></div>";

const STRIPES: &str = "<style>html,body{margin:0;background:white}div{width:110px;height:70px;background:repeating-linear-gradient(90deg,#000 0 6px,#fc0 6px 12px)}</style><div></div>";

fn device_pixel(buffer: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let offset = usize::try_from((y * width + x) * 4).expect("pixel offset fits in usize");
    buffer[offset..offset + 4]
        .try_into()
        .expect("a pixel has four channels")
}

#[test]
fn a_hard_stop_on_a_pixel_centre_takes_the_later_colour_at_seven_quarters() {
    let mut harness = open(App::new((), ROW));
    let buffer = render_at_ratio(&mut harness, (7, 4), ROW_WIDTH);
    let width = ROW_WIDTH * 7 / 4;
    assert_eq!(device_pixel(&buffer, width, 626, 60), WHITE);
    assert_eq!(device_pixel(&buffer, width, 627, 60), BLACK);
    for tie in (637..=805).step_by(21) {
        assert_eq!(
            device_pixel(&buffer, width, tie - 1, 60),
            BLACK,
            "column {}",
            tie - 1
        );
        assert_eq!(device_pixel(&buffer, width, tie, 60), GOLD, "column {tie}");
    }
}

#[test]
fn whole_pixel_stops_keep_their_edges_at_one_and_two_times() {
    let mut harness = open(App::new((), STRIPES));
    let buffer = render(&mut harness);
    for (column, expected) in [(5, BLACK), (6, GOLD), (11, GOLD), (12, BLACK)] {
        assert_eq!(pixel(&buffer, column, 35), expected, "1x column {column}");
    }
    let buffer = render_at_scale(&mut harness, 2);
    for (column, expected) in [(11, BLACK), (12, GOLD), (23, GOLD), (24, BLACK)] {
        assert_eq!(
            device_pixel(&buffer, WIDTH * 2, column, 70),
            expected,
            "2x column {column}"
        );
    }
}
