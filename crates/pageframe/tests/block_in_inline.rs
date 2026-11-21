use pageframe::{App, Page};
use pageframe_test_harness::Harness;

const BODY: &str = "<div id=a class=c><span id=as>a<div id=ad>b</div>c</span></div>\
<div id=e class=c><span id=es style=\"position:relative\"><div id=ed>y</div></span>after</div>\
<div id=h class=c><span id=hs style=\"position:relative;left:10px\"><div id=hd \
style=\"background:#eee\">y</div></span></div>\
<div id=j class=c>ab<span id=js><div id=jd>y</div></span>cd</div>";

fn page() -> Harness<Page<()>> {
    let html = format!(
        "<!doctype html><html><head><style>html,body{{margin:0;font:14px Arial}}\
.c{{width:300px;margin-bottom:6px}}</style></head><body>{BODY}</body></html>"
    );
    let page = App::new((), &html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    harness
}

fn rect(harness: &Harness<Page<()>>, id: &str) -> [f64; 4] {
    let doc = harness.base();
    let node_id = doc
        .query_selector_all(&format!("#{id}"))
        .expect("selector is valid")[0];
    let r = doc
        .get_client_bounding_rect(node_id)
        .expect("element has a box");
    drop(doc);
    [r.x, r.y, r.width, r.height]
}

#[test]
fn block_splits_its_inline_parent_into_lines_around_it() {
    let harness = page();
    for (id, expected) in [
        ("a", [0.0, 0.0, 300.0, 48.0]),
        ("as", [0.0, 0.0, 300.0, 48.0]),
        ("ad", [0.0, 16.0, 300.0, 16.0]),
        ("j", [0.0, 114.0, 300.0, 48.0]),
        ("js", [0.0, 130.0, 300.0, 16.0]),
        ("jd", [0.0, 130.0, 300.0, 16.0]),
    ] {
        assert_eq!(rect(&harness, id), expected, "{id} client rect");
    }
}

#[test]
fn empty_inline_parts_around_a_block_take_no_line() {
    let harness = page();
    for (id, expected) in [
        ("e", [0.0, 54.0, 300.0, 32.0]),
        ("es", [0.0, 54.0, 300.0, 16.0]),
        ("ed", [0.0, 54.0, 300.0, 16.0]),
    ] {
        assert_eq!(rect(&harness, id), expected, "{id} client rect");
    }
}

#[test]
fn relative_inline_offsets_its_block_child() {
    let harness = page();
    for (id, expected) in [
        ("h", [0.0, 92.0, 300.0, 16.0]),
        ("hs", [10.0, 92.0, 300.0, 16.0]),
        ("hd", [10.0, 92.0, 300.0, 16.0]),
    ] {
        assert_eq!(rect(&harness, id), expected, "{id} client rect");
    }
}
