mod support;

use pageframe::App;
use style::computed_values::list_style_position::T as ListStylePosition;
use style::values::computed::Float;
use style::values::specified::TextAlignKeyword;
use support::{open, pixel, render};

const CONTENT: &str = "<style>html,body{margin:0;font:16px Arial}\
input,textarea{border:2px solid;padding:3px;width:100px;height:24px}</style>\
<div style='height:7px'></div>\
<form id='form'><input id='input'><textarea id='area'></textarea>\
<input id='button' type='image' style='width:10px;height:10px'></form>\
<div id='after'>after</div>\
<div style='line-height:40px;position:relative'>\
<img id='left' align='left' style='width:10px;height:10px'>\
<img id='right' align='RIGHT' style='width:10px;height:10px'>\
<img id='plain' style='width:10px;height:10px'></div>\
<li id='item'>item</li><ul><li id='nested'>nested</li></ul>";

struct Measured {
    input: (f32, f32),
    area: (f32, f32),
    button: (f32, f32),
    form_margin_bottom: f32,
    left: (f32, f32, f32),
    right_x: f32,
    right_margin_left: f32,
    right_float: Float,
    plain_margin_left: f32,
    item: ListStylePosition,
    nested: ListStylePosition,
}

fn measure(standards: bool) -> Measured {
    let doctype = if standards { "<!doctype html>" } else { "" };
    let harness = open(App::new((), &format!("{doctype}{CONTENT}")));
    let base = harness.base();
    let node = |selector: &str| {
        let id = base
            .query_selector_all(selector)
            .expect("the selector parses")[0];
        base.get_node(id).expect("the node exists")
    };
    let size = |selector: &str| {
        let layout = node(selector).final_layout();
        (layout.size.width, layout.size.height)
    };
    let position = |selector: &str| {
        node(selector)
            .primary_styles()
            .expect("the item is styled")
            .get_list()
            .list_style_position
    };
    let form = node("#form").final_layout();
    let after = node("#after").final_layout();
    let left = node("#left").final_layout();
    Measured {
        input: size("#input"),
        area: size("#area"),
        button: size("#button"),
        form_margin_bottom: after.location.y - (form.location.y + form.size.height),
        left: (left.location.x, left.location.y, left.margin.right),
        right_x: node("#right").final_layout().location.x,
        right_margin_left: node("#right").final_layout().margin.left,
        right_float: node("#right")
            .primary_styles()
            .expect("the image is styled")
            .get_box()
            .float,
        plain_margin_left: node("#plain").final_layout().margin.left,
        item: position("#item"),
        nested: position("#nested"),
    }
}

#[test]
fn quirks_mode_text_fields_use_border_box_sizing() {
    let quirks = measure(false);
    assert_eq!(quirks.input, (100.0, 24.0));
    assert_eq!(quirks.area, (100.0, 24.0));
    assert_eq!(quirks.button, (20.0, 20.0));
    let standards = measure(true);
    assert_eq!(standards.input, (110.0, 34.0));
    assert_eq!(standards.area, (110.0, 34.0));
}

#[test]
fn quirks_mode_forms_keep_a_bottom_margin() {
    assert_eq!(measure(false).form_margin_bottom, 16.0);
    assert_eq!(measure(true).form_margin_bottom, 0.0);
}

#[test]
fn aligned_images_float_with_quirks_side_margins() {
    let quirks = measure(false);
    assert_eq!(quirks.left, (0.0, 0.0, 3.0));
    assert_eq!(quirks.right_x, 110.0);
    assert_eq!(quirks.right_margin_left, 3.0);
    assert_eq!(quirks.right_float, Float::Right);
    assert_eq!(quirks.plain_margin_left, 0.0);
    let standards = measure(true);
    assert_eq!(standards.left, (0.0, 0.0, 0.0));
    assert_eq!(standards.right_x, 110.0);
    assert_eq!(standards.right_margin_left, 0.0);
    assert_eq!(standards.right_float, Float::Right);
}

#[test]
fn quirks_mode_list_items_put_markers_inside() {
    let quirks = measure(false);
    assert_eq!(quirks.item, ListStylePosition::Inside);
    assert_eq!(quirks.nested, ListStylePosition::Outside);
    let standards = measure(true);
    assert_eq!(standards.item, ListStylePosition::Outside);
    assert_eq!(standards.nested, ListStylePosition::Outside);
}

fn input_bottom_border(standards: bool) -> Vec<bool> {
    let doctype = if standards { "<!doctype html>" } else { "" };
    let mut harness = open(App::new((), &format!("{doctype}{CONTENT}")));
    let buffer = render(&mut harness);
    [29, 35, 39]
        .into_iter()
        .map(|row| pixel(&buffer, 50, row)[0] < 64)
        .collect()
}

#[test]
fn quirks_mode_text_field_borders_paint_at_the_set_height() {
    assert_eq!(input_bottom_border(false), [true, false, false]);
    assert_eq!(input_bottom_border(true), [false, false, true]);
}

const ALIGN_CONTENT: &str = "<!doctype html><div id='upper' align='CENTER'>a</div>\
<div id='middle' align='middle'>b</div><p id='right' align='Right'>c</p>\
<div id='plain'>d</div><div id='justify' align='JUSTIFY'>e</div>\
<table id='table' align='center'><tr><td id='cell'>f</td></tr></table>";

#[test]
fn align_attributes_ignore_case_and_treat_middle_as_center() {
    let harness = open(App::new((), ALIGN_CONTENT));
    let base = harness.base();
    let align = |selector: &str| {
        let id = base
            .query_selector_all(selector)
            .expect("the selector parses")[0];
        base.get_node(id)
            .expect("the node exists")
            .primary_styles()
            .expect("the element is styled")
            .get_inherited_text()
            .text_align
    };
    assert_eq!(align("#upper"), TextAlignKeyword::MozCenter);
    assert_eq!(align("#middle"), TextAlignKeyword::MozCenter);
    assert_eq!(align("#right"), TextAlignKeyword::MozRight);
    assert_eq!(align("#plain"), TextAlignKeyword::Start);
    assert_eq!(align("#justify"), TextAlignKeyword::Justify);
    assert_eq!(align("#table"), TextAlignKeyword::Start);
    assert_eq!(align("#cell"), TextAlignKeyword::Start);
    let (left, right) = {
        let table_id = base
            .query_selector_all("#table")
            .expect("the selector parses")[0];
        let table = base.get_node(table_id).expect("the node exists");
        let parent = table
            .parent
            .and_then(|parent| base.get_node(parent))
            .expect("the table has a parent");
        let left = table.final_layout().location.x;
        let width = table.final_layout().size.width;
        (left, parent.final_layout().size.width - left - width)
    };
    drop(base);
    assert!(left > 0.0, "the centred table starts at {left}");
    assert!(
        (left - right).abs() <= 1.0,
        "the centred table has {left} on the left and {right} on the right"
    );
}
