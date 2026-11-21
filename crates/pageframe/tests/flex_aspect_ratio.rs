use pageframe::{App, Page};
use pageframe_test_harness::Harness;

const STYLE: &str = "html,body{margin:0;font:14px Arial}.w{width:234px;margin-bottom:4px}";

fn page(body: &str) -> Harness<Page<()>> {
    let html = format!(
        "<!doctype html><html><head><style>{STYLE}</style></head><body>{body}</body></html>"
    );
    let page = App::new((), &html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    harness
}

fn size(harness: &Harness<Page<()>>, id: &str) -> [f64; 2] {
    let doc = harness.base();
    let node_id = doc
        .query_selector_all(&format!("#{id}"))
        .expect("box selector is valid")[0];
    let rect = doc
        .get_client_bounding_rect(node_id)
        .expect("box has a client rect");
    drop(doc);
    [rect.width, rect.height]
}

fn assert_sizes(body: &str, expected: &[(&str, [f64; 2])]) {
    let harness = page(body);
    for (id, size_expected) in expected {
        assert_eq!(size(&harness, id), *size_expected, "{id}");
    }
}

#[test]
fn flex_and_grid_containers_take_their_height_from_aspect_ratio() {
    assert_sizes(
        "<div class=w><div id=thumb style=\"position:relative;display:flex;aspect-ratio:16/9;\
         overflow:hidden;align-items:center;justify-content:center\"><span>ph</span></div></div>\
         <div class=w><div id=grid style=\"display:grid;aspect-ratio:16/9\"><span>x</span></div></div>\
         <div style=\"display:flex;flex-direction:column;width:234px\">\
         <div id=column style=\"display:flex;aspect-ratio:16/9\">k</div></div>\
         <div class=w style=\"display:flex\"><div id=grow style=\"display:flex;aspect-ratio:16/9;flex:1\">j</div></div>",
        &[
            ("thumb", [234.0, 131.625]),
            ("grid", [234.0, 131.625]),
            ("column", [234.0, 131.625]),
            ("grow", [234.0, 131.625]),
        ],
    );
}

#[test]
fn aspect_ratio_respects_box_sizing() {
    assert_sizes(
        "<div class=w><div id=border style=\"display:flex;aspect-ratio:16/9;padding:10px;\
         border:2px solid;box-sizing:border-box\">h</div></div>\
         <div class=w><div id=content style=\"display:flex;aspect-ratio:16/9;padding:10px;\
         border:2px solid\">i</div></div>",
        &[("border", [234.0, 131.625]), ("content", [234.0, 142.125])],
    );
}

#[test]
fn tall_content_grows_the_box_unless_it_scrolls() {
    assert_sizes(
        "<div class=w><div id=visible style=\"display:flex;flex-direction:column;aspect-ratio:16/9\">\
         <div style=\"height:200px\"></div></div></div>\
         <div class=w><div id=hidden style=\"display:flex;aspect-ratio:16/9;overflow:hidden\">\
         <div style=\"height:200px;width:10px\"></div></div></div>\
         <div class=w><div id=block style=\"display:grid;aspect-ratio:16/9\">\
         <div style=\"height:200px\"></div></div></div>",
        &[
            ("visible", [234.0, 200.0]),
            ("hidden", [234.0, 131.625]),
            ("block", [234.0, 200.0]),
        ],
    );
}

#[test]
fn vertical_writing_mode_derives_the_width() {
    assert_sizes(
        "<div style=\"height:120px;writing-mode:vertical-rl\">\
         <div id=vertical style=\"display:flex;aspect-ratio:2\">e</div></div>",
        &[("vertical", [240.0, 120.0])],
    );
}
