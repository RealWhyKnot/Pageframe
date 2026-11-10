mod support;

use pageframe::App;
use support::{WIDTH, open, pixel, render};

const BASE: &str = "html,body{margin:0;background:white;font:10px/10px monospace;color:black}\
div{margin:0}.late{background:#0f0;height:20px}";

const TWO_LINES: &str = "aaaa bbbb cccc dddd eeee ffff gggg";

fn draw(body: &str) -> Vec<u8> {
    let html = format!("<html><head><style>{BASE}</style></head><body>{body}</body></html>");
    let mut harness = open(App::new((), &html));
    harness.pump();
    render(&mut harness)
}

fn has_ink(buffer: &[u8], rows: std::ops::Range<usize>) -> bool {
    let width = usize::try_from(WIDTH).expect("test width fits in usize");
    rows.into_iter().any(|y| {
        (0..width).any(|x| {
            let [red, green, blue, _] = pixel(buffer, x, y);
            red < 128 && green < 128 && blue < 128
        })
    })
}

#[test]
fn overflowing_text_paints_over_a_later_background() {
    let buffer = draw(&format!(
        "<div style=\"height:10px\">{TWO_LINES}</div><div class=\"late\"></div>"
    ));
    assert!(has_ink(&buffer, 10..20), "the second line is covered");
    assert_eq!(pixel(&buffer, 118, 25), [0, 255, 0, 255]);
}

#[test]
fn nested_text_paints_over_a_later_background() {
    let buffer = draw(&format!(
        "<div style=\"height:10px\"><div><div>{TWO_LINES}</div></div></div>\
         <div class=\"late\"></div>"
    ));
    assert!(has_ink(&buffer, 10..20), "the second line is covered");
}

#[test]
fn flex_item_text_paints_over_a_later_background() {
    let buffer = draw(&format!(
        "<div style=\"display:flex;height:10px\"><div>{TWO_LINES}</div></div>\
         <div class=\"late\"></div>"
    ));
    assert!(has_ink(&buffer, 10..20), "the second line is covered");
}

#[test]
fn overflowing_background_stays_under_a_later_background() {
    let buffer = draw(
        "<div style=\"height:10px\"><div style=\"background:red;height:30px\"></div></div>\
         <div class=\"late\"></div>",
    );
    assert_eq!(pixel(&buffer, 5, 5), [255, 0, 0, 255]);
    assert_eq!(pixel(&buffer, 5, 15), [0, 255, 0, 255]);
}

#[test]
fn later_positioned_block_covers_earlier_text() {
    let buffer = draw(&format!(
        "<div style=\"height:10px\">{TWO_LINES}</div>\
         <div class=\"late\" style=\"position:relative\"></div>"
    ));
    assert!(has_ink(&buffer, 0..10), "the first line is missing");
    assert!(!has_ink(&buffer, 10..20), "text shows through the block");
}

#[test]
fn positioned_block_in_a_wrapper_covers_earlier_text() {
    let buffer = draw(&format!(
        "<div style=\"height:10px\">{TWO_LINES}</div>\
         <div><div class=\"late\" style=\"position:relative\"></div></div>"
    ));
    assert!(!has_ink(&buffer, 10..20), "text shows through the block");
}

#[test]
fn positioned_block_paints_over_later_text() {
    let buffer = draw(&format!(
        "<div class=\"late\" style=\"position:relative\"></div>\
         <div style=\"margin-top:-20px\">{TWO_LINES}</div>"
    ));
    assert!(!has_ink(&buffer, 0..20), "later text covers the block");
}
