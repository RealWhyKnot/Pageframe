use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use pageframe_traits::shell::Viewport;

const STYLE: &str = "<!doctype html><meta charset=utf-8><style>html,body{margin:0;font:14px Arial}\
</style>";

const BODY: &str = "<div id=a1 style='line-height:40px'>ab<ruby>cd<rt>a</rt></ruby>ef</div>\
<div id=a2>ab<ruby><span style='font-size:30px'>cd</span><rt id=at2>a</rt></ruby>ef</div>\
<div id=b1>plain</div>\
<div id=b2>ab<ruby><span style='font-size:30px'>cd</span><rt>a</rt></ruby>ef</div>\
<div id=c1 style='margin-bottom:10px'>plain</div>\
<div id=c2>ab<ruby><span style='font-size:30px'>cd</span><rt>a</rt></ruby>ef</div>\
<div id=e1 style='line-height:30px'>ab</div>\
<div id=e2>ab<ruby>cd<rt id=et2>a</rt></ruby>ef</div>\
<div id=f1 style='width:60px'>aaaa bbbb<br>ab<ruby>cd<rt>a</rt></ruby>ef</div>\
<div id=g1 style='width:60px;line-height:30px'>aaaa bbbb<br>ab<ruby>cd<rt>a</rt></ruby>ef</div>\
<div id=h1 style='padding-top:3px'>ab<ruby>cd<rt>a</rt></ruby>ef</div>\
<div id=i1 style='padding-top:8px'>ab<ruby>cd<rt>a</rt></ruby>ef</div>\
<div id=j0 style='display:flow-root;margin-top:10px'>ab<ruby>cd<rt>a</rt></ruby>ef</div>\
<div id=k1 style='width:60px;border-top:1px solid'>aaaa <span style='line-height:30px'>bbbb</span>\
<br>ab<ruby>cd<rt>a</rt></ruby>ef</div>\
<div id=l1 style='width:60px;border-top:1px solid'>ab<ruby>cd<rt>a</rt></ruby>ef<br>\
ab<ruby>cd<rt>a</rt></ruby>ef</div>\
<div id=m1 style='border-top:1px solid;line-height:20px'>ab<ruby>cd<rt>a</rt></ruby>ef</div>";

const IDS: [&str; 17] = [
    "a1", "a2", "at2", "b1", "b2", "c1", "c2", "e1", "e2", "et2", "f1", "g1", "h1", "i1", "j0",
    "k1", "l1",
];

fn boxes((numerator, denominator): (u16, u16)) -> Vec<(f64, f64)> {
    let page: Page<()> = App::new((), &format!("{STYLE}{BODY}"))
        .page(640, 400)
        .expect("the page parses");
    let mut harness = Harness::wrap(page);
    harness.pump();
    {
        let mut doc = harness.base_mut();
        let color_scheme = doc.get_viewport().color_scheme;
        let device = |css: u32| css * u32::from(numerator) / u32::from(denominator);
        let scale = f32::from(numerator) / f32::from(denominator);
        doc.set_viewport(Viewport::new(device(640), device(400), scale, color_scheme));
    }
    harness.pump();
    let doc = harness.base();
    IDS.iter()
        .map(|id| {
            let node = doc
                .query_selector(&format!("#{id}"))
                .expect("valid selector")
                .expect("the element exists");
            let rect = doc
                .get_client_bounding_rect(node)
                .expect("the element has a box");
            (rect.y, rect.height)
        })
        .collect()
}

fn assert_boxes(scale: (u16, u16), expected: [(f64, f64); 17]) {
    let found = boxes(scale);
    for ((id, (y, height)), (want_y, want_height)) in IDS.iter().zip(found).zip(expected) {
        assert!(
            (y - want_y).abs() <= 0.02 && (height - want_height).abs() <= 0.02,
            "{id} at {scale:?}: y {y} height {height}, Chrome 157 has y {want_y} height {want_height}"
        );
    }
}

#[test]
fn annotations_borrow_the_space_above_their_line() {
    assert_boxes(
        (1, 1),
        [
            (0.0, 40.0),
            (40.0, 34.0),
            (36.0, 7.0),
            (74.0, 16.0),
            (90.0, 38.0),
            (128.0, 16.0),
            (154.0, 34.0),
            (188.0, 30.0),
            (218.0, 16.0),
            (213.0, 7.0),
            (234.0, 53.0),
            (287.0, 90.0),
            (377.0, 19.0),
            (396.0, 24.0),
            (430.0, 21.0),
            (451.0, 63.0),
            (514.0, 43.0),
        ],
    );
}

#[test]
fn borrowed_annotation_space_holds_at_double_scale() {
    assert_boxes(
        (2, 1),
        [
            (0.0, 40.0),
            (40.0, 34.5),
            (36.0, 8.0),
            (74.5, 16.0),
            (90.5, 37.0),
            (127.5, 16.0),
            (153.5, 34.5),
            (188.0, 30.0),
            (218.0, 16.0),
            (211.5, 8.0),
            (234.0, 53.0),
            (287.0, 90.0),
            (377.0, 19.0),
            (396.0, 24.0),
            (430.0, 21.5),
            (451.5, 63.0),
            (514.5, 43.5),
        ],
    );
}
