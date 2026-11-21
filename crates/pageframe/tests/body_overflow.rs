use pageframe::{App, Page};
use pageframe_test_harness::Harness;

fn open(root: &str, body: &str) -> Harness<Page<()>> {
    let html = format!(
        "<!doctype html><style>html,body{{margin:0}}html{{{root}}}body{{{body}}}\
         #k{{height:100px;width:2000px}}</style><body id=bd><div id=k></div>\
         <div style=\"height:2000px\"></div>"
    );
    let page = App::new((), &html)
        .page(640, 400)
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

fn scrolls(harness: &mut Harness<Page<()>>) -> ((f64, f64), f64) {
    for (selector, x, y) in [("html", 30.0, 40.0), ("body", 0.0, 5.0)] {
        let target = node(harness, selector);
        harness
            .base_mut()
            .scroll_to(target, x, y, pageframe_dom::ScrollBehavior::Instant);
    }
    let doc = harness.base();
    let viewport = doc.viewport_scroll();
    let body = doc
        .get_node(node(harness, "body"))
        .expect("the body exists")
        .scroll_offset()
        .y;
    drop(doc);
    ((viewport.x, viewport.y), body)
}

#[test]
fn a_body_keeps_its_overflow_when_the_root_has_its_own_like_chrome() {
    for (root, body, chrome_scroll) in [
        (
            "overflow:hidden",
            "overflow:hidden;height:50px",
            ((0.0, 0.0), 5.0),
        ),
        (
            "overflow:clip",
            "overflow:hidden;height:50px",
            ((0.0, 0.0), 5.0),
        ),
        (
            "overflow:auto",
            "overflow:auto;height:50px",
            ((0.0, 0.0), 5.0),
        ),
    ] {
        let mut harness = open(root, body);
        assert_eq!(hit_id(&harness, 5.0, 80.0), None, "{root} {body}");
        assert_eq!(scrolls(&mut harness), chrome_scroll, "{root} {body}");
    }
}

#[test]
fn a_body_overflow_on_one_axis_still_goes_to_the_viewport_like_chrome() {
    let mut harness = open("", "overflow-x:hidden;height:50px");
    assert_eq!(hit_id(&harness, 5.0, 80.0), Some("k".to_string()));
    assert_eq!(scrolls(&mut harness), ((30.0, 40.0), 0.0));
}
