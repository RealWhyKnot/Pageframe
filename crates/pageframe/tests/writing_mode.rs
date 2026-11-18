use pageframe::{App, Page};
use pageframe_test_harness::Harness;

const STYLE: &str = "html,body{margin:0;background:#fff;font:16px/18px Arial}\
.c{width:200px;height:100px;border:2px solid #999;padding:3px 5px 7px 11px;margin:4px}\
.c div{margin:2px 3px 4px 6px;border:1px solid red}";

fn page(body: &str) -> Harness<Page<()>> {
    let html = format!("<html><head><style>{STYLE}</style></head><body>{body}</body></html>");
    let page = App::new((), &html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    harness
}

fn rect(harness: &Harness<Page<()>>, id: &str) -> [f64; 4] {
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

#[test]
fn vertical_blocks_stack_along_the_block_axis_like_chrome() {
    let harness = page(
        "<div id=rl class=c style=\"writing-mode:vertical-rl\">\
         <div id=rl1 style=\"width:20px;height:40px\"></div><div id=rl2 style=\"width:30px\"></div>\
         <div id=rl3 style=\"width:10.5px;height:25%\"></div></div>\
         <div id=lr class=c style=\"writing-mode:vertical-lr\">\
         <div id=lr1 style=\"width:20px;height:40px\"></div><div id=lr2 style=\"width:30px\"></div>\
         <div id=lr3 style=\"width:10.5px;height:25%\"></div></div>\
         <div id=hz class=c><div id=or style=\"writing-mode:vertical-rl;height:60px\">ab</div>\
         <div id=ol style=\"writing-mode:vertical-lr;width:50px;height:30px\"></div></div>",
    );
    for (id, expected) in [
        ("rl", [4.0, 4.0, 220.0, 114.0]),
        ("rl1", [192.0, 11.0, 22.0, 42.0]),
        ("rl2", [154.0, 11.0, 32.0, 94.0]),
        ("rl3", [135.5, 11.0, 12.5, 27.0]),
        ("lr", [4.0, 122.0, 220.0, 114.0]),
        ("lr1", [23.0, 129.0, 22.0, 42.0]),
        ("lr2", [51.0, 129.0, 32.0, 94.0]),
        ("lr3", [89.0, 129.0, 12.5, 27.0]),
        ("or", [23.0, 247.0, 20.0, 62.0]),
        ("ol", [23.0, 313.0, 52.0, 32.0]),
    ] {
        assert_eq!(rect(&harness, id), expected, "{id}");
    }
}

#[test]
fn computed_sizes_inside_a_vertical_box_stay_physical() {
    let harness = page(
        "<div class=c style=\"writing-mode:vertical-rl\">\
         <div id=fixed style=\"width:20px;height:40px\"></div>\
         <div id=auto style=\"width:30px;padding:1px 2px 3px 4px\"></div></div>",
    );
    let doc = harness.base();
    let value = |id: &str, property: &str| {
        let node_id = doc
            .query_selector_all(&format!("#{id}"))
            .expect("box selector is valid")[0];
        doc.resolved_style_value(node_id, property)
    };
    assert_eq!(value("fixed", "width"), "20px");
    assert_eq!(value("fixed", "height"), "40px");
    assert_eq!(value("auto", "width"), "30px");
    assert_eq!(value("auto", "height"), "88px");
}
