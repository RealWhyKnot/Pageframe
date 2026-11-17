mod support;

use pageframe::App;
use support::{HEIGHT, WIDTH, open, pixel, render};

const NESTED: &str = "<!doctype html><style>html,body{margin:0}p{margin:0;font:16px/20px sans-serif}</style>\
<div style=\"visibility:hidden;background:#f00;border:4px solid #f00;padding:5px;width:50px\">\
<div style=\"visibility:visible;background:#00f;height:20px\"></div></div>\
<p><span style=\"visibility:hidden;background:#f00;color:#f00\">hid</span></p>\
<p style=\"visibility:hidden;background:#f00;color:#f00\">ab \
<span style=\"visibility:visible;background:#0f0;color:#000\">cd</span></p>";

const FADED: &str = "<!doctype html><style>html,body{margin:0}</style>\
<div style=\"visibility:hidden;opacity:0.5;background:#000;padding:5px\">\
<div style=\"visibility:visible;background:#00f;height:20px\"></div></div>";

fn on_white(buffer: &[u8], x: usize, y: usize) -> [u8; 3] {
    let [r, g, b, a] = pixel(buffer, x, y);
    let cover = 255 - a;
    [
        r.saturating_add(cover),
        g.saturating_add(cover),
        b.saturating_add(cover),
    ]
}

fn count(buffer: &[u8], rows: std::ops::Range<usize>, test: impl Fn([u8; 3]) -> bool) -> usize {
    let width = usize::try_from(WIDTH).expect("the width fits");
    rows.flat_map(|y| (0..width).map(move |x| (x, y)))
        .filter(|&(x, y)| test(on_white(buffer, x, y)))
        .count()
}

#[test]
fn visible_children_of_hidden_boxes_paint_like_chrome() {
    let buffer = render(&mut open(App::new((), NESTED)));
    assert_eq!(on_white(&buffer, 30, 19), [0, 0, 255]);
    assert_eq!(on_white(&buffer, 2, 2), [255, 255, 255]);
    assert_eq!(on_white(&buffer, 6, 6), [255, 255, 255]);
    let height = usize::try_from(HEIGHT).expect("the height fits");
    let red = count(&buffer, 0..height, |[r, g, b]| r > 200 && g < 80 && b < 80);
    assert_eq!(red, 0, "hidden boxes and text leave no red behind");
    let green = count(&buffer, 58..78, |[r, g, b]| g > 200 && r < 80 && b < 80);
    assert!(
        green > 100,
        "the visible span paints its background, got {green}"
    );
}

#[test]
fn hidden_boxes_still_fade_visible_children_like_chrome() {
    let buffer = render(&mut open(App::new((), FADED)));
    let [r, g, b] = on_white(&buffer, 30, 15);
    assert!(
        (126..=129).contains(&r) && (126..=129).contains(&g) && b == 255,
        "the child is half transparent blue, got {:?}",
        [r, g, b]
    );
    assert_eq!(on_white(&buffer, 2, 2), [255, 255, 255]);
}
