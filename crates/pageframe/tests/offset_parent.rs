use pageframe::{App, Page};
use pageframe_test_harness::Harness;

const PAGE: &str = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}</style>\
<div>ab<span id=s1 style=\"position:relative;left:20px;top:4px\">c<span id=b1 \
style=\"display:inline-block;width:10px;height:10px\"></span>d</span></div>\
<div>ab<span id=s4 style=\"position:relative;left:4px\">cd<span id=b4 \
style=\"position:absolute;left:3px;top:2px\">e</span></span></div>\
<div style=\"margin-left:30px\">ab<span id=s5 style=\"position:relative;border:3px solid;\
padding:4px 6px;left:2px\">c<span id=b5 style=\"display:inline-block;width:6px;height:6px\">\
</span></span></div>\
<div><div id=b6 style=\"position:fixed;top:5px;left:300px\">f</div><table id=t6><tr>\
<td id=c6>g<span id=b7 style=\"display:inline-block;width:4px;height:4px\"></span></td>\
</tr></table></div>";

fn offsets(harness: &mut Harness<Page<()>>, id: &str) -> (f64, f64, Option<String>) {
    let doc = harness.base_mut();
    let node = doc
        .query_selector(&format!("#{id}"))
        .expect("valid selector")
        .expect("the element exists");
    let rect = doc.offset_rect(node).expect("the element has a box");
    let parent = doc
        .get_node(node)
        .and_then(|node| node.offset_parent())
        .and_then(|parent| parent.element_data())
        .and_then(|element| element.id.as_ref())
        .map(ToString::to_string);
    drop(doc);
    (rect.x.round(), rect.y.round(), parent)
}

fn open() -> Harness<Page<()>> {
    let page = App::new((), PAGE)
        .page(640, 200)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    harness
}

fn expected(x: f64, y: f64, parent: &str) -> (f64, f64, Option<String>) {
    (x, y, Some(parent.to_string()))
}

#[test]
fn an_inline_block_measures_from_its_positioned_span_like_chrome() {
    let mut harness = open();
    assert_eq!(offsets(&mut harness, "b1"), expected(7.0, 3.0, "s1"));
    assert_eq!(offsets(&mut harness, "b5"), expected(16.0, 14.0, "s5"));
}

#[test]
fn an_absolute_box_measures_from_its_positioned_span_like_chrome() {
    let mut harness = open();
    assert_eq!(offsets(&mut harness, "b4"), expected(3.0, 2.0, "s4"));
}

#[test]
fn table_parts_and_fixed_boxes_pick_the_offset_parent_like_chrome() {
    let mut harness = open();
    assert_eq!(offsets(&mut harness, "c6"), expected(2.0, 2.0, "t6"));
    assert_eq!(offsets(&mut harness, "b7"), expected(9.0, 10.0, "c6"));
    assert_eq!(offsets(&mut harness, "b6"), (300.0, 5.0, None));
}
