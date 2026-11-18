mod scaled;
mod support;

use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use scaled::render_at_scale;
use support::{open, pixel, render};

const PAGE: &str = "<!doctype html><style>html,body{margin:0;background:white;font:14px Arial}div{border:1px solid #999;margin:3px}</style>\
<div>ab<ruby>x<rt>long annotation</rt></ruby>cd</div>\
<div><ruby>base text<rt>a</rt></ruby> after</div>\
<div><ruby><rb>A</rb><rb>B</rb><rt>1</rt><rt>2</rt></ruby></div>";

fn boxes<S: 'static>(harness: &mut Harness<Page<S>>, selector: &str) -> Vec<[f64; 4]> {
    let doc = harness.base_mut();
    doc.query_selector_all(selector)
        .expect("valid selector")
        .into_iter()
        .map(|id| {
            let rect = doc
                .get_client_bounding_rect(id)
                .expect("the element has a box");
            [rect.x, rect.y, rect.width, rect.height]
        })
        .collect()
}

fn inline_extents<S: 'static>(harness: &mut Harness<Page<S>>) -> Vec<[f64; 4]> {
    boxes(harness, "ruby")
        .into_iter()
        .map(|[left, _, width, _]| [left, 0.0, width, 0.0])
        .collect()
}

fn assert_boxes(found: &[[f64; 4]], chrome: &[[f64; 4]], what: &str) {
    assert_eq!(found.len(), chrome.len(), "{what}");
    for (found, chrome) in found.iter().zip(chrome) {
        assert!(
            found
                .iter()
                .zip(chrome)
                .all(|(have, want)| (have - want).abs() < 0.05),
            "{what}: {found:?}, Chrome {chrome:?}"
        );
    }
}

#[test]
fn annotations_sit_above_their_bases_like_chrome() {
    let mut harness = open(App::new((), PAGE));
    assert_boxes(
        &boxes(&mut harness, "div"),
        &[
            [3.0, 3.0, 114.0, 23.0],
            [3.0, 29.0, 114.0, 23.0],
            [3.0, 55.0, 114.0, 23.0],
        ],
        "divs at 1x",
    );
    assert_boxes(
        &inline_extents(&mut harness),
        &[
            [19.58, 0.0, 41.88, 0.0],
            [4.0, 0.0, 56.83, 0.0],
            [4.0, 0.0, 22.59, 0.0],
        ],
        "rubies at 1x",
    );
    assert_boxes(
        &boxes(&mut harness, "rt"),
        &[
            [16.58, 4.0, 47.88, 7.0],
            [4.0, 30.0, 56.81, 7.0],
            [4.0, 56.0, 18.69, 7.0],
            [22.69, 56.0, 3.9, 7.0],
        ],
        "annotations at 1x",
    );

    let buffer = render(&mut harness);
    let first_ink_after_base =
        (50..120).find(|&across| (14..24).any(|down| pixel(&buffer, across, down)[0] < 128));
    assert!(
        first_ink_after_base.is_some_and(|across| (61..=63).contains(&across)),
        "the text after the ruby starts at {first_ink_after_base:?}, Chrome 61.45"
    );

    render_at_scale(&mut harness, 2);
    assert_boxes(
        &inline_extents(&mut harness),
        &[
            [19.58, 0.0, 40.88, 0.0],
            [4.0, 0.0, 56.82, 0.0],
            [4.0, 0.0, 22.58, 0.0],
        ],
        "rubies at 2x",
    );
    assert_boxes(
        &boxes(&mut harness, "div"),
        &[
            [3.0, 3.0, 114.0, 23.5],
            [3.0, 29.5, 114.0, 23.5],
            [3.0, 56.0, 114.0, 23.5],
        ],
        "divs at 2x",
    );
    assert_boxes(
        &boxes(&mut harness, "rt"),
        &[
            [16.08, 3.0, 47.88, 8.0],
            [4.0, 29.5, 56.81, 8.0],
            [4.0, 56.0, 18.69, 8.0],
            [22.69, 56.0, 3.9, 8.0],
        ],
        "annotations at 2x",
    );
}

#[test]
fn unusual_rubies_match_chrome() {
    let mut harness = open(App::new(
        (),
        "<!doctype html><style>html,body{margin:0;font:14px Arial}div{border:1px solid #999;margin:3px;width:200px}</style>\
<div>ab<ruby id=hidden>base<rt style=\"display:none\">long annotation</rt></ruby>cd</div>\
<div>\n  <ruby id=blank>x<rt>long annotation</rt></ruby> after</div>\
<div><ruby id=floated style=\"float:left\">base<rt id=floated-rt>annotation</rt></ruby>text</div>\
<div dir=rtl><ruby><rb id=wide>AAA</rb><rt>1</rt><rb id=narrow>B</rb><rt>2</rt></ruby></div>\
<div dir=rtl>ab<ruby>x<rt id=rtl-rt>long annotation</rt></ruby>cd</div>",
    ));
    assert_boxes(
        &boxes(&mut harness, "div"),
        &[
            [3.0, 3.0, 202.0, 18.0],
            [3.0, 24.0, 202.0, 23.0],
            [3.0, 50.0, 202.0, 18.0],
            [3.0, 71.0, 202.0, 23.0],
            [3.0, 97.0, 202.0, 23.0],
        ],
        "divs",
    );
    assert_boxes(
        &boxes(&mut harness, "#hidden, #blank, #floated"),
        &[
            [19.58, 4.0, 30.36, 16.0],
            [4.0, 30.0, 44.88, 16.0],
            [4.0, 51.0, 32.7, 21.0],
        ],
        "rubies",
    );
    assert_boxes(
        &boxes(&mut harness, "#floated-rt, #wide, #narrow, #rtl-rt"),
        &[
            [4.0, 51.0, 32.7, 7.0],
            [175.98, 77.0, 28.02, 16.0],
            [166.64, 77.0, 9.34, 16.0],
            [144.31, 98.0, 47.89, 7.0],
        ],
        "annotation and bases",
    );
}

#[test]
fn a_ruby_without_annotations_stays_inline() {
    let mut harness = open(App::new(
        (),
        "<!doctype html><style>html,body{margin:0;font:14px Arial}</style><div><ruby>no annotation</ruby> plain</div>",
    ));
    assert_boxes(
        &boxes(&mut harness, "div"),
        &[[0.0, 0.0, 120.0, 16.0]],
        "div",
    );
}

#[test]
fn only_ruby_children_get_the_small_font_like_chrome() {
    let mut harness = open(App::new(
        (),
        "<!doctype html><style>html,body{margin:0;font:14px Arial}div{border:1px solid #999;margin:3px;width:200px}</style>\
<div>ab<ruby>x<rtc><rt>long annotation</rt></rtc></ruby>cd</div>\
<div lang=zh-TW>ab<ruby>x<rtc><rt>long annotation</rt></rtc></ruby>cd</div>\
<div lang=zh-TW>ab<ruby>x<rt>long annotation</rt></ruby>cd</div>\
<div>ab<rt>loose rt</rt>cd</div>",
    ));
    assert_boxes(
        &boxes(&mut harness, "ruby, rtc, rt"),
        &[
            [19.58, 4.0, 102.77, 16.0],
            [26.58, 4.0, 95.77, 16.0],
            [26.58, 4.0, 95.77, 16.0],
            [19.58, 25.0, 102.77, 16.0],
            [26.58, 25.0, 95.77, 16.0],
            [26.58, 25.0, 95.77, 16.0],
            [19.58, 51.0, 41.88, 16.0],
            [16.58, 46.0, 47.88, 7.0],
            [19.58, 72.0, 45.92, 16.0],
        ],
        "rubies, containers and annotations",
    );
}

#[test]
fn an_inline_ruby_box_covers_only_its_base_line_like_chrome() {
    let mut harness = open(App::new(
        (),
        "<!doctype html><style>html,body{margin:0;font:14px Arial}div{border:1px solid #999;margin:3px;width:200px}</style>\
<div>ab<ruby id=plain>x<rt>long annotation</rt></ruby>cd</div>\
<div style=\"line-height:30px\">ab<ruby>x<rt>long annotation</rt></ruby>cd</div>\
<div>ab<ruby style=\"font-size:20px\">x<rt>long annotation</rt></ruby>cd</div>\
<div>ab<ruby style=\"vertical-align:5px\">x<rt>long annotation</rt></ruby>cd</div>\
<div>ab<ruby><rb>A</rb><rb>B</rb><rt>1</rt><rt>2</rt></ruby>cd</div>\
<div>ab<ruby id=padded style=\"padding:2px 4px;border:1px solid red\">x<rt>long annotation</rt></ruby>cd</div>",
    ));
    let padded_extent = |harness: &mut Harness<Page<()>>| {
        boxes(harness, "#padded")
            .into_iter()
            .map(|[left, _, width, height]| [left, 0.0, width, height])
            .collect::<Vec<_>>()
    };
    assert_boxes(
        &boxes(&mut harness, "ruby:not(#padded)"),
        &[
            [19.58, 9.0, 41.88, 16.0],
            [19.58, 37.0, 41.88, 16.0],
            [19.58, 73.0, 58.41, 22.0],
            [19.58, 106.0, 41.88, 16.0],
            [19.58, 137.0, 22.59, 16.0],
        ],
        "rubies at 1x",
    );
    assert_boxes(
        &padded_extent(&mut harness),
        &[[19.58, 0.0, 51.88, 22.0]],
        "padded ruby at 1x",
    );
    let doc = harness.base_mut();
    let plain = doc
        .query_selector("#plain")
        .expect("valid selector")
        .expect("the ruby exists");
    let offset = doc.offset_rect(plain).expect("the ruby has a box");
    assert_eq!(
        [offset.y.round(), offset.height.round()],
        [9.0, 16.0],
        "offsetTop and offsetHeight"
    );
    drop(doc);

    render_at_scale(&mut harness, 2);
    assert_boxes(
        &boxes(&mut harness, "ruby:not(#padded)"),
        &[
            [19.58, 9.5, 40.88, 15.5],
            [19.58, 37.5, 40.88, 15.5],
            [19.58, 73.5, 58.39, 22.0],
            [19.58, 106.5, 40.88, 15.5],
            [19.58, 138.0, 22.58, 15.5],
        ],
        "rubies at 2x",
    );
    assert_boxes(
        &padded_extent(&mut harness),
        &[[19.58, 0.0, 50.88, 21.5]],
        "padded ruby at 2x",
    );
}

#[test]
fn a_ruby_in_columns_reports_the_same_base_line_box() {
    let mut harness = open(App::new(
        (),
        "<!doctype html><style>html,body{margin:0;font:14px Arial}</style>\
<div style=\"columns:2;column-gap:10px;width:210px;border:1px solid #999;margin:3px\">ab<ruby id=ruby>x<rt>long annotation</rt></ruby>cd</div>",
    ));
    assert_boxes(
        &boxes(&mut harness, "#ruby"),
        &[[19.58, 9.0, 41.88, 16.0]],
        "client rect",
    );
    let offset = {
        let doc = harness.base_mut();
        let ruby = doc
            .query_selector("#ruby")
            .expect("valid selector")
            .expect("the ruby exists");
        doc.offset_rect(ruby).expect("the ruby has a box")
    };
    assert_eq!(
        [offset.y.round(), offset.height.round()],
        [9.0, 16.0],
        "offsetTop and offsetHeight, Chrome 9 and 16"
    );
}
