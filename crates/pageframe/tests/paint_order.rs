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

const FLOAT_UNDER: &str = "<span style=\"float:left;width:60px;height:10px;\
    margin-right:-60px;background:#0f0\"></span>aaaa bbbb";

#[test]
fn text_paints_over_a_float_in_its_own_block() {
    let buffer = draw(&format!("<div>{FLOAT_UNDER}</div>"));
    assert_eq!(pixel(&buffer, 58, 9), [0, 255, 0, 255]);
    assert!(has_ink(&buffer, 0..10), "the float covers the text");
}

#[test]
fn text_paints_over_a_float_in_a_clipped_block() {
    let buffer = draw(&format!(
        "<div style=\"overflow:hidden\">{FLOAT_UNDER}</div>"
    ));
    assert_eq!(pixel(&buffer, 58, 9), [0, 255, 0, 255]);
    assert!(has_ink(&buffer, 0..10), "the float covers the text");
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

#[test]
fn click_lands_on_text_painted_over_a_float() {
    let body = "<div id=\"first\"><span id=\"late\" style=\"float:left;width:60px;\
        height:10px;margin-right:-60px\"></span>aaaa bbbb</div>";
    assert_eq!(clicked(body), ["first"]);
}

#[test]
fn click_passes_through_hidden_text_to_a_float() {
    let body = "<div id=\"first\"><span id=\"late\" style=\"float:left;width:60px;\
        height:10px;margin-right:-60px\"></span>\
        <span style=\"visibility:hidden\">aaaa bbbb</span></div>";
    assert_eq!(clicked(body), ["late", "first"]);
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

const RED: [u8; 4] = [255, 0, 0, 255];
const LIME: [u8; 4] = [0, 255, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

fn clipped_box(clipper: &str, child: &str) -> Vec<u8> {
    draw(&format!(
        "<div style=\"width:20px;height:20px;background:#0f0;{clipper}\">\
         <b style=\"display:block;width:40px;height:40px;background:red;{child}\"></b></div>"
    ))
}

#[test]
fn a_raised_child_is_clipped_by_a_static_clipping_box() {
    let buffer = clipped_box("overflow:hidden", "position:relative;z-index:1");
    assert_eq!(pixel(&buffer, 10, 10), RED);
    assert_eq!(pixel(&buffer, 30, 10), WHITE);
    assert_eq!(pixel(&buffer, 10, 30), WHITE);
}

#[test]
fn a_lowered_child_is_clipped_by_a_static_clipping_box() {
    let buffer = clipped_box("overflow:hidden", "position:relative;z-index:-1");
    assert_eq!(pixel(&buffer, 10, 10), LIME);
    assert_eq!(pixel(&buffer, 30, 10), WHITE);
    assert_eq!(pixel(&buffer, 10, 30), WHITE);
}

#[test]
fn a_raised_child_is_clipped_on_one_axis_only() {
    let buffer = clipped_box("overflow-x:clip", "position:relative;z-index:1");
    assert_eq!(pixel(&buffer, 10, 30), RED);
    assert_eq!(pixel(&buffer, 30, 10), WHITE);
}

#[test]
fn a_raised_child_is_clipped_to_the_padding_box_of_a_nested_box() {
    let buffer = draw(
        "<div style=\"position:relative;left:5px;width:50px\">\
         <div style=\"width:20px;height:20px;border:3px solid blue;overflow:hidden\">\
         <b style=\"display:block;width:40px;height:40px;background:red;\
         position:relative;z-index:1\"></b></div></div>",
    );
    assert_eq!(pixel(&buffer, 6, 10), [0, 0, 255, 255]);
    assert_eq!(pixel(&buffer, 9, 10), RED);
    assert_eq!(pixel(&buffer, 27, 10), RED);
    assert_eq!(pixel(&buffer, 29, 10), [0, 0, 255, 255]);
    assert_eq!(pixel(&buffer, 15, 24), [0, 0, 255, 255]);
    assert_eq!(pixel(&buffer, 15, 27), WHITE);
}

#[test]
fn a_raised_child_clipped_by_two_boxes_that_miss_each_other_is_not_painted() {
    let buffer = draw(
        "<div style=\"width:20px;height:20px;overflow:hidden\">\
         <div style=\"margin-left:30px;width:20px;height:20px;overflow:hidden\">\
         <b style=\"display:block;width:60px;height:20px;background:red;left:-40px;\
         position:relative;z-index:1\"></b></div></div>",
    );
    for x in [5, 25, 35, 55] {
        assert_eq!(pixel(&buffer, x, 10), WHITE);
    }
}

#[test]
fn a_click_on_the_clipped_part_of_a_raised_child_misses_it() {
    let html = format!(
        "<html><head><style>{BASE}.on{{background:#00f !important}}</style></head><body>\
         <div style=\"width:20px;height:20px;overflow:hidden\">\
         <b id=\"kid\" style=\"display:block;width:40px;height:40px;background:red;\
         position:relative;z-index:1\"></b></div></body></html>"
    );
    let app = App::new((), &html).on("#kid", EventKind::Click, |(), ui, _| {
        ui.select("#kid").add_class("on");
    });
    let mut harness = open(app);
    harness.click_at(30.0, 10.0);
    assert_eq!(pixel(&render(&mut harness), 10, 10), RED);
    harness.click_at(10.0, 10.0);
    assert_eq!(pixel(&render(&mut harness), 10, 10), [0, 0, 255, 255]);
}

#[test]
fn paint_containment_on_an_inline_box_does_not_clip_a_raised_child() {
    let buffer = draw(
        "<div><span style=\"contain:paint\"><b style=\"display:inline-block;width:10px;\
         height:10px;background:red;position:relative;z-index:1;top:30px\"></b></span></div>",
    );
    assert_eq!(pixel(&buffer, 5, 35), RED);
}
