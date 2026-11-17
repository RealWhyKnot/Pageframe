mod support;

use pageframe::App;
use support::{open, pixel, render};

const BOX: &str = "<span id='box' style='display:inline-block;width:30px;height:10px'></span>";
const OTHER: &str = "<span id='other' style='display:inline-block;width:30px;height:10px'></span>";
const BIG: &str = "<span style='font-size:32px'></span>";
const TALL: &str = "<span style='line-height:40px'></span>";

fn boxes(standards: bool, content: &str, selectors: &[&str]) -> Vec<(f32, f32)> {
    let doctype = if standards { "<!doctype html>" } else { "" };
    let html = format!(
        "{doctype}<style>html,body{{margin:0;font:16px/20px Arial}}</style><div id='line'>{content}</div>"
    );
    let harness = open(App::new((), &html));
    let base = harness.base();
    selectors
        .iter()
        .map(|selector| {
            let id = base
                .query_selector_all(selector)
                .expect("the selector parses")[0];
            let layout = base.get_node(id).expect("the node exists").final_layout();
            (layout.size.height, layout.location.y)
        })
        .collect()
}

#[test]
fn an_empty_span_adds_its_font_metrics_beside_a_box() {
    for content in [format!("{BOX}{BIG}"), format!("{BIG}{BOX}")] {
        assert_eq!(
            boxes(true, &content, &["#line", "#box"]),
            [(26.0, 0.0), (10.0, 11.0)]
        );
    }
}

#[test]
fn an_empty_span_adds_its_font_metrics_beside_text() {
    assert_eq!(boxes(true, &format!("x{BIG}"), &["#line"]), [(26.0, 0.0)]);
}

#[test]
fn an_empty_span_adds_its_line_height() {
    assert_eq!(
        boxes(true, &format!("{BOX}{TALL}"), &["#line", "#box"]),
        [(40.0, 0.0), (10.0, 15.0)]
    );
}

#[test]
fn a_span_with_collapsed_whitespace_keeps_its_font_metrics() {
    assert_eq!(
        boxes(
            true,
            &format!("{BOX}<span style='font-size:32px'> </span>"),
            &["#line", "#box"]
        ),
        [(26.0, 0.0), (10.0, 11.0)]
    );
}

#[test]
fn nested_empty_spans_keep_their_font_metrics() {
    for span in [
        BIG.to_owned(),
        "<span style='font-size:32px'><b></b></span>".to_owned(),
    ] {
        assert_eq!(
            boxes(
                true,
                &format!("{BOX}<span>{span}</span>"),
                &["#line", "#box"]
            ),
            [(26.0, 0.0), (10.0, 11.0)]
        );
    }
}

#[test]
fn an_empty_span_before_a_break_grows_only_its_line() {
    assert_eq!(
        boxes(true, &format!("{BIG}<br>{BOX}"), &["#line", "#box"]),
        [(46.0, 0.0), (10.0, 31.0)]
    );
}

#[test]
fn an_empty_span_after_a_break_does_not_open_a_line() {
    assert_eq!(
        boxes(true, &format!("{BOX}<br>{BIG}"), &["#line", "#box"]),
        [(20.0, 0.0), (10.0, 5.0)]
    );
}

#[test]
fn an_empty_span_stays_on_its_line_when_the_next_box_wraps() {
    assert_eq!(
        boxes(
            true,
            &format!("<div style='width:60px'>{BOX} {BIG}{OTHER}</div>"),
            &["#line", "#box", "#other"]
        ),
        [(46.0, 0.0), (10.0, 11.0), (10.0, 31.0)]
    );
}

#[test]
fn empty_spans_alone_do_not_open_a_line() {
    for content in [BIG, TALL, "<b></b><i></i>"] {
        assert_eq!(boxes(true, content, &["#line"]), [(0.0, 0.0)]);
    }
}

#[test]
fn a_top_aligned_empty_span_keeps_the_line_height() {
    assert_eq!(
        boxes(
            true,
            &format!("{BOX}<span style='font-size:32px;vertical-align:top'></span>"),
            &["#line", "#box"]
        ),
        [(20.0, 0.0), (10.0, 5.0)]
    );
}

#[test]
fn empty_emphasis_keeps_the_line_height() {
    assert_eq!(
        boxes(true, &format!("{BOX}<b></b><i></i>"), &["#line", "#box"]),
        [(20.0, 0.0), (10.0, 5.0)]
    );
}

#[test]
fn empty_spans_do_not_add_struts_in_quirks_mode() {
    for content in [
        format!("{BOX}{BIG}"),
        format!("{BIG}{BOX}"),
        format!("{BOX}{TALL}"),
        format!("{BOX}<span>{BIG}</span>"),
        format!("{BOX}<span style='font-size:32px'> </span>"),
    ] {
        assert_eq!(
            boxes(false, &content, &["#line", "#box"]),
            [(10.0, 0.0), (10.0, 0.0)]
        );
    }
    assert_eq!(boxes(false, &format!("x{BIG}"), &["#line"]), [(20.0, 0.0)]);
    assert_eq!(
        boxes(false, &format!("{BIG}<br>{BOX}"), &["#line", "#box"]),
        [(30.0, 0.0), (10.0, 20.0)]
    );
    assert_eq!(
        boxes(false, &format!("{BOX}<br>{BIG}"), &["#line", "#box"]),
        [(10.0, 0.0), (10.0, 0.0)]
    );
}

#[test]
fn painting_uses_the_baseline_from_the_empty_span() {
    let html = format!(
        "<!doctype html><style>html,body{{margin:0;background:white;font:16px/20px Arial}}#box{{background:lime}}</style>{BOX}{BIG}"
    );
    let mut harness = open(App::new((), &html));
    let pixels = render(&mut harness);
    assert_eq!(pixel(&pixels, 15, 5), [255, 255, 255, 255]);
    assert_eq!(pixel(&pixels, 15, 11), [0, 255, 0, 255]);
    assert_eq!(pixel(&pixels, 15, 20), [0, 255, 0, 255]);
    assert_eq!(pixel(&pixels, 15, 21), [255, 255, 255, 255]);
}

#[test]
fn lines_of_preserved_spaces_keep_their_height_like_chrome() {
    let image = "<img style='display:inline-block;width:10px;height:10px'>";
    let cases = [
        ("pre", " ".to_owned(), 20.0, 20.0),
        ("pre", "   ".to_owned(), 20.0, 20.0),
        ("pre", "\t".to_owned(), 20.0, 20.0),
        ("pre", "<span> </span>".to_owned(), 20.0, 20.0),
        ("pre-wrap", " ".to_owned(), 20.0, 20.0),
        ("break-spaces", " ".to_owned(), 20.0, 20.0),
        ("normal", " ".to_owned(), 0.0, 0.0),
        ("pre-line", " ".to_owned(), 0.0, 0.0),
        (
            "normal",
            " <span style='white-space:pre'> </span>".to_owned(),
            20.0,
            20.0,
        ),
        (
            "pre-line",
            "<span style='white-space:pre'> </span>".to_owned(),
            20.0,
            20.0,
        ),
        ("pre", format!("{image} "), 20.0, 20.0),
        ("pre", format!("{image}\n"), 10.0, 20.0),
        ("pre", format!("{image} \n{image}"), 30.0, 40.0),
        ("pre", format!("{image}\n{image}"), 20.0, 40.0),
    ];
    for (mode, inner, quirks, standard) in &cases {
        for (standards, expected) in [(true, *standard), (false, *quirks)] {
            let content = format!("<div id='spaces' style='white-space:{mode}'>{inner}</div>");
            let found = boxes(standards, &content, &["#spaces"])[0].0;
            assert_eq!(
                found, expected,
                "{inner:?} under white-space:{mode}, standards {standards}"
            );
        }
    }
}
