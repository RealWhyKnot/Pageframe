mod scaled;
mod support;

use pageframe::App;
use scaled::render_at_scale;
use support::{open, pixel, render};

fn page(style: &str) -> Vec<u8> {
    let html = format!(
        "<!DOCTYPE html><html><head><style>html,body{{margin:0;background:white}}</style></head><body><div style='background:lime;width:40px;height:20px;margin:14px;{style}'></div></body></html>"
    );
    let mut harness = open(App::new((), &html));
    render(&mut harness)
}

fn is_blue(found: [u8; 4]) -> bool {
    found[0] < 30 && found[1] < 30 && found[2] > 225
}

fn is_lime(found: [u8; 4]) -> bool {
    found[0] < 30 && found[1] > 225 && found[2] < 30
}

fn is_white(found: [u8; 4]) -> bool {
    found[0] > 225 && found[1] > 225 && found[2] > 225
}

#[test]
fn an_outline_offset_leaves_a_gap() {
    let buffer = page("outline:2px solid blue;outline-offset:3px");
    assert!(is_white(pixel(&buffer, 8, 24)), "outside the outline");
    assert!(is_blue(pixel(&buffer, 9, 24)), "outer outline pixel");
    assert!(is_blue(pixel(&buffer, 10, 24)), "inner outline pixel");
    assert!(is_white(pixel(&buffer, 11, 24)), "gap starts");
    assert!(is_white(pixel(&buffer, 13, 24)), "gap ends");
    assert!(is_lime(pixel(&buffer, 14, 24)), "the box");
}

#[test]
fn a_negative_outline_offset_draws_over_the_fill() {
    let buffer = page("outline:2px solid blue;outline-offset:-6px");
    assert!(is_lime(pixel(&buffer, 17, 24)), "fill outside the outline");
    assert!(is_blue(pixel(&buffer, 18, 24)), "outer outline pixel");
    assert!(is_blue(pixel(&buffer, 19, 24)), "inner outline pixel");
    assert!(is_lime(pixel(&buffer, 20, 24)), "fill inside the outline");
}

#[test]
fn an_outline_pulled_past_the_middle_is_not_drawn() {
    let buffer = page("outline:2px solid blue;outline-offset:-30px");
    for x in 14..54 {
        assert!(is_lime(pixel(&buffer, x, 24)), "({x},24) is fill");
    }
}

#[test]
fn an_offset_outline_grows_its_corner_by_the_whole_offset() {
    let buffer = page("outline:2px solid blue;outline-offset:6px;border-radius:2px");
    assert!(is_white(pixel(&buffer, 7, 7)), "outside a 10px corner");
    assert!(pixel(&buffer, 8, 8)[0] > 200, "still outside a 10px corner");
    assert!(is_blue(pixel(&buffer, 6, 24)), "the straight edge");
}

#[test]
fn a_spread_shadow_grows_its_corner() {
    let buffer = page("box-shadow:0 0 0 3px red;border-radius:10px");
    assert!(pixel(&buffer, 14, 14)[1] > 150, "outside a 13px corner");
    let edge = pixel(&buffer, 12, 24);
    assert!(edge[0] > 225 && edge[1] < 30, "the straight edge is red");
}

#[test]
fn an_inset_spread_wider_than_the_radius_has_a_square_hole() {
    let buffer = page("box-shadow:inset 0 0 0 6px red;border-radius:4px");
    let ring = pixel(&buffer, 19, 19);
    assert!(ring[0] > 225 && ring[1] < 30, "the shadow ring");
    assert!(is_lime(pixel(&buffer, 20, 20)), "the hole's corner pixel");
}

fn near(found: u8, expected: u8) -> bool {
    found.abs_diff(expected) <= 8
}

#[test]
fn a_blurred_shadow_fades_like_chromes() {
    let buffer = page("box-shadow:0 0 6px red");
    for (x, green) in [(13, 143), (11, 201), (8, 246), (5, 254)] {
        let found = pixel(&buffer, x, 24)[1];
        assert!(
            near(found, green),
            "({x},24) is {found}, Chrome has {green}"
        );
    }
}

#[test]
fn a_blurred_inset_shadow_fades_like_chromes() {
    let buffer = page("box-shadow:inset 0 0 6px red");
    for (x, red) in [(14, 108), (16, 57), (18, 20), (22, 0)] {
        let found = pixel(&buffer, x, 24)[0];
        assert!(near(found, red), "({x},24) is {found}, Chrome has {red}");
    }
}

#[test]
fn a_blurred_shadow_doubles_its_reach_at_double_scale() {
    let html = "<!DOCTYPE html><html><head><style>html,body{margin:0;background:white}</style></head><body><div style='background:lime;width:40px;height:20px;margin:14px;box-shadow:0 0 6px red'></div></body></html>";
    let mut harness = open(App::new((), html));
    let buffer = render_at_scale(&mut harness, 2);
    let width = usize::try_from(support::WIDTH).expect("test width fits in usize") * 2;
    for (x, green) in [(27, 135), (25, 166), (21, 217), (15, 248)] {
        let found = buffer[(48 * width + x) * 4 + 1];
        assert!(
            near(found, green),
            "({x},48) is {found}, Chrome has {green}"
        );
    }
}
