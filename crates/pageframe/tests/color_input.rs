mod support;

use pageframe::App;
use support::{open, pixel, render};

const CONTENT: &str = "<!doctype html><style>html,body{margin:0;font:16px Arial}</style>\
<input type=color value='#ff0000' id='red'>\
<input type=color value='#0000FF' id='blue' disabled>\
<div><input type=color id='tall' style='height:40px'>\
<span id='mark' style='display:inline-block;width:5px;height:10px'></span></div>";

#[test]
fn color_inputs_size_and_align_like_chrome() {
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
    assert_eq!(layout("#red"), (0.0, 0.0, 50.0, 27.0));
    assert_eq!(layout("#blue"), (50.0, 0.0, 50.0, 27.0));
    assert_eq!(layout("#tall"), (0.0, 0.0, 50.0, 40.0));
    assert_eq!(layout("#mark"), (50.0, 24.0, 5.0, 10.0));
}

#[test]
fn color_inputs_paint_a_swatch_of_their_value() {
    let mut harness = open(App::new((), CONTENT));
    let buffer = render(&mut harness);
    let samples = [
        ((2, 2), [239, 239, 239]),
        ((4, 5), [239, 239, 239]),
        ((5, 6), [119, 119, 119]),
        ((25, 13), [255, 0, 0]),
        ((45, 21), [239, 239, 239]),
        ((52, 5), [238, 238, 238]),
        ((55, 6), [119, 119, 119]),
        ((75, 13), [0, 0, 255]),
        ((4, 31), [239, 239, 239]),
        ((25, 47), [0, 0, 0]),
    ];
    for ((column, row), expected) in samples {
        let [red, green, blue, _] = pixel(&buffer, column, row);
        assert_eq!([red, green, blue], expected, "pixel at ({column}, {row})");
    }
}
