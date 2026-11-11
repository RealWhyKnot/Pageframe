mod support;

use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use support::{open, pixel, render};

const RED: [u8; 3] = [255, 0, 0];
const GREEN: [u8; 3] = [0, 128, 0];
const BLUE: [u8; 3] = [0, 0, 255];

const PAGE: &str = r#"<html><head><style>
body{margin:0;background:white}
.box{position:absolute;top:0;width:20px;height:20px;background:rgb(255,0,0)}
.box:has(.on),.box:has(.item){background:rgb(0,128,0)}
.box:has(> input:checked){background:rgb(0,0,255)}
.box:has(+ .flag){background:rgb(0,0,255)}
.box:has(p:empty){background:rgb(0,0,255)}
input{display:none}
li{height:10px;background:rgb(255,0,0)}
li:nth-child(2 of .pick){background:rgb(0,128,0)}
</style></head><body>
<div class="box" id="a" style="left:0"><span id="a1"></span></div>
<div class="box" id="b" style="left:30px"><input type="checkbox" id="b1"></div>
<div class="box" id="c" style="left:60px"></div>
<div class="box" id="d" style="left:90px"></div><p id="d1" style="margin:0"></p>
<div class="box" id="e" style="left:30px;top:22px;height:15px"><p id="e1" style="margin:0"></p></div>
<template id="item"><span class="item"></span></template>
<ul style="position:absolute;top:40px;left:0;margin:0;padding:0;width:20px;list-style:none">
<li class="pick"></li><li></li><li class="pick" id="second"></li><li class="pick"></li>
</ul>
</body></html>"#;

fn frame(harness: &mut Harness<Page<()>>) -> Vec<u8> {
    harness.pump();
    render(harness)
}

fn check(buffer: &[u8], x: usize, y: usize, expected: [u8; 3]) {
    let actual = pixel(buffer, x, y);
    assert_eq!(
        [actual[0], actual[1], actual[2]],
        expected,
        "pixel ({x}, {y})"
    );
}

#[test]
fn has_follows_descendant_class_changes() {
    let mut harness = open(App::new((), PAGE));
    check(&frame(&mut harness), 10, 10, RED);
    harness.doc.ui().select("#a1").add_class("on");
    check(&frame(&mut harness), 10, 10, GREEN);
    harness.doc.ui().select("#a1").remove_class("on");
    check(&frame(&mut harness), 10, 10, RED);
}

#[test]
fn has_follows_child_state_changes() {
    let mut harness = open(App::new((), PAGE));
    check(&frame(&mut harness), 40, 10, RED);
    harness.doc.ui().select("#b1").set_checked(true);
    check(&frame(&mut harness), 40, 10, BLUE);
    harness.doc.ui().select("#b1").set_checked(false);
    check(&frame(&mut harness), 40, 10, RED);
}

#[test]
fn has_follows_inserted_and_removed_children() {
    let mut harness = open(App::new((), PAGE));
    check(&frame(&mut harness), 70, 10, RED);
    let mut ui = harness.doc.ui();
    let item = ui.instantiate("#item");
    ui.select("#c").append(&item);
    check(&frame(&mut harness), 70, 10, GREEN);
    harness.doc.ui().select("#c .item").remove();
    check(&frame(&mut harness), 70, 10, RED);
}

#[test]
fn has_follows_sibling_class_changes() {
    let mut harness = open(App::new((), PAGE));
    check(&frame(&mut harness), 100, 10, RED);
    harness.doc.ui().select("#d1").add_class("flag");
    check(&frame(&mut harness), 100, 10, BLUE);
    harness.doc.ui().select("#d1").remove_class("flag");
    check(&frame(&mut harness), 100, 10, RED);
}

#[test]
fn nth_child_of_selector_counts_matching_siblings() {
    let mut harness = open(App::new((), PAGE));
    let buffer = frame(&mut harness);
    check(&buffer, 10, 45, RED);
    check(&buffer, 10, 65, GREEN);
    check(&buffer, 10, 75, RED);
    harness.doc.ui().select("li:nth-child(2)").add_class("pick");
    let buffer = frame(&mut harness);
    check(&buffer, 10, 55, GREEN);
    check(&buffer, 10, 65, RED);
}

#[test]
fn has_follows_text_making_a_child_non_empty() {
    let mut harness = open(App::new((), PAGE));
    check(&frame(&mut harness), 40, 28, BLUE);
    harness.doc.ui().select("#e1").set_text("x");
    check(&frame(&mut harness), 40, 28, RED);
    harness.doc.ui().select("#e1").clear();
    check(&frame(&mut harness), 40, 28, BLUE);
}
