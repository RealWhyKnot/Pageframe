use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use pageframe_traits::shell::Viewport;

const BOX: &str = "<!doctype html><style>html,body{margin:0}#b{position:absolute;left:10.5px;\
top:20.5px;width:10px;height:10px;background:red}</style><div id=b></div>";

const LINE_END: &str = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}\
</style><div id=d1 style=\"width:44px\">x<span id=s1>aaaa bbbb</span></div>";

const VERTICAL_LR: &str = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}\
</style><div id=d1 style=\"writing-mode:vertical-lr;height:40px\">xx<span id=w1 \
style=\"position:relative;top:6px\">aaaa</span> <span id=w2>bbbb</span> \
<span id=w3>cccc</span> <span id=w4>dddd</span></div>";

const EMPTY_SPANS: &str = "<!doctype html><style>html,body{margin:0;background:#fff;font:16px Arial}</style><div id=d1 style=\"width:200px\">xx<span id=s1> </span></div><div id=d2 style=\"width:200px;white-space:pre-wrap\">xx<span id=s2></span></div><div id=d3 style=\"width:200px\">xx<span id=s3></span>yy</div><div id=d4 style=\"width:30px\">xx<span id=s4> </span>yyyyyy</div><div id=d5 style=\"width:30px;text-align:right\">xx<span id=s5> </span>yyyyyy</div>";

const TRANSFORMED: &str = "<!doctype html><style>html,body{margin:0;background:#fff}\
div{position:absolute}</style>\
<div style=\"transform:scale(4);transform-origin:0 0\"><div id=a style=\"left:10px;top:2px;\
width:5px;height:5px\"></div></div>\
<div style=\"top:100px;transform:scale(0.5);transform-origin:0 0\"><div id=b \
style=\"left:20px;top:20px;width:20px;height:20px\"></div></div>\
<div id=c style=\"left:10.5px;top:150px;width:20px;height:20px;overflow:hidden\"><div id=c2 \
style=\"left:-10px;width:60px;height:20px\"></div></div>\
<div id=e style=\"top:200px;width:40px;height:20px;clip-path:inset(0 0 0 10.5px)\"></div>";

const CLIPPED: &str = "<!doctype html><style>html,body{margin:0;background:#fff}</style>\
<div id=g style=\"position:relative;overflow:hidden;z-index:1;width:20px;height:20px\"><div id=g2 \
style=\"position:absolute;z-index:2;left:0;top:0;width:60px;height:20px\"></div></div>\
<div id=h style=\"overflow:hidden;opacity:0.5;width:20px;height:20px\"><div id=h2 \
style=\"position:absolute;top:20px;width:60px;height:20px\"></div></div>\
<div id=k style=\"overflow:hidden;width:20px;height:20px\"><div id=k2 \
style=\"width:60px;height:20px\"></div></div>";

const BORDERED_CLIPS: &str = "<!doctype html><style>html,body{margin:0;background:#fff}\
div{box-sizing:border-box}</style><div style=\"padding:20px 0 0 20px\"><div id=c8 \
style=\"width:50px;height:30px;overflow:hidden;border:2px solid #888\"><div id=k8 \
style=\"margin:-10px 0 0 -10px;width:140px;height:60px\"></div></div><div id=c9 \
style=\"width:50px;height:30px;overflow:hidden;border:2px solid #888;opacity:0.9;margin-top:30px\">\
<div id=k9 style=\"margin:-10px 0 0 -10px;width:140px;height:60px\"></div></div></div><div id=c4 \
style=\"position:absolute;left:100px;top:100px;width:50px;height:30px;overflow:hidden;\
border:2px solid #888\"><div id=k4 style=\"width:140px;height:12px;position:relative\"></div></div>";

fn open(page: &str, scale: f32) -> Harness<Page<()>> {
    let page = App::new((), page)
        .page(160, 120)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    {
        let mut doc = harness.base_mut();
        let color_scheme = doc.get_viewport().color_scheme;
        doc.set_viewport(Viewport::new(320, 240, scale, color_scheme));
    }
    harness.pump();
    harness
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

fn ids_at(page: &str, scale: f32, points: &[(f32, f32)]) -> Vec<Option<String>> {
    let harness = open(page, scale);
    points
        .iter()
        .map(|&(x, y)| hit_id(&harness, x, y))
        .collect()
}

fn expected(ids: &[Option<&str>]) -> Vec<Option<String>> {
    ids.iter().map(|id| id.map(str::to_string)).collect()
}

#[test]
fn a_point_hits_every_box_its_device_pixel_overlaps_like_chrome() {
    let (hit, miss) = (Some("b"), None);
    let whole = BOX.replace("left:10.5px;top:20.5px", "left:10px;top:20px");
    let points = [
        (9.0, 25.0),
        (9.5, 25.0),
        (9.9, 25.0),
        (19.9, 25.0),
        (20.0, 25.0),
        (15.0, 19.0),
        (15.0, 19.5),
        (15.0, 29.9),
        (15.0, 30.0),
    ];
    assert_eq!(
        ids_at(&whole, 1.0, &points),
        expected(&[miss, hit, hit, hit, miss, miss, hit, hit, miss]),
        "scale 1"
    );
    let points = [
        (9.9, 25.0),
        (10.0, 25.0),
        (10.1, 25.0),
        (20.4, 25.0),
        (20.5, 25.0),
        (21.0, 25.0),
        (15.0, 20.0),
        (15.0, 20.25),
        (15.0, 30.25),
        (15.0, 30.5),
    ];
    assert_eq!(
        ids_at(BOX, 2.0, &points),
        expected(&[miss, miss, hit, hit, miss, miss, miss, hit, hit, miss]),
        "scale 2"
    );
}

#[test]
fn a_collapsed_space_at_a_line_end_is_not_hit_like_chrome() {
    let points = [
        (6.0, 8.0),
        (6.25, 8.0),
        (38.0, 8.0),
        (38.2, 8.0),
        (40.0, 8.0),
    ];
    assert_eq!(
        ids_at(LINE_END, 1.0, &points),
        expected(&[Some("d1"), Some("s1"), Some("s1"), Some("d1"), Some("d1")])
    );
    let points = [(16.25, 30.25), (30.25, 30.25)];
    assert_eq!(
        ids_at(VERTICAL_LR, 1.0, &points),
        expected(&[Some("w2"), Some("w2")])
    );
}

#[test]
fn a_span_with_no_width_is_never_hit_like_chrome() {
    let offsets = [-1.5, -1.0, -0.75, -0.5, -0.25, 0.0, 0.25, 0.5];
    for (y, id) in [(9.0, "d1"), (27.0, "d2"), (45.0, "d3")] {
        let points: Vec<(f32, f32)> = offsets.iter().map(|offset| (16.0 + offset, y)).collect();
        assert_eq!(
            ids_at(EMPTY_SPANS, 1.0, &points),
            vec![Some(id.to_string()); offsets.len()],
            "{id}"
        );
    }
    let points = [
        (14.5, 63.0),
        (15.5, 63.0),
        (15.75, 63.0),
        (16.0, 63.0),
        (29.0, 99.0),
        (29.5, 99.0),
    ];
    assert_eq!(
        ids_at(EMPTY_SPANS, 1.0, &points),
        expected(&[
            Some("d4"),
            Some("d4"),
            Some("d4"),
            Some("d4"),
            Some("d5"),
            Some("d5")
        ]),
        "hung"
    );
}

#[test]
fn transforms_scale_the_device_pixel_and_clips_test_it_like_chrome() {
    let points = [
        (39.0, 16.0),
        (39.25, 16.0),
        (59.75, 16.0),
        (60.0, 16.0),
        (9.0, 115.0),
        (9.5, 115.0),
        (19.5, 115.0),
        (20.0, 115.0),
        (10.25, 210.0),
        (10.5, 210.0),
        (5.0, 160.0),
        (20.0, 160.0),
        (50.0, 160.0),
    ];
    assert_eq!(
        ids_at(TRANSFORMED, 1.0, &points),
        expected(&[
            None,
            Some("a"),
            Some("a"),
            None,
            None,
            Some("b"),
            Some("b"),
            None,
            None,
            Some("e"),
            None,
            Some("c2"),
            None
        ]),
        "scale 1"
    );
    let points = [
        (39.5, 16.0),
        (39.75, 16.0),
        (9.5, 115.0),
        (9.75, 115.0),
        (10.0, 160.0),
        (10.25, 160.0),
        (30.25, 160.0),
        (30.5, 160.0),
        (10.25, 210.0),
        (10.5, 210.0),
    ];
    assert_eq!(
        ids_at(TRANSFORMED, 2.0, &points),
        expected(&[
            None,
            Some("a"),
            None,
            Some("b"),
            None,
            Some("c2"),
            Some("c2"),
            None,
            None,
            Some("e")
        ]),
        "scale 2"
    );
}

#[test]
fn overflow_clips_hits_on_the_children_it_contains_like_chrome() {
    let points = [
        (10.0, 10.0),
        (40.0, 10.0),
        (10.0, 30.0),
        (40.0, 30.0),
        (10.0, 50.0),
        (40.0, 50.0),
    ];
    assert_eq!(
        ids_at(CLIPPED, 1.0, &points),
        expected(&[Some("g2"), None, Some("h2"), Some("h2"), Some("k2"), None])
    );
}

#[test]
fn a_layer_clips_flow_children_at_its_border_box_like_chrome() {
    let points = [
        (20.5, 30.0),
        (21.5, 30.0),
        (67.5, 30.0),
        (68.5, 30.0),
        (40.0, 47.5),
        (40.0, 48.5),
        (20.5, 90.0),
        (68.5, 90.0),
        (40.0, 109.5),
        (147.9, 105.0),
        (148.0, 105.0),
        (149.9, 105.0),
    ];
    assert_eq!(
        ids_at(BORDERED_CLIPS, 1.0, &points),
        expected(&[
            Some("c8"),
            Some("k8"),
            Some("k8"),
            Some("c8"),
            Some("k8"),
            Some("c8"),
            Some("k9"),
            Some("k9"),
            Some("k9"),
            Some("k4"),
            Some("c4"),
            Some("c4"),
        ])
    );
}
