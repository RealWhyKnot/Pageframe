use pageframe::{App, Page};
use pageframe_test_harness::Harness;

const BODY: &str = "<div dir=rtl><input type=tel id=tel><input type=text id=txt></div>\
<table id=b1 border=1><tr><td id=b1c>x</td></tr></table>\
<table id=b0 border=0><tr><td id=b0c>x</td></tr></table>\
<table id=be border=\"\"><tr><td id=bec>x</td></tr></table>\
<table id=bf border=1 frame=void><tr><td id=bfc>x</td></tr></table>\
<table id=ca align=center style=\"width:300px\"><caption id=cl align=left style=\"width:50px\">L\
</caption><tr><td>x</td></tr></table>\
<table id=cb align=center style=\"width:300px\"><caption id=cr align=right style=\"width:50px\">R\
</caption><tr><td>x</td></tr></table>\
<table id=ft><form id=f1><tr><td>x</td></tr></form></table>\
<div style=\"width:300px\"><marquee id=mq scrollamount=0>m</marquee></div>\
<table id=ra border=1 rules=all><tr><td id=rac>x</td><td>y</td></tr></table>";

const UNSUPPORTED: &[&str] = &[
    ":-moz-",
    "::-moz-",
    ":dir(",
    "-moz-top-layer",
    "-moz-user-input",
    "-moz-available",
    "-moz-bool-pref",
    "-moz-platform",
];

fn page() -> Harness<Page<()>> {
    let html = format!(
        "<!doctype html><html><head><style>html,body{{margin:0;font:14px Arial}}</style></head>\
<body>{BODY}</body></html>"
    );
    let page = App::new((), &html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    harness
}

fn rect(harness: &Harness<Page<()>>, id: &str) -> Option<[f64; 4]> {
    let doc = harness.base();
    let node_id = doc
        .query_selector_all(&format!("#{id}"))
        .expect("selector is valid")[0];
    let r = doc.get_client_bounding_rect(node_id);
    drop(doc);
    r.map(|r| [r.x, r.y, r.width, r.height])
}

fn without_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        let end = rest[start..]
            .find("*/")
            .map_or(rest.len(), |end| start + end + 2);
        out.extend(rest[start..end].chars().filter(|&c| c == '\n'));
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

#[test]
fn default_stylesheet_has_no_gecko_only_syntax() {
    let css = without_comments(pageframe_dom::DEFAULT_CSS);
    for (index, line) in css.lines().enumerate() {
        for token in UNSUPPORTED {
            assert!(
                !line.contains(token),
                "default.css:{} uses {token}: {line}",
                index + 1
            );
        }
    }
}

#[test]
fn table_border_attribute_draws_table_and_cell_borders() {
    let harness = page();
    for (id, expected) in [
        ("b1", [0.0, 21.0, 17.0, 26.0]),
        ("b1c", [3.0, 24.0, 11.0, 20.0]),
        ("b0", [0.0, 47.0, 13.0, 22.0]),
        ("b0c", [2.0, 49.0, 9.0, 18.0]),
        ("be", [0.0, 69.0, 17.0, 26.0]),
        ("bec", [3.0, 72.0, 11.0, 20.0]),
        ("bf", [0.0, 95.0, 15.0, 24.0]),
        ("bfc", [2.0, 97.0, 11.0, 20.0]),
    ] {
        assert_eq!(rect(&harness, id), Some(expected), "{id} client rect");
    }
}

#[test]
fn aligned_captions_stay_at_the_inline_start() {
    let harness = page();
    for (table, caption) in [("ca", "cl"), ("cb", "cr")] {
        let table = rect(&harness, table).expect("table has a box");
        let caption = rect(&harness, caption).expect("caption has a box");
        assert_eq!(caption[0], table[0], "{caption:?} x");
        assert_eq!(caption[2], 50.0, "{caption:?} width");
    }
}

#[test]
fn form_inside_table_generates_no_box() {
    let harness = page();
    assert_eq!(rect(&harness, "f1"), None);
}

#[test]
fn marquee_fills_its_container() {
    let harness = page();
    let marquee = rect(&harness, "mq").expect("marquee has a box");
    assert_eq!([marquee[2], marquee[3]], [300.0, 16.0]);
}

#[test]
fn border_attribute_outranks_rules_attribute() {
    let harness = page();
    let table = rect(&harness, "ra").expect("table has a box");
    let cell = rect(&harness, "rac").expect("cell has a box");
    assert_eq!([table[2], table[3]], [21.0, 20.0], "table size");
    assert_eq!([cell[2], cell[3]], [10.0, 19.0], "cell size");
}
