#![cfg(target_os = "windows")]

use pageframe::App;
use pageframe_test_harness::Harness;

#[test]
fn clicks_in_centred_text_find_the_line_under_them_like_chrome() {
    let html = "<html><head><style>html,body{margin:0;font:16px/20px Arial}\
                div{height:100px;width:200px;align-content:center}</style></head>\
                <body><div id=\"d\">aaa<br>bbb</div></body></html>";
    let page = App::new((), html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let doc = harness.base();
    let root = doc.query_selector_all("#d").expect("selector is valid")[0];
    for (x, y, offset) in [
        (10.0, 40.0, 1),
        (10.0, 55.0, 5),
        (10.0, 65.0, 5),
        (20.0, 55.0, 6),
    ] {
        assert_eq!(
            doc.find_text_position(x, y),
            Some((root, offset)),
            "{x},{y}"
        );
    }
}

#[test]
fn clicks_on_and_around_moved_spans_find_the_offsets_chrome_reports() {
    let html = "<!doctype html><style>html,body{margin:0;font:14px Arial}\
                div{width:150px;height:40px}</style>\
                <div id=d1>xx<span style=\"position:relative;left:30px;top:20px\">abcdef</span>yy</div>\
                <div id=d2>xx<span style=\"position:relative;left:-10px\">ab<span \
                style=\"position:relative;top:16px;left:20px\">cdef</span>gh</span>yy</div>";
    let page = App::new((), html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let doc = harness.base();
    let first = doc.query_selector_all("#d1").expect("selector is valid")[0];
    let second = doc.query_selector_all("#d2").expect("selector is valid")[0];
    for (x, y, root, offset) in [
        (50.25, 24.25, first, 3),
        (30.25, 4.25, first, 2),
        (62.25, 4.25, first, 9),
        (110.25, 4.25, first, 8),
        (110.25, 24.25, first, 8),
        (30.25, 44.25, second, 4),
        (66.25, 44.25, second, 10),
        (56.25, 60.25, second, 6),
        (14.25, 60.25, second, 3),
    ] {
        assert_eq!(
            doc.find_text_position(x, y),
            Some((root, offset)),
            "{x},{y}"
        );
    }
}

#[test]
fn clicks_on_vertical_lr_lines_find_the_offsets_chrome_reports() {
    let html = "<!doctype html><style>html,body{margin:0;font:14px Arial}div{width:150px}</style>\
                <div id=d1 style=\"writing-mode:vertical-lr;height:40px\">xx<span \
                style=\"position:relative;top:6px\">aaaa</span> <span>bbbb</span> \
                <span>cccc</span> <span>dddd</span></div>";
    let page = App::new((), html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let doc = harness.base();
    let root = doc.query_selector_all("#d1").expect("selector is valid")[0];
    for (x, y, offset) in [
        (2.25, 24.25, 3),
        (20.25, 14.25, 9),
        (56.25, 14.25, 19),
        (56.25, 20.25, 20),
        (90.25, 28.25, 21),
        (20.25, 36.25, 11),
        (36.25, 34.25, 16),
    ] {
        assert_eq!(
            doc.find_text_position(x, y),
            Some((root, offset)),
            "{x},{y}"
        );
    }
}

#[test]
fn clicks_past_a_wrapped_line_land_before_its_trailing_space_like_chrome() {
    let html = "<!doctype html><style>html,body{margin:0;font:14px Arial}</style>\
                <div id=d1 style=\"width:60px\">bbbb cccc dd<span>ee ff</span> gggg</div>";
    let page = App::new((), html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let doc = harness.base();
    let root = doc.query_selector_all("#d1").expect("selector is valid")[0];
    for (x, y, offset) in [
        (32.25, 4.25, 4),
        (50.25, 4.25, 4),
        (50.25, 20.25, 9),
        (50.25, 34.25, 17),
        (2.25, 4.25, 0),
    ] {
        assert_eq!(
            doc.find_text_position(x, y),
            Some((root, offset)),
            "{x},{y}"
        );
    }
}

#[test]
fn clicks_past_rtl_and_shifted_wrapped_lines_land_before_the_trailing_space_like_chrome() {
    let html = "<!doctype html><style>html,body{margin:0;font:14px Arial}\
                div{width:60px;height:60px}</style>\
                <div id=d1 dir=rtl>אבגד הוזח טיכל מנסע</div>\
                <div id=d2>bbbb <span style=\"position:relative;left:3px\">cc</span> dddd eeee</div>";
    let page = App::new((), html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let doc = harness.base();
    let rtl = doc.query_selector_all("#d1").expect("selector is valid")[0];
    let shifted = doc.query_selector_all("#d2").expect("selector is valid")[0];
    for (x, y, root, offset) in [
        (0.25, 4.25, rtl, 9),
        (56.25, 64.25, shifted, 7),
        (56.25, 80.25, shifted, 12),
    ] {
        let found = doc
            .find_text_position(x, y)
            .map(|(node, byte)| (node, byte_chars(&doc, node, byte)));
        assert_eq!(found, Some((root, offset)), "{x},{y}");
    }
    drop(doc);
}

fn byte_chars(
    doc: &pageframe_dom::BaseDocument,
    node: pageframe_dom::NodeId,
    byte: usize,
) -> usize {
    doc.get_node(node)
        .and_then(|node| node.element_data())
        .and_then(|element| element.inline_layout_data.as_ref())
        .and_then(|data| data.text.get(..byte))
        .map_or(0, |text| {
            text.chars().filter(|ch| u32::from(*ch) < 0x2000).count()
        })
}
