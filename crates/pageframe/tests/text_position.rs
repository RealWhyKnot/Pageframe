#![cfg(target_os = "windows")]

use pageframe::App;
use pageframe_dom::ScrollBehavior;
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

#[test]
fn clicks_outside_wrapped_text_find_the_offsets_chrome_reports() {
    let html = "<!doctype html><style>html,body{margin:0;font:14px Arial}\
                div{width:60px;height:60px}</style>\
                <div id=d1 style=\"overflow-wrap:anywhere\">aaaa&nbsp;bbbbbbbbbbbbbbbbb</div>\
                <div id=d2 style=\"white-space:pre-wrap\">aaa   bbb   ccc</div>";
    let page = App::new((), html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let doc = harness.base();
    let anywhere = doc.query_selector_all("#d1").expect("selector is valid")[0];
    let preserved = doc.query_selector_all("#d2").expect("selector is valid")[0];
    for (x, y, root, offset) in [
        (100.25, 20.25, anywhere, 15),
        (100.25, 44.25, anywhere, 22),
        (64.25, 64.25, preserved, 11),
        (100.25, 64.25, preserved, 12),
        (100.25, 300.25, preserved, 15),
        (300.25, 10.25, anywhere, 8),
    ] {
        let found = doc
            .find_text_position(x, y)
            .map(|(node, byte)| (node, byte_chars(&doc, node, byte)));
        assert_eq!(found, Some((root, offset)), "{x},{y}");
    }
    drop(doc);
}

#[test]
fn clicks_below_vertical_lr_text_find_the_offsets_chrome_reports() {
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
        (4.25, 80.25, 6),
        (20.25, 100.25, 11),
        (40.25, 100.25, 16),
        (100.25, 100.25, 21),
    ] {
        assert_eq!(
            doc.find_text_position(x, y),
            Some((root, offset)),
            "{x},{y}"
        );
    }
}

#[test]
fn clicks_beside_padded_flex_grid_and_scrolled_boxes_find_the_offsets_chrome_reports() {
    let html = "<!doctype html><style>html,body{margin:0;font:14px Arial}</style>\
        <div id=f1 style=\"width:200px;height:60px;overflow:hidden\">\
        <div id=e1a style=\"width:60px;height:40px\">aaaa</div>\
        <div id=e1b style=\"width:60px;height:40px\">bbbb</div>\
        <div id=e1c style=\"width:60px;height:40px\">cccc</div></div>\
        <div id=e2 style=\"width:100px;padding:20px 10px;border:5px solid #000;height:98px;\
        box-sizing:border-box\">aaaa bbbb cccc dddd eeee ffff</div>\
        <div style=\"display:flex;justify-content:center;width:300px;height:30px;margin-top:2px\">\
        <div id=e3a>aaaa</div><div id=e3b>bbbb</div></div>\
        <div style=\"display:grid;grid-template-columns:100px 100px;width:300px\">\
        <div id=e4a style=\"height:20px\">aaaa</div><div id=e4b style=\"height:40px\">bbbb</div></div>\
        <div id=e5 style=\"width:100px;height:30px;transform:translateY(30px)\">aaaa bbbb</div>\
        <div id=e5s style=\"height:40px;width:100px\">ssss</div>\
        <div style=\"writing-mode:vertical-rl;height:100px;width:200px\">\
        <div id=e6a style=\"height:50px\">aaaa</div><div id=e6b style=\"height:50px\">bbbb</div></div>";
    let page = App::new((), html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    {
        let mut doc = harness.base_mut();
        let scroller = doc.query_selector_all("#f1").expect("selector is valid")[0];
        doc.scroll_to(scroller, 0.0, 50.0, ScrollBehavior::Instant);
    }
    harness.pump();
    let doc = harness.base();
    let node = |id: &str| {
        doc.query_selector_all(&format!("#{id}"))
            .expect("selector is valid")[0]
    };
    for (x, y, id, offset) in [
        (150.25, 8.25, "e1b", 4),
        (300.25, 90.25, "e2", 9),
        (300.25, 104.25, "e2", 19),
        (2.25, 168.25, "e3a", 0),
        (298.25, 168.25, "e3b", 4),
        (50.25, 225.25, "e4a", 4),
        (300.25, 238.25, "e5", 9),
        (300.25, 268.25, "e5s", 4),
        (190.25, 390.25, "e6a", 4),
        (176.25, 390.25, "e6b", 4),
        (100.25, 330.25, "e6b", 4),
    ] {
        assert_eq!(
            doc.find_text_position(x, y),
            Some((node(id), offset)),
            "{x},{y}"
        );
    }
    drop(doc);
}

#[test]
fn clicks_at_line_edges_follow_the_paragraph_direction_like_chrome() {
    let html = "<!doctype html><meta charset=utf-8>\
        <style>html,body{margin:0;font:14px Arial}div{width:200px;height:30px}\
        .narrow{width:60px;height:60px}\
        .box{display:inline-block;width:10px;height:10px}</style>\
        <div id=e1>אבגד הוזח</div>\
        <div id=e2 dir=rtl>abcd efgh</div>\
        <div id=e3>abc אבג</div>\
        <div id=e4>אבג abc</div>\
        <div id=e5 dir=rtl>abc אבג</div>\
        <div id=e6 dir=rtl>אבג abc</div>\
        <div id=e7 dir=rtl class=narrow>aaaa bbbb cccc</div>\
        <div id=e8 class=narrow>אבגד הוזח טיכל</div>\
        <div id=e9 dir=rtl>xx<span style=\"position:relative;left:30px\">abcdef</span>yy</div>\
        <div id=e10 dir=rtl class=narrow>aaaa bbbb \
        <span style=\"position:relative;left:4px\">cccc</span></div>\
        <div id=e11>אבגד <span style=\"position:relative;left:4px\">הוזח</span></div>\
        <div id=e12><span class=box></span> abcd</div>\
        <div id=e13 dir=rtl>abcd <span class=box></span></div>\
        <div id=e14 dir=rtl><span class=box></span> abcd</div>\
        <div id=e15>abcd <span class=box></span></div>";
    let page = App::new((), html)
        .page(640, 600)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let doc = harness.base();
    for (x, y, id, offset) in [
        (2.25, 8.25, "e1", 0),
        (196.25, 8.25, "e1", 9),
        (2.25, 38.25, "e2", 9),
        (196.25, 38.25, "e2", 0),
        (196.25, 68.25, "e3", 7),
        (2.25, 98.25, "e4", 0),
        (400.25, 128.25, "e5", 0),
        (2.25, 158.25, "e6", 7),
        (58.25, 188.25, "e7", 0),
        (58.25, 204.25, "e7", 5),
        (2.25, 248.25, "e8", 0),
        (2.25, 264.25, "e8", 10),
        (58.25, 264.25, "e8", 14),
        (2.25, 308.25, "e9", 10),
        (400.25, 308.25, "e9", 8),
        (58.25, 338.25, "e10", 0),
        (58.25, 354.25, "e10", 5),
        (2.25, 398.25, "e11", 0),
        (196.25, 398.25, "e11", 9),
        (2.25, 428.25, "e12", 0),
        (196.25, 428.25, "e12", 5),
        (2.25, 458.25, "e13", 5),
        (196.25, 458.25, "e13", 0),
        (2.25, 488.25, "e14", 5),
        (196.25, 488.25, "e14", 0),
        (400.25, 488.25, "e14", 0),
        (196.25, 518.25, "e15", 5),
        (400.25, 518.25, "e15", 5),
    ] {
        let root = doc
            .query_selector_all(&format!("#{id}"))
            .expect("selector is valid")[0];
        let found = doc
            .find_text_position(x, y)
            .map(|(node, byte)| (node, byte_chars(&doc, node, byte)));
        assert_eq!(found, Some((root, offset)), "{x},{y}");
    }
    drop(doc);
}

#[test]
fn text_overflowing_into_a_later_block_is_hit_before_its_background_like_chrome() {
    let html = "<!doctype html><style>html,body{margin:0;font:14px Arial}\
                div{width:60px;height:40px}</style>\
                <div id=d1>bbbb cccc dd<span>ee ff</span> gggg</div>\
                <div id=d2 style=\"text-align:right\">bbbb cccc dd ee</div>";
    let page = App::new((), html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let doc = harness.base();
    let first = doc.query_selector_all("#d1").expect("selector is valid")[0];
    let second = doc.query_selector_all("#d2").expect("selector is valid")[0];
    for (x, y, root, offset) in [
        (0.25, 40.25, first, 10),
        (12.25, 44.25, first, 12),
        (4.25, 50.25, first, 19),
        (24.25, 52.25, first, 21),
        (8.25, 58.25, first, 19),
        (34.25, 44.25, second, 1),
        (16.25, 60.25, second, 6),
    ] {
        assert_eq!(
            doc.find_text_position(x, y),
            Some((root, offset)),
            "{x},{y}"
        );
    }
}

#[test]
fn clicks_at_bidi_run_boundaries_land_in_the_base_direction_run_like_chrome() {
    let html = "<!doctype html><meta charset=utf-8>\
        <style>html,body{margin:0;font:14px Arial}div{width:200px;height:30px}\
        .box{display:inline-block;width:20px;height:10px}</style>\
        <div id=b5 dir=rtl><span class=box></span> abcd</div>\
        <div id=b6 dir=rtl>abcd <span class=box></span></div>\
        <div id=b7>abcd <span class=box></span></div>\
        <div id=d3 dir=rtl style=\"width:60px;height:40px\">bbbb cccc dd ee</div>\
        <div id=m4>abc <span class=box></span>\u{5d0}\u{5d1}\u{5d2}</div>\
        <div id=m5>abc \u{5d0}\u{5d1}\u{5d2}<span class=box></span></div>";
    let page = App::new((), html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let doc = harness.base();
    for (x, y, id, offset) in [
        (150.25, 8.25, "b5", 2),
        (172.25, 8.25, "b5", 1),
        (176.25, 8.25, "b5", 1),
        (182.25, 8.25, "b5", 0),
        (160.25, 38.25, "b6", 5),
        (170.25, 38.25, "b6", 4),
        (174.25, 38.25, "b6", 1),
        (28.25, 68.25, "b7", 4),
        (36.25, 68.25, "b7", 5),
        (40.25, 68.25, "b7", 5),
        (30.25, 94.25, "d3", 4),
        (14.25, 110.25, "d3", 12),
        (28.25, 138.25, "m4", 4),
        (44.25, 138.25, "m4", 4),
        (48.25, 138.25, "m4", 4),
        (50.25, 138.25, "m4", 6),
        (30.25, 168.25, "m5", 6),
        (44.25, 168.25, "m5", 7),
    ] {
        let root = doc
            .query_selector_all(&format!("#{id}"))
            .expect("selector is valid")[0];
        let found = doc
            .find_text_position(x, y)
            .map(|(node, byte)| (node, byte_chars(&doc, node, byte)));
        assert_eq!(found, Some((root, offset)), "{x},{y}");
    }
    drop(doc);
}

#[test]
fn later_siblings_hit_in_chrome_paint_phase_order() {
    let text = "<div id=t style=\"width:60px;height:20px\">aaaa bbbb cccc dddd</div>";
    for (later, text_wins) in [
        (
            "<div><div id=l style=\"float:left;width:50px;height:50px\">ffff ffff</div></div>",
            true,
        ),
        (
            "<div id=l style=\"overflow:hidden;height:60px;text-align:right\">zz</div>",
            true,
        ),
        (
            "<div style=\"display:table\"><div id=l style=\"display:table-cell;width:50px;height:50px\"></div></div>",
            true,
        ),
        (
            "<div style=\"overflow:hidden\"><div id=l style=\"height:50px\"></div></div>",
            true,
        ),
        (
            "<input id=l style=\"display:block;width:50px;height:40px;margin:0\">",
            true,
        ),
        (
            "<svg id=l style=\"display:block\" width=50 height=50><rect width=50 height=50 /></svg>",
            false,
        ),
        (
            "<div style=\"display:flex\"><div id=l style=\"width:50px;height:50px\"></div></div>",
            false,
        ),
        (
            "<div style=\"display:grid\"><div id=l style=\"width:50px;height:50px\"></div></div>",
            false,
        ),
        (
            "<div><span id=l style=\"display:inline-block;width:50px;height:40px\"></span></div>",
            false,
        ),
        ("<div id=l style=\"opacity:0.5;height:50px\"></div>", false),
        (
            "<div id=l style=\"position:relative;height:50px\"></div>",
            false,
        ),
    ] {
        let html = format!(
            "<!doctype html><style>html,body{{margin:0;font:14px Arial}}</style>{text}{later}"
        );
        let page = App::new((), &html)
            .page(640, 400)
            .expect("test pages have valid selectors");
        let mut harness = Harness::wrap(page);
        harness.pump();
        let doc = harness.base();
        let owner = doc
            .hit(4.25, 36.25)
            .and_then(|hit| {
                let mut node = doc.get_node(hit.node_id);
                while let Some(current) = node {
                    if let Some(id) = current.attr(pageframe_dom::local_name!("id")) {
                        return Some(id.to_string());
                    }
                    node = current.parent.and_then(|parent| doc.get_node(parent));
                }
                None
            })
            .expect("the point is over an element with an id");
        assert_eq!(owner == "t", text_wins, "{later}");
        drop(doc);
    }
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
