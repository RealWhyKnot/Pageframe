mod support;

use pageframe::App;
use support::{open, pixel, render};

const HIGHLIGHT: [u8; 4] = [180, 213, 255, 255];
const BLANK: [u8; 4] = [255, 255, 255, 255];

const PAGE: &str = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial;color:transparent}</style>\
<div>ab<ruby>XY<rt>zz</rt></ruby>cd</div>";

fn labels(doc: &pageframe_dom::BaseDocument) -> Vec<String> {
    doc.get_text_selection_ranges()
        .into_iter()
        .map(|(node, start, end)| {
            let element = doc
                .get_node(node)
                .and_then(|node| {
                    let mut cursor = Some(node);
                    while let Some(current) = cursor {
                        if let Some(data) = current.element_data() {
                            return Some(data.name.local.to_string());
                        }
                        cursor = current.parent.and_then(|parent| doc.get_node(parent));
                    }
                    None
                })
                .unwrap_or_default();
            format!("{element}:{start}..{}", end.min(64))
        })
        .collect()
}

#[test]
fn selecting_across_ruby_selects_its_base_and_annotation_text() {
    let mut harness = open(App::new((), PAGE));
    let mut doc = harness.base_mut();
    let div = doc
        .query_selector("div")
        .expect("valid selector")
        .expect("the div exists");
    doc.set_text_selection(div, 0, div, usize::MAX);
    let selected = labels(&doc);
    drop(doc);
    assert_eq!(selected, ["div:0..64", "div:0..2", "rt:0..2"]);
}

#[test]
fn a_selection_ending_inside_ruby_keeps_only_what_comes_before_it() {
    let mut harness = open(App::new((), PAGE));
    let mut doc = harness.base_mut();
    let div = doc
        .query_selector("div")
        .expect("valid selector")
        .expect("the div exists");
    let rt = doc
        .query_selector("rt")
        .expect("valid selector")
        .expect("the rt exists");
    doc.set_text_selection(div, 1, rt, 1);
    let into_annotation = labels(&doc);
    let base = doc
        .get_text_selection_ranges()
        .into_iter()
        .map(|(node, ..)| node)
        .find(|&node| node != div && node != rt)
        .expect("the base is selected");
    doc.set_text_selection(div, 1, base, 1);
    let into_base = labels(&doc);
    drop(doc);
    assert_eq!(
        [into_annotation, into_base],
        [
            vec!["div:1..4", "div:0..2", "rt:0..1"],
            vec!["div:1..4", "div:0..1"],
        ]
    );
}

#[test]
fn the_ruby_line_highlight_leaves_the_annotation_space_like_chrome() {
    let mut harness = open(App::new((), PAGE));
    let mut doc = harness.base_mut();
    let div = doc
        .query_selector("div")
        .expect("valid selector")
        .expect("the div exists");
    doc.set_text_selection(div, 0, div, usize::MAX);
    drop(doc);
    let frame = render(&mut harness);
    assert_eq!(
        pixel(&frame, 5, 2),
        BLANK,
        "above ab, beside the annotation"
    );
    assert_eq!(
        pixel(&frame, 42, 2),
        BLANK,
        "above cd, beside the annotation"
    );
    assert_eq!(pixel(&frame, 25, 2), HIGHLIGHT, "the annotation");
    assert_eq!(
        pixel(&frame, 35, 2),
        HIGHLIGHT,
        "the annotation's line break"
    );
    assert_eq!(pixel(&frame, 5, 6), HIGHLIGHT, "the base line from y 5");
    assert_eq!(pixel(&frame, 5, 4), BLANK, "nothing above the base line");
}
