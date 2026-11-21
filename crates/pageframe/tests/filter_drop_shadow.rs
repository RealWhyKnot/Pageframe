mod support;

use pageframe::App;
use support::{open, pixel, render};

#[test]
fn a_blurred_drop_shadow_reaches_back_past_its_offset_like_chromes() {
    let html = "<!DOCTYPE html><html><head><style>html,body{margin:0;background:white}</style></head><body><div style='position:absolute;left:0;top:0;width:40px;height:30px;background:#36c;filter:drop-shadow(6px 6px 3px #000)'></div></body></html>";
    let mut harness = open(App::new((), html));
    let buffer = render(&mut harness);
    for (x, y, chrome) in [
        (0, 32, 246),
        (1, 32, 238),
        (3, 32, 206),
        (6, 33, 144),
        (0, 35, 249),
        (2, 35, 235),
        (11, 30, 20),
    ] {
        let red = pixel(&buffer, x, y)[0];
        assert!(
            red.abs_diff(chrome) <= 2,
            "({x},{y}) red {red}, Chrome {chrome}"
        );
    }
}

#[test]
fn a_drop_shadow_pushed_past_the_page_leaves_the_box_alone() {
    for offset in ["0 -9223372036854775808px", "100000000000000000000px 0"] {
        let html = format!(
            "<!DOCTYPE html><html><head><style>html,body{{margin:0;background:white}}</style></head><body><div style='position:absolute;left:0;top:0;width:40px;height:30px;background:rgba(0,0,255,0.5);filter:drop-shadow({offset} 0 #000)'></div></body></html>"
        );
        let mut harness = open(App::new((), &html));
        let buffer = render(&mut harness);
        let red = pixel(&buffer, 20, 15)[0];
        assert!(red.abs_diff(128) <= 1, "{offset}: red {red}");
    }
}
