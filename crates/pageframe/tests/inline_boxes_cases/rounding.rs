use super::*;

#[test]
fn text_before_an_inline_block_rounds_up_like_chrome() {
    let html = "<html><head><style>html,body{margin:0;background:white;font:14px Arial}\
                div{width:400px}.ib{display:inline-block;width:40px;height:30px;margin:2px}</style></head><body>\
                <div>a <span id=one class=ib></span> b <span id=two class=ib></span> c</div>\
                <div>Emoji \u{1F600}\u{1F44D} <span id=three class=ib></span></div>\
                <div><b>a</b><i>b</i> <span id=four class=ib></span></div></body></html>";
    let mut harness = open(App::new((), html));
    render(&mut harness);
    let doc = harness.base_mut();
    let left = |id: &str| {
        let node = doc
            .query_selector(&format!("#{id}"))
            .expect("valid selector")
            .expect("the box exists");
        doc.get_client_bounding_rect(node)
            .expect("the box is laid out")
            .x
    };
    assert_eq!(
        [left("one"), left("two"), left("three"), left("four")],
        [13.6875, 73.265_625, 83.234_375, 21.484_375]
    );
}

#[test]
fn client_rects_snap_to_device_pixel_sixty_fourths_like_chrome() {
    let html = "<html><head><style>html,body{margin:0;background:white;font:14px Arial}\
                div{width:400px}.ib{display:inline-block;width:40px;height:30px;margin:2px}\
                </style></head><body>\
                <div>a <span id=one class=ib></span></div>\
                <div>Emoji \u{1F600}\u{1F44D} <span id=two class=ib></span></div>\
                <div><b>a</b><i>b</i> <span id=three class=ib></span></div></body></html>";
    let corners = |width: u32, height: u32, scale: f32| {
        let mut harness = open(App::new((), html));
        {
            let mut doc = harness.base_mut();
            let color_scheme = doc.get_viewport().color_scheme;
            doc.set_viewport(pageframe_traits::shell::Viewport::new(
                width,
                height,
                scale,
                color_scheme,
            ));
        }
        harness.pump();
        let doc = harness.base_mut();
        ["one", "two", "three"]
            .iter()
            .flat_map(|id| {
                let node = doc
                    .query_selector(&format!("#{id}"))
                    .expect("valid selector")
                    .expect("the box exists");
                let rect = doc
                    .get_client_bounding_rect(node)
                    .expect("the box is laid out");
                [rect.x, rect.y]
            })
            .collect::<Vec<f64>>()
    };
    assert_eq!(
        corners(support::WIDTH * 3 / 2, support::HEIGHT * 3 / 2, 1.5),
        [1313.0, 192.0, 7991.0, 3776.0, 2062.0, 7360.0].map(|steps: f64| steps / 96.0)
    );
    assert_eq!(
        corners(support::WIDTH * 2, support::HEIGHT * 2, 2.0),
        [1751.0, 256.0, 10654.0, 5056.0, 2748.0, 9856.0].map(|steps: f64| steps / 128.0)
    );
}

#[test]
fn wrapped_text_rounds_each_line_up_like_chrome() {
    let html = "<html><head><style>html,body{margin:0;background:white;font:14px Arial}\
                .ib{display:inline-block;width:10px;height:10px}\
                .z{display:inline-block;width:0;height:10px}</style></head><body>\
                <div style=\"width:90px\">fold brown over mild wrap<span id=a class=ib></span></div>\
                <div style=\"width:103px\">lazy tiny ruby lines quick jumps\
                <span id=b class=ib></span></div>\
                <div style=\"width:81px\">wavy here fox lazy tiny ruby<span id=c class=ib></span></div>\
                <div style=\"width:145px;text-align:right\"><span id=d class=z></span>\
                tiny lazy fox here \u{1F600}here fox</div>\
                <div style=\"width:134px;text-align:right\"><span id=e class=z></span>\
                text kilt dogs jumps \u{1F600}jumps dogs</div></body></html>";
    let mut harness = open(App::new((), html));
    render(&mut harness);
    let doc = harness.base_mut();
    let left = |id: &str| {
        let node = doc
            .query_selector(&format!("#{id}"))
            .expect("valid selector")
            .expect("the box exists");
        doc.get_client_bounding_rect(node)
            .expect("the box is laid out")
            .x
            * 64.0
    };
    assert_eq!(
        ["a", "b", "c", "d", "e"].map(left),
        [1943.0, 2391.0, 1743.0, 1077.0, 956.0]
    );
}

#[test]
fn trailing_spaces_split_across_spans_keep_the_min_content_width() {
    let html = "<!doctype html><html><head><style>html,body{margin:0;font:14px Arial}\
                div{width:min-content;white-space:pre-wrap}</style></head><body>\
                <div id=two>wavy quick <b> </b>x</div>\
                <div id=three>wavy quick<i> </i><b> </b>x</div></body></html>";
    let mut harness = open(App::new((), html));
    {
        let mut doc = harness.base_mut();
        let color_scheme = doc.get_viewport().color_scheme;
        doc.set_viewport(pageframe_traits::shell::Viewport::new(
            support::WIDTH * 3 / 2,
            support::HEIGHT * 3 / 2,
            1.5,
            color_scheme,
        ));
    }
    harness.pump();
    let doc = harness.base_mut();
    let width = |id: &str| {
        let node = doc
            .query_selector(&format!("#{id}"))
            .expect("valid selector")
            .expect("the box exists");
        doc.get_client_bounding_rect(node)
            .expect("the box is laid out")
            .width
    };
    assert_eq!(width("three"), width("two"));
}
