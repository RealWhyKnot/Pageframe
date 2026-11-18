use pageframe::{App, Page};
use pageframe_dom::ScrollBehavior;
use pageframe_test_harness::Harness;

const WIDTH: u32 = 640;
const HEIGHT: u32 = 400;
const STYLE: &str = "html,body{margin:0;background:white}body{font:16px/20px Arial}p{margin:0}\
.c{width:320px;column-count:3;column-gap:10px}.b{height:20px;background:#00ff00}";

fn page(body: &str) -> Harness<Page<()>> {
    let html = format!("<html><head><style>{STYLE}</style></head><body>{body}</body></html>");
    let page = App::new((), &html)
        .page(WIDTH, HEIGHT)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    harness
}

fn blocks(count: usize) -> String {
    (0..count)
        .map(|index| format!("<div id=\"b{index}\" class=\"b\"></div>"))
        .collect()
}

fn offset(harness: &Harness<Page<()>>, id: &str) -> [f64; 4] {
    let doc = harness.base();
    let node_id = doc
        .query_selector_all(&format!("#{id}"))
        .expect("box selector is valid")[0];
    let rect = doc.offset_rect(node_id).expect("box has an offset rect");
    drop(doc);
    [rect.x, rect.y, rect.width, rect.height]
}

#[test]
fn offsets_follow_the_first_column_fragment_like_chrome() {
    let positioned: String = (0..7)
        .map(|index| format!("<div id=\"q{index}\" class=\"b\"></div>"))
        .collect();
    let harness = page(&format!(
        "<div class=\"c\">{}</div><div class=\"c\" style=\"position:relative;\
         margin:10px 0 0 5px;border:3px solid\">{positioned}<div id=\"v\" \
         style=\"writing-mode:vertical-rl;height:30px;width:12px\"></div></div>",
        blocks(7)
    ));
    for (id, expected) in [
        ("b2", [0.0, 40.0, 210.0, 47.0]),
        ("b3", [110.0, 13.0, 100.0, 20.0]),
        ("b4", [110.0, 33.0, 210.0, 47.0]),
        ("b6", [220.0, 27.0, 100.0, 20.0]),
        ("q2", [0.0, 40.0, 210.0, 57.0]),
        ("q3", [110.0, 3.0, 100.0, 20.0]),
        ("q5", [110.0, 43.0, 210.0, 57.0]),
        ("v", [220.0, 27.0, 12.0, 30.0]),
    ] {
        assert_eq!(offset(&harness, id).map(f64::round), expected, "{id}");
    }
}

#[test]
fn scrolled_boxes_in_columns_keep_their_offsets_like_chrome() {
    let mut harness = page(
        "<div class=\"c\" style=\"width:210px;column-count:2;height:60px;column-fill:auto\">\
         <div style=\"height:5px\"></div><div id=\"s1\" style=\"height:50px;overflow:auto\">\
         <div style=\"height:200px\"></div></div></div>\
         <div class=\"c\" style=\"width:210px;column-count:2;height:60px;position:relative;\
         margin-top:20px;border:3px solid\"><div style=\"height:10px\"></div>\
         <div id=\"p2\" style=\"position:relative;border:2px solid;font:16px/20px Arial\">\
         x<br>x<br>x<br><span id=\"i2\">yy</span></div></div>\
         <div class=\"c\" style=\"width:210px;column-count:2;height:60px;position:relative;\
         margin-top:20px\"><div id=\"s3\" style=\"height:50px;overflow:auto\">\
         <div style=\"height:70px\"></div><div id=\"k3\" style=\"height:10px\"></div></div></div>",
    );
    scroll(&mut harness, &[("s1", 17.0), ("s3", 33.0)]);
    for (id, expected) in [
        ("s1", [0.0, 5.0, 100.0, 50.0]),
        ("p2", [0.0, 10.0, 210.0, 52.0]),
        ("s3", [0.0, 0.0, 100.0, 50.0]),
        ("k3", [0.0, 70.0, 100.0, 10.0]),
    ] {
        assert_eq!(offset(&harness, id).map(f64::round), expected, "{id}");
    }
    let [left, top, width, _] = offset(&harness, "i2");
    assert_eq!([left.round(), width.round()], [110.0, 16.0]);
    assert!((top - 9.0).abs() <= 1.5, "i2 offsetTop {top}, Chrome 9");
    for (id, expected) in [
        ("s1", [0.0, 5.0, 100.0, 50.0]),
        ("s3", [0.0, 166.0, 100.0, 50.0]),
        ("k3", [0.0, 206.0, 100.0, 10.0]),
    ] {
        assert_eq!(client_rect(&harness, id), expected, "{id}");
    }
}

#[test]
fn scrolled_content_follows_its_scroller_like_chrome() {
    let mut harness = page(
        "<div class=\"c\" style=\"width:210px;column-count:2;height:60px;column-fill:auto;\
         position:relative\"><div style=\"height:60px\"></div><div id=\"s1\" \
         style=\"height:50px;overflow:auto\"><div style=\"height:5px\"></div>\
         <div id=\"k1\" style=\"height:10px\"></div><div style=\"height:100px\"></div></div></div>\
         <div id=\"s2\" style=\"margin-top:20px;width:100px;height:40px;overflow:auto\">\
         aa<br>bb<br><span id=\"i2\">cc</span><br>dd<br>ee<br>ff</div>\
         <div id=\"s3\" style=\"margin-top:20px;width:100px;height:40px;overflow:auto\">\
         <div style=\"height:30px\"></div><div id=\"k3\" style=\"height:10px\"></div>\
         <div style=\"height:100px\"></div></div>",
    );
    scroll(&mut harness, &[("s1", 20.0), ("s2", 25.0), ("s3", 30.0)]);
    for (id, expected) in [
        ("s1", [110.0, 0.0, 100.0, 50.0]),
        ("k1", [110.0, 5.0, 100.0, 10.0]),
        ("s2", [0.0, 80.0, 100.0, 40.0]),
        ("s3", [0.0, 140.0, 100.0, 40.0]),
        ("k3", [0.0, 170.0, 100.0, 10.0]),
    ] {
        assert_eq!(offset(&harness, id).map(f64::round), expected, "{id}");
    }
    for (id, expected) in [
        ("s1", [110.0, 0.0, 100.0, 50.0]),
        ("k1", [110.0, -15.0, 100.0, 10.0]),
        ("s2", [0.0, 80.0, 100.0, 40.0]),
        ("s3", [0.0, 140.0, 100.0, 40.0]),
        ("k3", [0.0, 140.0, 100.0, 10.0]),
    ] {
        assert_eq!(client_rect(&harness, id).map(f64::round), expected, "{id}");
    }
    let [left, top, width, _] = offset(&harness, "i2");
    let [client_left, client_top, client_width, _] = client_rect(&harness, "i2");
    assert_eq!(
        [left, width, client_left, client_width].map(f64::round),
        [0.0, 16.0, 0.0, 16.0]
    );
    assert_eq!((client_top - top).round(), -25.0);
}

fn scroll(harness: &mut Harness<Page<()>>, targets: &[(&str, f64)]) {
    {
        let mut doc = harness.base_mut();
        for &(id, top) in targets {
            let node_id = doc
                .query_selector_all(&format!("#{id}"))
                .expect("box selector is valid")[0];
            doc.scroll_to(node_id, 0.0, top, ScrollBehavior::Instant);
        }
    }
    harness.pump();
}

fn client_rect(harness: &Harness<Page<()>>, id: &str) -> [f64; 4] {
    let doc = harness.base();
    let node_id = doc
        .query_selector_all(&format!("#{id}"))
        .expect("box selector is valid")[0];
    let rect = doc
        .get_client_bounding_rect(node_id)
        .expect("box has a rectangle");
    drop(doc);
    [rect.x, rect.y, rect.width, rect.height]
}
