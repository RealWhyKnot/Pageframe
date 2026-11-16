mod scaled;
mod support;

use pageframe::App;
use scaled::render_at_scale;
use support::{WIDTH, open, pixel, render};

const PAGE: &str = r#"<html><body style="margin:0;background:white">
<div style="position:absolute;left:0;top:0;width:120px;height:20px;background:rgb(255,0,0)"></div>
<div style="position:absolute;left:90px;top:0;width:20px;height:10px;backdrop-filter:blur(2px)"><div style="position:absolute;left:-10px;top:12px;width:30px;height:6px;background:rgb(0,0,0)"></div></div>
<div style="position:absolute;left:0;top:30px;width:20px;height:20px;background:rgb(255,0,0);filter:grayscale(1)"></div>
<div style="position:absolute;left:30px;top:30px;width:20px;height:20px;background:rgb(255,0,0);filter:grayscale(1);opacity:0.5"></div>
<div style="position:absolute;left:60px;top:30px;width:20px;height:20px;background:rgb(255,0,0);filter:drop-shadow(10px 10px 0 rgb(0,0,255))"></div>
<div style="position:absolute;left:10px;top:60px;width:20px;height:10px;background:rgb(0,0,0);filter:blur(4px)"></div>
</body></html>"#;

fn assert_near(actual: [u8; 4], chrome: [u8; 3], at: (usize, usize)) {
    let off = (0..3)
        .map(|channel| actual[channel].abs_diff(chrome[channel]))
        .max()
        .unwrap_or(0);
    assert!(
        off <= 3,
        "pixel {at:?} is {actual:?}, Chrome draws {chrome:?}"
    );
}

fn check(buffer: &[u8], x: usize, y: usize, chrome: [u8; 3]) {
    assert_near(pixel(buffer, x, y), chrome, (x, y));
}

#[test]
fn color_filters_match_chrome() {
    let buffer = render(&mut open(App::new((), PAGE)));
    check(&buffer, 10, 40, [54, 54, 54]);
    check(&buffer, 40, 40, [153, 153, 153]);
}

#[test]
fn drop_shadow_sits_under_the_element() {
    let buffer = render(&mut open(App::new((), PAGE)));
    check(&buffer, 65, 35, [255, 0, 0]);
    check(&buffer, 72, 48, [255, 0, 0]);
    check(&buffer, 85, 55, [0, 0, 255]);
    check(&buffer, 81, 35, [255, 255, 255]);
}

#[test]
fn blur_spreads_past_the_box() {
    let buffer = render(&mut open(App::new((), PAGE)));
    check(&buffer, 20, 65, [63, 63, 63]);
    check(&buffer, 20, 62, [82, 82, 82]);
    check(&buffer, 15, 65, [81, 81, 81]);
    check(&buffer, 20, 58, [162, 162, 162]);
    check(&buffer, 20, 56, [202, 202, 202]);
    check(&buffer, 20, 54, [230, 230, 230]);
    check(&buffer, 5, 65, [227, 227, 227]);
    check(&buffer, 3, 65, [244, 244, 244]);
}

#[test]
fn blur_scales_with_the_display() {
    let buffer = render_at_scale(&mut open(App::new((), PAGE)), 2);
    let width = usize::try_from(WIDTH * 2).expect("test width fits in usize");
    let at = |x: usize, y: usize| {
        let start = (y * width + x) * 4;
        let mut rgba = [0; 4];
        rgba.copy_from_slice(&buffer[start..start + 4]);
        rgba
    };
    for (x, y, chrome) in [
        (40, 130, [49, 49, 49]),
        (40, 124, [77, 77, 77]),
        (40, 116, [171, 171, 171]),
        (40, 112, [213, 213, 213]),
        (40, 108, [238, 238, 238]),
        (10, 130, [232, 232, 232]),
        (6, 130, [248, 248, 248]),
        (20, 80, [54, 54, 54]),
    ] {
        assert_near(at(x, y), chrome, (x, y));
    }
}

#[test]
fn backdrop_filter_leaves_children_unclipped() {
    let buffer = render(&mut open(App::new((), PAGE)));
    check(&buffer, 82, 15, [0, 0, 0]);
    check(&buffer, 95, 15, [0, 0, 0]);
}

#[test]
fn backdrop_blur_matches_chrome() {
    let html = r#"<html><body style="margin:0;background:white">
<div style="position:absolute;left:0;top:0;width:60px;height:60px;background:rgb(255,0,0)"></div>
<div style="position:absolute;left:60px;top:0;width:60px;height:60px;background:rgb(0,0,255)"></div>
<div style="position:absolute;left:0;top:8px;width:120px;height:6px;background:rgb(0,0,0)"></div>
<div style="position:absolute;left:40px;top:10px;width:40px;height:40px;backdrop-filter:blur(4px)"></div>
</body></html>"#;
    let buffer = render(&mut open(App::new((), html)));
    for (x, y, chrome) in [
        (40, 10, [90, 0, 0]),
        (50, 16, [182, 0, 1]),
        (56, 30, [202, 0, 53]),
        (60, 30, [116, 0, 139]),
        (63, 20, [50, 0, 190]),
        (70, 13, [0, 0, 125]),
        (79, 49, [0, 0, 255]),
        (30, 30, [255, 0, 0]),
        (85, 11, [0, 0, 0]),
    ] {
        check(&buffer, x, y, chrome);
    }
}
