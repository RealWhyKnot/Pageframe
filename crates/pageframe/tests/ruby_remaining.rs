use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use pageframe_traits::shell::Viewport;

const STYLE: &str = "<!doctype html><meta charset=utf-8><style>html,body{margin:0;font:14px Arial}\
div{width:200px;margin-bottom:20px;border-top:2px solid}</style>";

fn boxes(body: &str, (numerator, denominator): (u16, u16), ids: &[&str]) -> Vec<[f64; 4]> {
    let page: Page<()> = App::new((), &format!("{STYLE}{body}"))
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
    ids.iter()
        .map(|id| {
            let node = doc
                .query_selector(&format!("#{id}"))
                .expect("valid selector")
                .expect("the element exists");
            let rect = doc
                .get_client_bounding_rect(node)
                .expect("the element has a box");
            [rect.x, rect.y, rect.width, rect.height]
        })
        .collect()
}

fn assert_near(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 1.0 / 64.0,
        "{what}: {actual} vs {expected}"
    );
}

#[test]
fn an_annotation_overhangs_a_lone_space_by_half_its_width() {
    let body = "<div>&nbsp;<ruby id=a>x<rt>long annotation</rt></ruby>y</div>\
<div style='white-space:pre'> <ruby id=b>x<rt>long annotation</rt></ruby>y</div>\
<div dir=rtl>ab<ruby id=c>x<rt>long annotation</rt></ruby>cd</div>";
    let found = boxes(body, (1, 1), &["a", "b", "c"]);
    assert_near(found[0][2], 42.94, "after a no-break space");
    assert_near(found[1][2], 42.94, "after a preserved space");
    assert_near(found[2][2], 41.88, "between words");
}

#[test]
fn a_lowered_base_carries_its_annotation_down_with_it() {
    let body = "<div id=d>ab<ruby><span id=s style='vertical-align:sub'>cd</span><rt id=t>a</rt></ruby>ef</div>";
    for (scale, height, span_top, annotation_top) in
        [((1, 1), 23.0, 7.0, 2.0), ((2, 1), 23.5, 7.5, 1.0)]
    {
        let found = boxes(body, scale, &["d", "s", "t"]);
        let (line, span, annotation) = (found[0], found[1], found[2]);
        assert_near(line[3], height, "line");
        assert_near(span[1] - line[1], span_top, "lowered base");
        assert_near(annotation[1] - line[1], annotation_top, "annotation");
    }
}

#[test]
fn a_vertical_annotation_sits_beside_the_centered_em_box() {
    let body = "<div id=d style='writing-mode:vertical-rl;height:200px;width:auto'>ab<ruby>\
<span id=s style='font-size:30px'>cd</span><rt id=t>a</rt></ruby>ef</div>";
    let found = boxes(body, (1, 1), &["d", "s", "t"]);
    let (line, span, annotation) = (found[0], found[1], found[2]);
    assert_near(line[2], 39.5, "line");
    assert_near(span[0], 1.0, "base");
    assert_near(annotation[0], 32.5, "annotation");
}

#[test]
fn annotation_tops_snap_like_chrome_at_fractional_scales() {
    let body = "<div style='border:0'>ab<ruby><span style='line-height:40px'>cd</span><rt id=t>a</rt></ruby>ef</div>";
    for (scale, top) in [((1, 1), 7.0), ((7, 4), 40.0 / 7.0), ((2, 1), 5.5)] {
        assert_near(boxes(body, scale, &["t"])[0][1], top, "annotation");
    }
}

#[test]
fn a_fallback_font_base_uses_the_primary_em_box() {
    let body = "<div id=d>ab<ruby id=r>&#28450;&#23383;<rt>a</rt></ruby>ef</div>";
    let found = boxes(body, (1, 1), &["d", "r"]);
    let (line, ruby) = (found[0], found[1]);
    assert_near(line[3], 24.0, "line");
    assert_near(ruby[1] - line[1], 7.0, "ruby");
}
