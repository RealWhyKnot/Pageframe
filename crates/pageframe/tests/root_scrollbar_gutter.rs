use pageframe::{App, Page};
use pageframe_test_harness::Harness;

const FLOW: &str = "<div id=d style='height:20px'>x</div><p id=p style='margin:0'>y</p><pre></pre>";

const POSITIONED: &str = "<div id=d style='height:20px'>x</div>\
<div id=f style='position:fixed;left:0;right:0;top:40px;height:10px'></div>\
<div id=a style='position:absolute;left:0;right:0;top:60px;height:10px'></div>\
<div id=w style='width:100%;height:5px'></div><pre></pre>";

type Expected<'a> = &'a [(&'a str, [f64; 4])];

fn check(html_attrs: &str, css: &str, body: &str, expected: Expected<'_>) {
    let source = format!(
        "<!doctype html><html id=h {html_attrs}><meta charset=utf-8><style>\
         html,body{{margin:0;font:14px Arial}}{css}</style><body id=b>{body}"
    );
    let page: Page<()> = App::new((), &source)
        .page(624, 305)
        .expect("the page parses");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let doc = harness.base();
    for (id, want) in expected {
        let node = doc
            .query_selector(&format!("#{id}"))
            .expect("valid selector")
            .expect("the element exists");
        let rect = doc
            .get_client_bounding_rect(node)
            .expect("the element has a box");
        let found = [rect.x, rect.y, rect.width, rect.height];
        assert!(
            found.iter().zip(want).all(|(a, b)| (a - b).abs() <= 0.01),
            "{css} {html_attrs} #{id}: found {found:?}, Chrome 157 has {want:?}"
        );
    }
    drop(doc);
}

#[test]
fn stable_gutter_takes_the_right_edge_of_the_viewport() {
    let root = [0.0, 0.0, 609.0, 50.0];
    let paragraph = [0.0, 20.0, 609.0, 16.0];
    for css in [
        "html{scrollbar-gutter:stable}",
        "html{overflow:auto;scrollbar-gutter:stable}",
        "html{overflow:hidden;scrollbar-gutter:stable}",
        "html{overflow:clip;scrollbar-gutter:stable}",
    ] {
        check("", css, FLOW, &[("h", root), ("p", paragraph)]);
    }
    let rtl = "html{scrollbar-gutter:stable}";
    check("dir=rtl", rtl, FLOW, &[("h", root), ("p", paragraph)]);
    let tall = "<div id=d style='height:900px'>x</div><pre></pre>";
    check("", rtl, tall, &[("d", [0.0, 0.0, 609.0, 900.0])]);
}

#[test]
fn both_edges_reserves_a_gutter_on_each_side() {
    let root = [15.0, 0.0, 594.0, 50.0];
    let paragraph = [15.0, 20.0, 594.0, 16.0];
    let both = "html{scrollbar-gutter:stable both-edges}";
    let scroll = "html{overflow:scroll;scrollbar-gutter:stable both-edges}";
    for (attrs, css) in [
        ("", both),
        ("dir=rtl", both),
        ("", scroll),
        ("dir=rtl", scroll),
    ] {
        check(attrs, css, FLOW, &[("h", root), ("p", paragraph)]);
    }
}

#[test]
fn body_gutter_stays_on_the_body() {
    let css = "body{overflow:auto;scrollbar-gutter:stable}";
    check("", css, FLOW, &[("h", [0.0, 0.0, 624.0, 50.0])]);
}

#[test]
fn root_insets_sit_inside_the_gutter() {
    let bordered = "html{scrollbar-gutter:stable;padding:10px;border:2px solid}";
    check(
        "",
        bordered,
        FLOW,
        &[
            ("h", [0.0, 0.0, 609.0, 74.0]),
            ("p", [12.0, 32.0, 585.0, 16.0]),
        ],
    );
    let margined = "html{scrollbar-gutter:stable both-edges;margin:0 7px}";
    check(
        "",
        margined,
        POSITIONED,
        &[
            ("b", [22.0, 0.0, 580.0, 25.0]),
            ("f", [15.0, 40.0, 594.0, 10.0]),
            ("a", [15.0, 60.0, 594.0, 10.0]),
        ],
    );
}

#[test]
fn positioned_boxes_fill_the_space_between_gutters() {
    check(
        "",
        "html{scrollbar-gutter:stable both-edges}",
        POSITIONED,
        &[
            ("f", [15.0, 40.0, 594.0, 10.0]),
            ("a", [15.0, 60.0, 594.0, 10.0]),
            ("w", [15.0, 20.0, 594.0, 5.0]),
        ],
    );
}

#[test]
fn scrollbar_width_sets_the_gutter_size() {
    let thin = "html{scrollbar-gutter:stable;scrollbar-width:thin}";
    check("", thin, POSITIONED, &[("b", [0.0, 0.0, 614.0, 25.0])]);
    let none = "html{scrollbar-gutter:stable both-edges;scrollbar-width:none}";
    check("", none, POSITIONED, &[("b", [0.0, 0.0, 624.0, 25.0])]);
}

#[test]
fn vertical_root_reserves_the_top_and_bottom() {
    check(
        "",
        "html{scrollbar-gutter:stable both-edges;writing-mode:vertical-rl}",
        POSITIONED,
        &[
            ("b", [608.0, 15.0, 16.0, 275.0]),
            ("f", [0.0, 55.0, 624.0, 10.0]),
            ("a", [0.0, 75.0, 624.0, 10.0]),
        ],
    );
}
