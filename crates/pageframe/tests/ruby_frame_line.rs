use pageframe::App;
use pageframe_test_harness::Harness;

const PAGE: &str = "<!doctype html><style>html,body{margin:0;font:14px Arial}</style>\
<div id=a0>ab<ruby id=a1 style=\"padding:4px 0;border-top:3px solid\">XY<rt id=a2>zz</rt></ruby>cd</div>";

fn vertical(harness: &Harness<pageframe::Page<()>>, id: &str) -> [i64; 2] {
    let doc = harness.base();
    let node = doc
        .query_selector(&format!("#{id}"))
        .expect("valid selector")
        .expect("the element exists");
    let rects = doc.node_client_rects(node);
    drop(doc);
    let rect = rects.first().expect("the element has a rect");
    [rect.y, rect.height].map(|value| {
        num_traits::ToPrimitive::to_i64(&(value * 64.0).round()).expect("test rects fit in i64")
    })
}

#[test]
fn inline_ruby_padding_and_border_do_not_grow_the_line_like_chrome() {
    let page = App::new((), PAGE)
        .page(640, 800)
        .expect("the page has valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let measured: Vec<[i64; 2]> = ["a0", "a1", "a2"]
        .iter()
        .map(|id| vertical(&harness, id))
        .collect();
    assert_eq!(measured, [[0, 1344], [-128, 1728], [0, 448]]);
}
