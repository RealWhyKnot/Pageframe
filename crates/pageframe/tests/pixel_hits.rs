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
