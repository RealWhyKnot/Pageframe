mod support;

use pageframe::App;
use support::{open, pixel, render};

const BASE: &str =
    "html,body{margin:0;background:white;font:10px/20px monospace;color:transparent}";
const RED: [u8; 4] = [255, 0, 0, 255];

fn underlined_lines(body: &str) -> Vec<bool> {
    let html = format!("<html><head><style>{BASE}</style></head><body>{body}</body></html>");
    let mut harness = open(App::new((), &html));
    harness.pump();
    let buffer = render(&mut harness);
    (0..3)
        .map(|line| {
            (line * 20..line * 20 + 20)
                .any(|down| (0..60).any(|across| pixel(&buffer, across, down) == RED))
        })
        .collect()
}

#[test]
fn a_block_underline_reaches_text_in_block_children() {
    assert_eq!(
        underlined_lines(
            "<div style=\"text-decoration:underline red\">aaa<div>bbb</div>\
             <div><div>ccc</div></div></div>"
        ),
        [true, true, true]
    );
}

#[test]
fn a_span_underline_reaches_blocks_inside_it() {
    assert_eq!(
        underlined_lines(
            "<span style=\"text-decoration:underline red\">aaa<div>bbb</div>ccc</span>"
        ),
        [true, true, true]
    );
}

#[test]
fn underlines_stop_at_atomic_and_out_of_flow_boxes() {
    for child in [
        "display:inline-block",
        "display:inline-flex",
        "float:left",
        "position:absolute;top:0",
    ] {
        assert_eq!(
            underlined_lines(&format!(
                "<div style=\"text-decoration:underline red\">\
                 <div style=\"{child}\">aaa</div></div>"
            )),
            [false, false, false],
            "{child}"
        );
    }
}

fn underlined_sides(body: &str) -> (bool, bool) {
    let html = format!(
        "<html><head><style>{BASE}ul,ol{{margin:0;padding-left:60px}}</style></head>\
         <body>{body}</body></html>"
    );
    let mut harness = open(App::new((), &html));
    harness.pump();
    let buffer = render(&mut harness);
    let red = |columns: std::ops::Range<usize>| {
        (0..20).any(|down| {
            columns
                .clone()
                .any(|across| pixel(&buffer, across, down) == RED)
        })
    };
    (red(0..60), red(60..120))
}

#[test]
fn underlines_skip_outside_list_markers() {
    for body in [
        "<div style=\"text-decoration:underline red\"><ul><li>aaa</li></ul></div>",
        "<div style=\"text-decoration:underline red\"><ol><li>aaa</li></ol></div>",
        "<ul style=\"text-decoration:underline red\"><li>aaa</li></ul>",
        "<ol style=\"text-decoration:underline red\"><li>aaa</li></ol>",
        "<ol><li style=\"text-decoration:underline red\">aaa</li></ol>",
    ] {
        assert_eq!(underlined_sides(body), (false, true), "{body}");
    }
}
