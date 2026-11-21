mod support;

use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use support::{open, pixel, render};

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

fn page(container_style: &str, content: &str) -> String {
    format!(
        "<!doctype html><style>html,body{{margin:0;font:14px Arial;background:#fff}}</style>\
<div id='c' style='width:100px;{container_style}'>{content}</div>"
    )
}

fn rects(container_style: &str, content: &str, selectors: &[&str]) -> Vec<[f32; 4]> {
    let harness = open(App::new((), &page(container_style, content)));
    let base = harness.base();
    selectors
        .iter()
        .map(|selector| {
            let id = base
                .query_selector(selector)
                .expect("the selector parses")
                .expect("the element exists");
            let node = base.get_node(id).expect("the node exists");
            let origin = node.absolute_position(0.0, 0.0);
            let size = node.final_layout().size;
            [origin.x, origin.y, size.width, size.height]
        })
        .collect()
}

fn container_height(container_style: &str, content: &str) -> f32 {
    rects(container_style, content, &["#c"])[0][3]
}

#[test]
fn a_block_inside_a_span_gets_its_own_full_width_line() {
    assert_eq!(
        rects("", "ab<span>x<div id='d'>y</div>z</span>cd", &["#c", "#d"]),
        [[0.0, 0.0, 100.0, 48.0], [0.0, 16.0, 100.0, 16.0]]
    );
}

#[test]
fn lines_around_the_block_match_chrome_heights() {
    for (content, height) in [
        (
            "<span style='position:relative'><div>y</div></span>after",
            32.0,
        ),
        ("ab<span><span>x<div>y</div>z</span>w</span>cd", 48.0),
        (
            "<span style='position:relative;left:10px'><div>y</div></span>",
            16.0,
        ),
        ("ab <span>x <div>y</div> z</span> cd", 48.0),
        ("ab<span><div>y</div></span>cd", 48.0),
        ("ab<span></span>cd", 16.0),
        ("ab<span>x<span></span><div>y</div></span>cd", 48.0),
        (
            "ab<span>x<span>u<div>y</div>v</span><div>w</div>z</span>cd",
            80.0,
        ),
        ("ab<span>x<div>y</div><div>w</div>z</span>cd", 64.0),
        (
            "ab<span style='position:relative;top:2px'>x<div>y</div>z</span>cd",
            48.0,
        ),
    ] {
        assert_eq!(container_height("", content), height, "{content}");
    }
}

#[test]
fn nowrap_still_breaks_around_the_block() {
    for content in [
        "ab<span>x<div>y</div>z</span>cd",
        "ab <span>x <div>y</div> z</span> cd",
    ] {
        assert_eq!(
            container_height("white-space:nowrap", content),
            48.0,
            "{content}"
        );
    }
}

#[test]
fn block_margins_and_span_padding_shape_the_block_line() {
    let padded = "ab<span style='border:2px solid;padding:0 4px'>x<div id='d' style='margin:5px 0'>y</div>z</span>cd";
    assert_eq!(
        rects("", padded, &["#c", "#d"]),
        [[0.0, 0.0, 100.0, 58.0], [0.0, 21.0, 100.0, 16.0]]
    );
    let indented = "<span style='padding:0 4px;border:2px solid'>x<div id='d' style='margin-left:10px'>y</div>z</span>";
    assert_eq!(rects("", indented, &["#d"]), [[10.0, 16.0, 90.0, 16.0]]);
}

#[test]
fn a_sized_block_follows_its_margins_and_direction_not_text_align() {
    let sized =
        |style: &str| format!("ab<span>x<div id='d' style='width:50px;{style}'>y</div>z</span>cd");
    let x = |container: &str, style: &str| rects(container, &sized(style), &["#d"])[0][0];
    assert_eq!(x("", ""), 0.0);
    assert_eq!(x("text-align:center", ""), 0.0);
    assert_eq!(x("", "margin:0 auto"), 25.0);
    assert_eq!(x("direction:rtl", ""), 50.0);
    assert_eq!(x("", "margin-left:auto"), 50.0);
    assert_eq!(x("direction:rtl", "margin-right:auto"), 0.0);
    assert_eq!(x("direction:rtl", "margin-left:auto"), 50.0);
}

#[test]
fn the_block_line_follows_zero_and_max_content_widths() {
    let content = "ab<span>x<div id='d'>y</div>z</span>cd";
    assert_eq!(rects("width:0", content, &["#d"])[0][2], 0.0);
    let fitted = rects("width:max-content", content, &["#c", "#d"]);
    assert_eq!(fitted[1][2], fitted[0][2]);
    assert!(fitted[0][2] > 0.0 && fitted[0][2] < 100.0, "{fitted:?}");
}

fn block_line(harness: &Harness<Page<()>>) -> [f32; 2] {
    let base = harness.base();
    let find = |selector: &str| {
        let id = base
            .query_selector(selector)
            .expect("the selector parses")
            .expect("the element exists");
        base.get_node(id).expect("the node exists")
    };
    [
        find("#c").final_layout().size.height,
        find("#d").absolute_position(0.0, 0.0).y,
    ]
}

#[test]
fn adding_and_removing_span_text_rebuilds_the_block_line() {
    let fresh = |content: &str| block_line(&open(App::new((), &page("", content))));
    let block_only = "ab<span id='s'><div id='d'>y</div></span>cd";
    let mut harness = open(App::new((), &page("", block_only)));
    let before = block_line(&harness);
    assert_eq!(before, fresh(block_only));
    let span = harness
        .base()
        .query_selector("#s")
        .expect("the selector parses")
        .expect("the element exists");
    let text = harness.base_mut().mutate().create_text_node("x");
    harness.base_mut().mutate().append_children(span, &[text]);
    harness.pump();
    assert_eq!(
        block_line(&harness),
        fresh("ab<span id='s'><div id='d'>y</div>x</span>cd")
    );
    harness.base_mut().mutate().remove_and_drop_node(text);
    harness.pump();
    assert_eq!(block_line(&harness), before);
}

#[test]
fn the_span_background_and_border_skip_the_block_line() {
    let content = "ab<span style='background:#f00;border:2px solid #00f;padding:0 4px'>x<div style='margin:5px 0'>&nbsp;</div>z</span>";
    let mut harness = open(App::new((), &page("color:transparent", content)));
    let frame = render(&mut harness);
    assert_eq!(pixel(&frame, 24, 8), RED);
    assert_eq!(pixel(&frame, 12, 45), BLUE);
    assert_eq!(pixel(&frame, 3, 45), RED);
    for x in [1, 8, 40, 98] {
        assert_eq!(pixel(&frame, x, 29), WHITE, "x {x}");
    }
}
