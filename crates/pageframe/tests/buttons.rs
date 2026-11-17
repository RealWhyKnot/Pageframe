mod support;

use pageframe::App;
use support::{open, pixel, render};

const CONTENT: &str = "<!doctype html><style>html,body{margin:0}</style>\
<div style=\"background:#000;padding:5px;height:30px\">\
<button id=off disabled style=\"width:50px;height:30px\"></button>\
<button id=on style=\"width:50px;height:30px\"></button></div>\
<div style=\"padding:5px\">\
<button id=tint style=\"width:50px;height:30px;background:rgb(204,221,238)\"></button></div>";

#[test]
fn buttons_size_their_border_box_like_chrome() {
    let harness = open(App::new((), CONTENT));
    let sizes = {
        let base = harness.base();
        ["#off", "#on", "#tint"].map(|selector| {
            let id = base
                .query_selector_all(selector)
                .expect("the selector parses")[0];
            let size = base
                .get_node(id)
                .expect("the node exists")
                .final_layout()
                .size;
            (selector, size.width, size.height)
        })
    };
    for (selector, width, height) in sizes {
        assert_eq!((width, height), (50.0, 30.0), "{selector}");
    }
}

const ROWS: &str = "<!doctype html><style>html,body{margin:0;font:14px Arial}\
p{margin:0;white-space:nowrap}input,button{width:40px}</style>\
<p id=row><input type=email><button>Go</button></p>\
<p><input id=big style=\"font-size:20px\"><input id=send type=submit value=Go></p>";

#[test]
fn buttons_share_the_text_field_baseline_like_chrome() {
    let harness = open(App::new((), ROWS));
    let [row, big, send] = {
        let base = harness.base();
        ["#row", "#big", "#send"].map(|selector| {
            let id = base
                .query_selector_all(selector)
                .expect("the selector parses")[0];
            let layout = base
                .get_node(id)
                .expect("the node exists")
                .unrounded_layout();
            (layout.location.y, layout.size.height)
        })
    };
    assert_eq!(row, (0.0, 21.0));
    assert_eq!(big.0, 0.0);
    assert_eq!(send.0, 6.0);
}

#[test]
fn buttons_paint_the_native_look_until_styled_like_chrome() {
    let buffer = render(&mut open(App::new((), CONTENT)));
    let samples = [
        ((5, 20), [70, 70, 70]),
        ((30, 5), [70, 70, 70]),
        ((30, 20), [95, 95, 95]),
        ((55, 20), [118, 118, 118]),
        ((80, 5), [118, 118, 118]),
        ((80, 20), [239, 239, 239]),
        ((104, 20), [118, 118, 118]),
        ((5, 60), [168, 168, 168]),
        ((6, 60), [168, 168, 168]),
        ((30, 46), [168, 168, 168]),
        ((30, 74), [84, 84, 84]),
        ((53, 60), [84, 84, 84]),
        ((30, 60), [204, 221, 238]),
    ];
    for ((x, y), chrome) in samples {
        let [r, g, b, a] = pixel(&buffer, x, y);
        let cover = 255 - a;
        let actual = [
            r.saturating_add(cover),
            g.saturating_add(cover),
            b.saturating_add(cover),
        ];
        let off = (0..3)
            .map(|channel| actual[channel].abs_diff(chrome[channel]))
            .max()
            .unwrap_or(0);
        assert!(
            off <= 3,
            "pixel ({x}, {y}) is {actual:?}, Chrome draws {chrome:?}"
        );
    }
    let [corner, ..] = pixel(&buffer, 55, 5);
    assert!(
        corner < 40,
        "the native corner at (55, 5) is {corner}, Chrome rounds it off to 0"
    );
}
