mod support;

use pageframe::App;
use support::{open, pixel, render};

const CONTENT: &str = "<!doctype html><style>html,body{margin:0;background:#fff}\
input{margin:0;width:100px;display:block}</style>\
<input type=range id='half'><input type=range id='quarter' value=25 disabled>\
<input type=range id='tall' value=70 style='accent-color:#c00;height:30px'>\
<div style='width:200px'><input type=range id='plain' \
style='display:inline-block;margin:2px;width:auto'>\
<span id='mark' style='display:inline-block;width:5px;height:10px'></span></div>";

const VALUES: &str = "<!doctype html><input type=range value=' 30'>\
<input type=range value='+30'><input type=range step=7 value=50>\
<input type=range min=20 max=10><input type=range step=30 value=100>\
<input type=range step=any value=33.3><input type=range value=1e1>\
<input type=range value='.5' step=any><input type=range value='5.'>\
<input type=range min=-50 max=50><input type=range value=1000>\
<input type=range value=infinity><input type=range step=3 min=1 value=5>\
<input type=range step=-2 value=33.3>";

#[test]
fn range_inputs_size_and_align_like_chrome() {
    let harness = open(App::new((), CONTENT));
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
    assert_eq!(layout("#half"), (0.0, 0.0, 100.0, 16.0));
    assert_eq!(layout("#tall"), (0.0, 32.0, 100.0, 30.0));
    assert_eq!(layout("#plain"), (2.0, 2.0, 129.0, 16.0));
    assert_eq!(layout("#mark"), (133.0, 8.0, 5.0, 10.0));
}

#[test]
fn range_inputs_paint_track_and_thumb_like_chrome() {
    let mut harness = open(App::new((), CONTENT));
    let buffer = render(&mut harness);
    let samples = [
        ((3, 8), [0x00, 0x75, 0xff]),
        ((20, 4), [0x3b, 0x75, 0xba]),
        ((80, 8), [0xef, 0xef, 0xef]),
        ((80, 4), [0xb2, 0xb2, 0xb2]),
        ((50, 1), [0x00, 0x75, 0xff]),
        ((99, 8), [0xff, 0xff, 0xff]),
        ((60, 20), [0xd2, 0xd2, 0xd2]),
        ((26, 24), [0xcb, 0xcb, 0xcb]),
        ((60, 47), [0xcc, 0x00, 0x00]),
        ((75, 47), [0xef, 0xef, 0xef]),
        ((75, 40), [0xff, 0xff, 0xff]),
    ];
    for ((column, row), expected) in samples {
        let [red, green, blue, _] = pixel(&buffer, column, row);
        let ours = [red, green, blue];
        let off = ours
            .iter()
            .zip(expected)
            .map(|(channel, chrome)| channel.abs_diff(chrome))
            .max()
            .expect("three channels");
        assert!(
            off <= 2,
            "pixel at ({column}, {row}) is {ours:?}, Chrome has {expected:?}"
        );
    }
}

#[test]
fn range_values_sanitize_like_chrome() {
    let harness = open(App::new((), VALUES));
    let fractions: Vec<f64> = {
        let base = harness.base();
        base.query_selector_all("input")
            .expect("the selector parses")
            .into_iter()
            .map(|id| {
                base.get_node(id)
                    .expect("the node exists")
                    .element_data()
                    .expect("inputs are elements")
                    .range_input_fraction()
            })
            .collect()
    };
    let chrome = [
        0.5,
        0.5,
        0.5,
        0.0,
        1.0,
        0.333,
        0.1,
        0.005,
        0.5,
        0.5,
        1.0,
        0.5,
        3.0 / 99.0,
        0.333,
    ];
    assert_eq!(fractions.len(), chrome.len());
    for (index, (ours, expected)) in fractions.iter().zip(chrome).enumerate() {
        assert!(
            (ours - expected).abs() < 1e-9,
            "input {index}: {ours} vs Chrome {expected}"
        );
    }
}
