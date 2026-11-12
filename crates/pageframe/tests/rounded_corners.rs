mod scaled;
mod support;

use pageframe::App;
use scaled::render_at_scale;
use support::{open, pixel, render};

fn page(body: &str) -> Vec<u8> {
    let html = format!(
        "<!DOCTYPE html><html><head><style>html,body{{margin:0;background:white}}</style></head><body>{body}</body></html>"
    );
    let mut harness = open(App::new((), &html));
    render(&mut harness)
}

fn near(found: u8, expected: u8) -> bool {
    found.abs_diff(expected) <= 6
}

#[test]
fn a_round_corner_has_chromes_edge_pixels() {
    let buffer =
        page("<div style='background:#0a0;width:70px;height:70px;border-radius:10px'></div>");
    let chrome = [
        ((4, 1), 221),
        ((5, 1), 57),
        ((3, 2), 155),
        ((4, 2), 12),
        ((2, 3), 151),
        ((1, 4), 220),
        ((2, 4), 10),
        ((3, 1), 255),
        ((3, 3), 0),
    ];
    for ((x, y), red) in chrome {
        let found = pixel(&buffer, x, y)[0];
        assert!(near(found, red), "({x},{y}) is {found}, Chrome has {red}");
    }
}

#[test]
fn a_round_border_matches_chrome_inside_and_out() {
    let buffer = page(
        "<div style='border:2px solid blue;background:lime;border-radius:6px;width:40px;height:18px'></div>",
    );
    let chrome = [
        ((3, 0), [162, 162, 255]),
        ((4, 0), [82, 82, 255]),
        ((1, 1), [231, 231, 255]),
        ((2, 1), [66, 66, 255]),
        ((0, 3), [155, 155, 255]),
        ((3, 2), [0, 78, 177]),
        ((4, 2), [0, 196, 59]),
        ((2, 4), [0, 199, 56]),
        ((3, 3), [0, 255, 0]),
    ];
    for ((x, y), expected) in chrome {
        let found = pixel(&buffer, x, y);
        for channel in 0..3 {
            assert!(
                near(found[channel], expected[channel]),
                "({x},{y}) is {found:?}, Chrome has {expected:?}"
            );
        }
    }
}

#[test]
fn a_block_keeps_its_fill_inside_an_opaque_round_border() {
    let block = page(
        "<div style='border:2px solid blue;background:lime;border-radius:6px;width:40px;height:18px'></div>",
    );
    for (x, y) in [(3, 0), (1, 1), (0, 3), (0, 4)] {
        let [red, green, ..] = pixel(&block, x, y);
        assert!(red.abs_diff(green) <= 2, "({x},{y}) shows the fill");
    }

    let sheer = page(
        "<div style='border:2px solid rgba(0,0,255,0.5);background:lime;border-radius:6px;width:40px;height:18px'></div>",
    );
    let [red, green, ..] = pixel(&sheer, 22, 0);
    assert!(green > red, "the fill shows through the border");
}

#[test]
fn a_clip_path_circle_has_chromes_edge_pixels() {
    let buffer =
        page("<div style='background:#0a0;width:60px;height:60px;clip-path:circle(30px)'></div>");
    let chrome = [176, 121, 97, 75, 53, 31, 9];
    for (x, red) in (23..).zip(chrome) {
        let found = pixel(&buffer, x, 0)[0];
        assert!(near(found, red), "({x},0) is {found}, Chrome has {red}");
    }
}

#[test]
fn a_clip_path_inset_round_has_chromes_edge_pixels() {
    let buffer = page(
        "<div style='background:#0a0;width:60px;height:60px;clip-path:inset(0 round 12px)'></div>",
    );
    let chrome = [240, 161, 79, 39, 13];
    for (x, red) in (7..).zip(chrome) {
        let found = pixel(&buffer, x, 0)[0];
        assert!(near(found, red), "({x},0) is {found}, Chrome has {red}");
    }
}

#[test]
fn a_dashed_round_border_spaces_its_dashes_like_chromes() {
    let buffer = page(
        "<div style='background:lime;width:60px;height:40px;border:2px dashed blue;border-radius:8px'></div>",
    );
    let chrome = [
        (8, 0),
        (13, 0),
        (14, 255),
        (16, 255),
        (17, 191),
        (23, 63),
        (27, 127),
    ];
    for (x, green) in chrome {
        let found = pixel(&buffer, x, 1)[1];
        assert!(near(found, green), "({x},1) is {found}, Chrome has {green}");
    }
}

#[test]
fn a_dashed_round_border_too_short_for_two_dashes_is_solid() {
    let buffer = page(
        "<div style='background:lime;width:2px;height:2px;border:2px dashed blue;border-radius:3px'></div>",
    );
    for (x, y) in [
        (2, 1),
        (3, 1),
        (1, 2),
        (1, 3),
        (3, 4),
        (2, 4),
        (1, 1),
        (4, 1),
        (1, 4),
        (4, 4),
    ] {
        let [red, green, blue, _] = pixel(&buffer, x, y);
        assert!(red < 30 && green < 30 && blue > 225, "({x},{y}) is blue");
    }
}

#[test]
fn a_short_dashed_round_border_is_dashed_at_double_scale_like_chromes() {
    let html = "<!DOCTYPE html><html><head><style>html,body{margin:0;background:white}</style></head><body><div style='background:lime;width:2px;height:2px;border:2px dashed blue;border-radius:3px'></div></body></html>";
    let mut harness = open(App::new((), html));
    let buffer = render_at_scale(&mut harness, 2);
    let width = usize::try_from(support::WIDTH).expect("test width fits in usize") * 2;
    let chrome = [
        ((2, 2), [0, 255, 0]),
        ((3, 1), [0, 255, 0]),
        ((9, 9), [0, 255, 0]),
        ((7, 2), [0, 0, 255]),
        ((2, 7), [0, 0, 255]),
    ];
    for ((x, y), expected) in chrome {
        let start = (y * width + x) * 4;
        let found = &buffer[start..start + 3];
        for channel in 0..3 {
            assert!(
                near(found[channel], expected[channel]),
                "({x},{y}) is {found:?}, Chrome has {expected:?}"
            );
        }
    }
}
