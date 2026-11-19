use pageframe::App;
use pageframe_test_harness::Harness;

const PAGE: &str = "<!doctype html><style>html,body{margin:0;font:14px Arial}</style>\
<div id=n1 style=\"display:none\"><span id=k1>x</span><div id=q1>y</div></div>\
<div id=c2 style=\"display:contents\"><span id=s2>z</span></div>\
<div id=v4 style=\"visibility:hidden\">v</div>";

fn rects(ids: &[&str]) -> Vec<(usize, bool)> {
    let page = App::new((), PAGE)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let doc = harness.base();
    ids.iter()
        .map(|id| {
            let node = doc
                .query_selector(&format!("#{id}"))
                .expect("valid selector")
                .expect("the element exists");
            (
                doc.node_client_rects(node).len(),
                doc.get_client_bounding_rect(node).is_some(),
            )
        })
        .collect()
}

#[test]
fn hidden_subtrees_and_contents_boxes_have_no_client_rects_like_chrome() {
    assert_eq!(
        rects(&["n1", "k1", "q1", "c2"]),
        vec![(0, false), (0, false), (0, false), (0, false)]
    );
}

#[test]
fn children_of_contents_and_invisible_boxes_keep_their_rects_like_chrome() {
    assert_eq!(rects(&["s2", "v4"]), vec![(1, true), (1, true)]);
}
