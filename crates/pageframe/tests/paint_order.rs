mod support;

use pageframe::{App, EventKind};
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

const SEE_THROUGH: &str = "<div id=\"first\" style=\"opacity:.5;background:red;height:20px\"></div>\
<div id=\"late\" class=\"late\" style=\"margin-top:-20px\"></div>";

#[test]
fn see_through_box_paints_over_a_later_block() {
    let buffer = draw(SEE_THROUGH);
    let [red, green, _, _] = pixel(&buffer, 5, 5);
    assert!(red > 100, "the later block covers the box");
    assert!(green > 100, "the box is not see-through");
}

fn clicked(body: &str) -> Vec<&'static str> {
    let html = format!("<html><head><style>{BASE}</style></head><body>{body}</body></html>");
    let app = App::new(Vec::new(), &html)
        .on("#first", EventKind::Click, |seen, _, _| seen.push("first"))
        .on("#late", EventKind::Click, |seen, _, _| seen.push("late"));
    let mut harness = open(app);
    harness.pump();
    harness.click_at(5.0, 5.0);
    harness.doc.state().clone()
}

const LATE_ON_TOP: &str = "<div id=\"late\" class=\"late\" style=\"margin-top:-20px\"></div>";

#[test]
fn click_lands_on_the_box_painted_on_top() {
    assert_eq!(clicked(SEE_THROUGH), ["first"]);
}

#[test]
fn click_lands_on_a_see_through_box_in_a_wrapper() {
    let body = format!(
        "<div><div id=\"first\" style=\"opacity:.5;background:red;height:20px\"></div></div>\
         {LATE_ON_TOP}"
    );
    assert_eq!(clicked(&body), ["first"]);
}

#[test]
fn click_lands_on_a_positioned_box_in_a_wrapper() {
    let body = format!(
        "<div><div id=\"first\" style=\"position:relative;background:red;height:20px\"></div></div>\
         {LATE_ON_TOP}"
    );
    assert_eq!(clicked(&body), ["first"]);
}

#[test]
fn see_through_box_in_a_wrapper_paints_over_a_later_block() {
    let buffer = draw(&format!(
        "<div><div style=\"opacity:.5;background:red;height:20px\"></div></div>{LATE_ON_TOP}"
    ));
    let [red, green, _, _] = pixel(&buffer, 5, 5);
    assert!(red > 100, "the later block covers the box");
    assert!(green > 100, "the box is not see-through");
}

#[test]
fn see_through_flex_items_keep_their_order() {
    let item = "opacity:.5;height:20px;width:40px";
    let body = format!(
        "<div style=\"display:flex\">\
         <div id=\"late\" style=\"{item};order:2;margin-left:-40px;background:blue\"></div>\
         <div id=\"first\" style=\"{item};order:1;background:red\"></div></div>"
    );
    assert_eq!(clicked(&body), ["late"]);
}
