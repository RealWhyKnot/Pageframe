use pageframe::{App, Page};
use pageframe_test_harness::Harness;

const CLIPPING_BODY: &str = "<!doctype html><style>html{margin:0;background:#fff}\
body{margin:20px;overflow:hidden;height:60px}</style><body id=bd>\
<div id=a style=\"margin-left:-15px;width:60px;height:20px\"></div>\
<div id=b style=\"position:relative;left:-15px;width:60px;height:20px\"></div>\
<div id=c style=\"width:60px;height:60px\"></div>";

const FORMATTING_BODY: &str = "<!doctype html><style>html{margin:0;background:#fff}\
body{margin:0;overflow:hidden}</style><body id=bd>\
<div id=m style=\"margin-top:30px;height:20px;width:60px\"></div>\
<div id=f style=\"float:left;width:40px;height:40px\"></div>";

fn open(page: &str) -> Harness<Page<()>> {
    let page = App::new((), page)
        .page(320, 240)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    harness
}

fn node(harness: &Harness<Page<()>>, selector: &str) -> pageframe_dom::NodeId {
    harness
        .base()
        .query_selector(selector)
        .expect("valid selector")
        .expect("the element exists")
}

fn set_style(harness: &mut Harness<Page<()>>, selector: &str, style: &str) {
    let node = node(harness, selector);
    harness.base_mut().mutate().set_attribute(
        node,
        pageframe_dom::QualName::new(
            None,
            pageframe_dom::ns!(),
            pageframe_dom::local_name!("style"),
        ),
        style,
    );
    harness.pump();
}

fn hit_id(harness: &Harness<Page<()>>, x: f32, y: f32) -> Option<String> {
    let doc = harness.base();
    let hit = doc.hit(x, y)?;
    let mut node = doc.get_node(hit.node_id)?;
    loop {
        if let Some(id) = node.element_data().and_then(|element| element.id.as_ref()) {
            return Some(id.to_string());
        }
        node = doc.get_node(node.parent?)?;
    }
}

fn hits(harness: &Harness<Page<()>>) -> Vec<Option<String>> {
    [
        (6.25, 30.25),
        (6.25, 50.25),
        (30.25, 110.25),
        (30.25, 70.25),
    ]
    .iter()
    .map(|&(x, y)| hit_id(harness, x, y))
    .collect()
}

fn body_box(harness: &Harness<Page<()>>) -> (f64, f64) {
    let body = node(harness, "body");
    let rect = harness
        .base()
        .get_client_bounding_rect(body)
        .expect("the body has a box");
    (rect.y, rect.height)
}

fn named(ids: &[Option<&str>]) -> Vec<Option<String>> {
    ids.iter().map(|id| id.map(str::to_string)).collect()
}

#[test]
fn a_body_whose_overflow_goes_to_the_viewport_clips_nothing_like_chrome() {
    let mut harness = open(CLIPPING_BODY);
    assert_eq!(
        hits(&harness),
        named(&[Some("a"), Some("b"), Some("c"), Some("c")])
    );
    set_style(&mut harness, "html", "overflow:hidden");
    assert_eq!(hits(&harness), named(&[None, None, None, Some("c")]));
}

#[test]
fn a_body_whose_overflow_goes_to_the_viewport_is_no_formatting_root_like_chrome() {
    let mut harness = open(FORMATTING_BODY);
    assert_eq!(body_box(&harness), (30.0, 20.0));
    set_style(&mut harness, "html", "overflow:hidden");
    assert_eq!(body_box(&harness), (0.0, 90.0));
    set_style(&mut harness, "html", "");
    assert_eq!(body_box(&harness), (30.0, 20.0));
    set_style(&mut harness, "body", "contain:paint");
    assert_eq!(body_box(&harness), (0.0, 90.0));
    set_style(&mut harness, "body", "");
    set_style(&mut harness, "html", "contain:paint");
    assert_eq!(body_box(&harness), (0.0, 90.0));
}

const TALL: &str = "<!doctype html><style>body{margin:0}</style>\
<div id=h style=\"overflow:hidden;width:100px;height:100px\"><div style=\"height:300px\"></div></div>\
<div style=\"width:3000px;height:3000px\"></div>";

fn scroll(harness: &Harness<Page<()>>) -> (f64, f64) {
    let offset = harness.base().viewport_scroll();
    (offset.x, offset.y)
}

fn script_scroll(harness: &mut Harness<Page<()>>, selector: &str, (x, y): (f64, f64)) {
    let node = node(harness, selector);
    harness
        .base_mut()
        .scroll_to(node, x, y, pageframe_dom::ScrollBehavior::Instant);
}

#[test]
fn a_hidden_viewport_ignores_the_wheel_but_scrolls_from_script_like_chrome() {
    let mut harness = open(TALL);
    harness.wheel_at(200.0, 150.0, -40.0, -60.0);
    assert_eq!(scroll(&harness), (40.0, 60.0));
    for (selector, style) in [
        ("html", "overflow:hidden"),
        ("body", "overflow:hidden"),
        ("html", "overflow:clip"),
    ] {
        script_scroll(&mut harness, "html", (0.0, 0.0));
        set_style(&mut harness, selector, style);
        script_scroll(&mut harness, "html", (100.0, 200.0));
        assert_eq!(scroll(&harness), (100.0, 200.0), "{selector} {style}");
        harness.wheel_at(200.0, 150.0, -40.0, -60.0);
        assert_eq!(scroll(&harness), (100.0, 200.0), "{selector} {style}");
        harness.set_viewport_size(330, 250);
        harness.set_viewport_size(320, 240);
        assert_eq!(scroll(&harness), (100.0, 200.0), "{selector} {style}");
        set_style(&mut harness, selector, "");
    }
    set_style(&mut harness, "html", "overflow-x:hidden");
    harness.wheel_at(200.0, 150.0, -40.0, -60.0);
    assert_eq!(scroll(&harness), (100.0, 260.0));
}

#[test]
fn the_wheel_leaves_a_hidden_box_where_script_scrolled_it() {
    let mut harness = open(TALL);
    script_scroll(&mut harness, "#h", (0.0, 50.0));
    harness.wheel_at(50.0, 50.0, 0.0, -30.0);
    let offset = harness
        .base()
        .get_node(node(&harness, "#h"))
        .expect("the box exists")
        .scroll_offset()
        .y;
    assert_eq!((offset, scroll(&harness).1), (50.0, 30.0));
}
