use num_traits::ToPrimitive;
use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use pageframe_traits::shell::Viewport;

fn open(app: App<()>) -> Harness<Page<()>> {
    let page = app.page(120, 100).expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    harness
}

const PAGE: &str = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}\
div{height:30px}</style>\
<div id=d1>ab<span id=s1 style=\"position:relative;left:30px;top:10px\">cdef</span>gh</div>\
<div id=d5>xyzxyzxyz</div>\
<div id=d6>ab<span id=s6 style=\"position:relative;top:-14px;left:10px;padding:2px\">cdef</span></div>";

const NESTED: &str = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}\
div{height:40px}</style>\
<div id=outer>ab<span id=mid style=\"position:relative;left:20px\">cd<span id=inner \
style=\"position:relative;top:16px\">efgh</span></span></div>";

const MIXED: &str = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}\
div{height:30px;width:120px}</style>\
<div id=d1 style=\"direction:rtl\">ab<span id=s1 style=\"position:relative;left:-20px;top:8px\">\
cdef</span>gh</div>\
<div id=d2>ab<span id=s2 style=\"position:relative;left:10px;top:4px\">c<button id=b2 \
style=\"font:10px Arial;padding:0 4px\">Go</button>d</span></div>\
<div id=d3 style=\"writing-mode:vertical-rl;height:60px;width:60px\">ab<span id=s3 \
style=\"position:relative;left:-12px;top:10px\">cdef</span></div>";

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

fn ids_at(page: &str, points: &[(f32, f32)]) -> Vec<Option<String>> {
    let harness = open(App::new((), page));
    points
        .iter()
        .map(|&(x, y)| hit_id(&harness, x, y))
        .collect()
}

fn expected(ids: &[&str]) -> Vec<Option<String>> {
    ids.iter().map(|id| Some((*id).to_string())).collect()
}

#[test]
fn moved_inline_text_is_hit_where_it_is_painted_like_chrome() {
    let points = [(58.25, 17.25), (24.25, 5.25), (5.25, 5.25), (90.25, 24.25)];
    assert_eq!(ids_at(PAGE, &points), expected(&["s1", "d1", "d1", "d1"]));
}

#[test]
fn a_span_moved_over_an_earlier_block_is_hit_above_it_like_chrome() {
    let points = [
        (40.25, 50.25),
        (40.25, 45.25),
        (20.25, 50.25),
        (70.25, 50.25),
    ];
    assert_eq!(ids_at(PAGE, &points), expected(&["s6", "s6", "d5", "d5"]));
}

#[test]
fn the_old_place_of_a_moved_span_hits_its_block_like_chrome() {
    let points = [(40.25, 70.25), (40.25, 80.25), (5.25, 70.25)];
    assert_eq!(ids_at(PAGE, &points), expected(&["d6", "d6", "d6"]));
}

#[test]
fn moved_spans_hit_like_chrome_in_rtl_vertical_text_and_around_buttons() {
    let points = [
        (70.25, 14.25),
        (96.25, 4.25),
        (20.25, 42.25),
        (44.25, 42.25),
        (60.25, 42.25),
        (40.25, 96.25),
        (52.25, 80.25),
        (52.25, 96.25),
    ];
    assert_eq!(
        ids_at(MIXED, &points),
        expected(&["s1", "d1", "d2", "b2", "s2", "s3", "d3", "d3"])
    );
}

#[test]
fn nested_moved_spans_are_hit_at_their_summed_offsets() {
    let mut harness = open(App::new((), NESTED));
    let rect = {
        let doc = harness.base_mut();
        let node = doc
            .query_selector("#inner")
            .expect("valid selector")
            .expect("the span exists");
        doc.node_client_rects(node)
            .into_iter()
            .next()
            .expect("the span has a box")
    };
    let x = (rect.x + rect.width / 2.0)
        .to_f32()
        .expect("the centre fits in f32");
    let y = (rect.y + rect.height / 2.0)
        .to_f32()
        .expect("the centre fits in f32");
    assert_eq!(hit_id(&harness, x, y).as_deref(), Some("inner"));
    assert_eq!(hit_id(&harness, x, y - 16.0).as_deref(), Some("mid"));
    assert_eq!(hit_id(&harness, x + 30.0, y).as_deref(), Some("outer"));
}

const LATE: &str = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}\
</style><div id=late>l1<br>l2<br>l3<br>l4<br><span id=m>move</span></div>";

#[test]
fn a_span_moved_after_layout_is_hit_over_earlier_lines() {
    let mut harness = open(App::new((), LATE));
    assert_eq!(hit_id(&harness, 20.25, 8.25).as_deref(), Some("late"));
    let span = harness
        .base()
        .query_selector("#m")
        .expect("valid selector")
        .expect("the span exists");
    harness.base_mut().mutate().set_attribute(
        span,
        pageframe_dom::QualName::new(
            None,
            pageframe_dom::ns!(),
            pageframe_dom::local_name!("style"),
        ),
        "position:relative;top:-60px",
    );
    harness.pump();
    assert_eq!(hit_id(&harness, 20.25, 8.25).as_deref(), Some("m"));
    assert_eq!(hit_id(&harness, 20.25, 70.25).as_deref(), Some("late"));
}

#[test]
fn a_span_moved_over_earlier_lines_is_hit_at_half_and_double_scale() {
    let page = LATE.replace("id=m>", "id=m style=\"position:relative;top:-60px\">");
    for scale in [0.5, 2.0] {
        let mut harness = open(App::new((), &page));
        {
            let mut doc = harness.base_mut();
            let color_scheme = doc.get_viewport().color_scheme;
            doc.set_viewport(Viewport::new(240, 200, scale, color_scheme));
        }
        harness.pump();
        assert_eq!(
            hit_id(&harness, 20.25, 8.25).as_deref(),
            Some("m"),
            "scale {scale}"
        );
    }
}

const VERTICAL_LR: &str = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}</style>\
<div id=d1 style=\"writing-mode:vertical-lr;height:40px\">xx<span id=w1 \
style=\"position:relative;top:6px\">aaaa</span> <span id=w2>bbbb</span> \
<span id=w3>cccc</span> <span id=w4>dddd</span></div>";

#[test]
fn every_vertical_lr_line_is_hit_like_chrome() {
    let points = [
        (4.25, 30.25),
        (20.25, 10.25),
        (36.25, 10.25),
        (36.25, 30.25),
        (50.25, 10.25),
        (62.25, 4.25),
    ];
    assert_eq!(
        ids_at(VERTICAL_LR, &points),
        expected(&["w1", "w2", "w3", "d1", "w4", "w4"])
    );
}

#[test]
fn every_sideways_lr_line_is_hit_like_chrome() {
    let page = VERTICAL_LR.replace("vertical-lr", "sideways-lr");
    let points = [
        (4.25, 20.25),
        (20.25, 20.25),
        (36.25, 20.25),
        (50.25, 20.25),
        (62.25, 28.25),
    ];
    assert_eq!(
        ids_at(&page, &points),
        expected(&["w1", "w2", "w3", "w4", "w4"])
    );
}
