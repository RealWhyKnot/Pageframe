mod scaled;
mod support;

use pageframe::App;
use scaled::render_at_scale;
use support::{WIDTH, open, pixel, render};

fn check(style: &str, scale: u16, samples: &[(usize, usize, [u8; 3])]) {
    let html = format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:white}}</style></head><body><div style='background:lime;{style}'></div></body></html>"
    );
    let mut harness = open(App::new((), &html));
    let buffer = if scale == 1 {
        render(&mut harness)
    } else {
        render_at_scale(&mut harness, scale)
    };
    let width = usize::try_from(WIDTH).expect("test width fits in usize") * usize::from(scale);
    for &(x, y, expected) in samples {
        let offset = (y * width + x) * 4;
        let rgba = if scale == 1 {
            pixel(&buffer, x, y)
        } else {
            [
                buffer[offset],
                buffer[offset + 1],
                buffer[offset + 2],
                buffer[offset + 3],
            ]
        };
        let found = &rgba[..3];
        for channel in 0..3 {
            assert!(
                found[channel].abs_diff(expected[channel]) <= 6,
                "({x},{y}) at {scale}x is {found:?}, Chrome has {expected:?}"
            );
        }
    }
}

#[test]
fn two_pixel_round_dashes_match_chromes_phase_and_outer_edge() {
    let style = "margin:4px;width:40px;height:24px;border:2px dashed blue;border-radius:6px";
    check(
        style,
        1,
        &[
            (4, 8, [70, 70, 255]),
            (43, 4, [81, 81, 255]),
            (27, 31, [0, 59, 196]),
            (21, 30, [0, 196, 59]),
        ],
    );
    check(
        style,
        2,
        &[
            (86, 8, [79, 79, 255]),
            (95, 54, [78, 78, 255]),
            (14, 9, [109, 128, 236]),
            (10, 59, [157, 157, 255]),
        ],
    );
}

#[test]
fn one_pixel_round_dashes_use_integer_center_outsets() {
    let style = "margin:4px;width:40px;height:24px;border:1px dashed blue;border-radius:6px";
    check(
        style,
        1,
        &[
            (45, 21, [0, 0, 255]),
            (45, 14, [0, 255, 0]),
            (36, 29, [0, 0, 255]),
            (28, 29, [0, 255, 0]),
        ],
    );
    check(
        style,
        2,
        &[
            (91, 47, [0, 0, 255]),
            (91, 33, [0, 255, 0]),
            (69, 9, [0, 255, 0]),
            (63, 59, [0, 255, 0]),
        ],
    );
}

#[test]
fn rounded_dashed_outlines_match_chromes_phase() {
    let style = "margin:12px;width:60px;height:30px;outline:2px dashed blue;border-radius:8px";
    check(
        style,
        1,
        &[
            (72, 38, [55, 55, 255]),
            (13, 41, [152, 152, 255]),
            (65, 10, [75, 75, 255]),
            (10, 35, [71, 71, 255]),
        ],
    );
    check(
        style,
        2,
        &[
            (31, 83, [196, 196, 255]),
            (135, 22, [30, 30, 255]),
            (22, 35, [0, 0, 255]),
            (54, 87, [242, 242, 255]),
        ],
    );
}

#[test]
fn larger_round_dashes_match_chromes_corner_coverage() {
    let style = "width:60px;height:40px;border:2px dashed blue;border-radius:8px";
    check(
        style,
        1,
        &[
            (58, 0, [102, 102, 255]),
            (5, 43, [102, 102, 255]),
            (2, 5, [0, 97, 158]),
            (7, 1, [0, 95, 160]),
        ],
    );
    check(
        style,
        2,
        &[
            (126, 78, [102, 102, 255]),
            (115, 87, [91, 91, 255]),
            (115, 0, [91, 91, 255]),
            (9, 1, [115, 115, 255]),
        ],
    );
}

#[test]
fn tiny_round_dashes_clip_the_background_and_border_together() {
    let style = "width:2px;height:2px;border:2px dashed blue;border-radius:3px";
    check(
        style,
        1,
        &[
            (0, 0, [230, 230, 255]),
            (1, 0, [92, 92, 255]),
            (0, 1, [87, 87, 255]),
            (2, 2, [0, 191, 64]),
        ],
    );
    check(style, 2, &[(1, 4, [0, 28, 227]), (11, 7, [70, 70, 255])]);
}
