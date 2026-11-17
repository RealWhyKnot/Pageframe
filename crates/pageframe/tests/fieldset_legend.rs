mod support;

use pageframe::App;
use support::{open, pixel, render};

const FIELDSETS: &str = "<!doctype html><style>body{margin:0;background:#fff}\
fieldset{margin:0;padding:4px;border:2px solid #000;width:100px}legend{font-size:0}\
legend span{display:inline-block;width:30px;height:18px}</style>\
<fieldset id=f1 style='background:#0f0'><legend id=l1><span></span></legend>\
<div id=c1 style='height:10px'></div></fieldset>\
<fieldset id=f2 style='border-width:10px'><legend id=l2><span style='height:6px'></span>\
</legend><div id=c2 style='height:10px'></div></fieldset>\
<fieldset id=f3><legend id=l3 style='margin:0 auto'><span></span></legend>\
<div id=c3 style='height:10px'></div></fieldset>\
<div><fieldset id=f4 style='display:inline-block'><legend id=l4><span></span></legend>\
<div id=c4 style='height:10px'></div></fieldset></div>";

const FLOATS_AND_SHADOWS: &str = "<!doctype html><style>body{margin:0;background:#fff}\
fieldset{margin:0;padding:0;border:2px solid #000}legend{font-size:0;padding:0}\
legend span{display:inline-block;width:20px;height:10px}</style>\
<div style='float:left;width:30px;height:30px;background:#00f'></div>\
<fieldset id=a><div style='height:4px'></div></fieldset><div style='clear:both'></div>\
<fieldset id=b style='margin:4px;box-shadow:0 0 0 4px #f00;width:60px'><legend id=bl>\
<span></span></legend><div style='height:10px'></div></fieldset>\
<div style='display:flex'><fieldset id=c><legend id=cl><span></span></legend>\
<div id=cc style='height:6px;width:40px'></div></fieldset></div>";

type Expected = (&'static str, Option<f32>, Option<f32>, f32, f32);

fn layout_mismatches(page: &'static str, chrome: &[Expected]) -> Vec<String> {
    let harness = open(App::new((), page));
    let base = harness.base();
    let layout = |selector: &str| {
        let id = base
            .query_selector_all(selector)
            .expect("the selector parses")[0];
        let layout = base.get_node(id).expect("the node exists").final_layout();
        (
            layout.location.x,
            layout.location.y,
            layout.size.width,
            layout.size.height,
        )
    };
    let mismatches = chrome
        .iter()
        .filter_map(|&(selector, left, top, width, height)| {
            let ours = layout(selector);
            let matches = left.is_none_or(|left| ours.0 == left)
                && top.is_none_or(|top| ours.1 == top)
                && ours.2 == width
                && ours.3 == height;
            (!matches).then(|| {
                format!("{selector} is {ours:?}, Chrome has {left:?} {top:?} {width} {height}")
            })
        })
        .collect();
    drop(base);
    mismatches
}

fn assert_pixels(page: &'static str, samples: &[((usize, usize), [u8; 3])]) {
    let mut harness = open(App::new((), page));
    let buffer = render(&mut harness);
    for &((column, row), expected) in samples {
        let [red, green, blue, _] = pixel(&buffer, column, row);
        assert_eq!(
            [red, green, blue],
            expected,
            "pixel at ({column}, {row}) differs from Chrome"
        );
    }
}

#[test]
fn legends_sit_on_the_top_border_like_chrome() {
    let chrome = [
        ("#f1", None, None, 112.0, 38.0),
        ("#l1", Some(6.0), Some(0.0), 34.0, 18.0),
        ("#c1", Some(6.0), Some(22.0), 100.0, 10.0),
        ("#f2", None, None, 128.0, 38.0),
        ("#l2", Some(14.0), Some(2.0), 34.0, 6.0),
        ("#c2", Some(14.0), Some(14.0), 100.0, 10.0),
        ("#f3", None, None, 112.0, 38.0),
        ("#l3", Some(39.0), Some(0.0), 34.0, 18.0),
        ("#c3", Some(6.0), Some(22.0), 100.0, 10.0),
        ("#f4", None, None, 112.0, 38.0),
        ("#l4", Some(6.0), Some(0.0), 34.0, 18.0),
        ("#c4", Some(6.0), Some(22.0), 100.0, 10.0),
    ];
    let mismatches = layout_mismatches(FIELDSETS, &chrome);
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn fieldsets_avoid_floats_and_size_legends_as_flex_items_like_chrome() {
    let chrome = [
        ("#a", Some(30.0), Some(0.0), 90.0, 8.0),
        ("#b", Some(4.0), Some(34.0), 64.0, 22.0),
        ("#bl", Some(2.0), Some(0.0), 20.0, 10.0),
        ("#c", Some(0.0), Some(0.0), 44.0, 18.0),
        ("#cl", Some(2.0), Some(0.0), 20.0, 10.0),
        ("#cc", Some(2.0), Some(10.0), 40.0, 6.0),
    ];
    let mismatches = layout_mismatches(FLOATS_AND_SHADOWS, &chrome);
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn fieldset_border_breaks_behind_the_legend_like_chrome() {
    assert_pixels(
        FIELDSETS,
        &[
            ((60, 4), [0xff, 0xff, 0xff]),
            ((20, 4), [0xff, 0xff, 0xff]),
            ((60, 8), [0x00, 0x00, 0x00]),
            ((60, 9), [0x00, 0x00, 0x00]),
            ((20, 9), [0x00, 0xff, 0x00]),
            ((60, 12), [0x00, 0xff, 0x00]),
            ((1, 30), [0x00, 0x00, 0x00]),
            ((5, 30), [0x00, 0xff, 0x00]),
            ((60, 37), [0x00, 0x00, 0x00]),
        ],
    );
}

#[test]
fn fieldset_shadow_follows_the_border_under_the_legend_like_chrome() {
    assert_pixels(
        FLOATS_AND_SHADOWS,
        &[
            ((40, 32), [0xff, 0xff, 0xff]),
            ((40, 34), [0xff, 0x00, 0x00]),
            ((40, 37), [0xff, 0x00, 0x00]),
            ((40, 38), [0x00, 0x00, 0x00]),
            ((15, 38), [0xff, 0xff, 0xff]),
            ((70, 45), [0xff, 0x00, 0x00]),
            ((31, 2), [0x00, 0x00, 0x00]),
        ],
    );
}
