mod support;

use pageframe::App;
use support::open;

const BOUNDARY: &str = "<!doctype html><style>html,body{margin:0}\
.boxes{font-size:0}.boxes span{display:inline-block;width:11px;height:5px;background:#f00}\
section{display:flow-root;font:10px/10px Consolas,monospace}\
b{float:left;width:10px;height:30px;background:#00f}</style>\
<div id=fits class=boxes style='width:21.99px'><span></span><span></span></div>\
<div id=wraps class=boxes style='width:21.98px'><span></span><span></span></div>\
<section style='width:calc(4ch + 10px)'><b></b><div id=beside>abcd abcd</div></section>\
<section style='width:calc(4ch + 9.98px)'><b></b><div id=below>abcd abcd</div></section>";

#[test]
fn lines_fit_one_layout_unit_past_their_box_like_chrome() {
    let mut harness = open(App::new((), BOUNDARY));
    let heights = ["#fits", "#wraps", "#beside", "#below"].map(|selector| {
        let doc = harness.base_mut();
        let id = doc
            .query_selector(selector)
            .expect("valid selector")
            .expect("the div exists");
        doc.get_client_bounding_rect(id)
            .map(|rect| rect.height)
            .expect("the div has a box")
    });
    assert_eq!(heights, [5.0, 10.0, 20.0, 50.0]);
}
