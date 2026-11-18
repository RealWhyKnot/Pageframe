mod scaled;
mod support;

use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use scaled::{render_at_ratio, render_at_scale};
use support::{WIDTH, open, pixel, render};

const PAGE: &str = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}</style>\
<div style=\"width:200px\">ab<span id=rel style=\"position:relative;left:3.99px;top:2.49px\">cd<span id=kid>ef</span></span></div>\
<div style=\"width:200px\">ab<span style=\"position:relative;left:5px;top:3px\">cd<span id=in1 style=\"position:relative;left:2px;top:-1px\">ef</span></span></div>\
<div style=\"width:200px\">ab<span id=rgt style=\"position:relative;right:6px;bottom:4px\">cd</span></div>\
<div style=\"width:200px\">ab<span id=pct style=\"position:relative;left:10%;top:10%;background:#0f0\">cd</span></div>\
<div style=\"width:200px;direction:rtl\">ab<span id=rtl style=\"position:relative;left:3px;right:7px\">cd</span></div>\
<div style=\"width:60px\">ab<span id=wrap style=\"position:relative;left:3px;top:2px\">cd efgh ijkl mnop</span></div>\
<div style=\"width:200px;position:relative\">ab<span style=\"position:relative;left:4px;top:2px\">cd<span id=parkid>ef</span></span></div>\
<div style=\"width:200px\">ab<span style=\"position:relative;border:2px solid;padding:0 5px\">cd<span id=bkid>ef</span></span></div>";

const IDS: [&str; 8] = ["rel", "kid", "in1", "rgt", "pct", "rtl", "wrap", "parkid"];

type Ratio = (u16, u16);
type Rects = Vec<(&'static str, [f64; 4])>;

fn rects_in_units(harness: &mut Harness<Page<()>>) -> Rects {
    let doc = harness.base_mut();
    IDS.iter()
        .flat_map(|id| {
            let node = doc
                .query_selector(&format!("#{id}"))
                .expect("valid selector")
                .expect("the span exists");
            doc.node_client_rects(node)
                .into_iter()
                .map(move |rect| {
                    let values = [rect.x, rect.y, rect.width, rect.height];
                    (*id, values.map(|value| value * 64.0))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn render_at(harness: &mut Harness<Page<()>>, ratio: Ratio) {
    match ratio {
        (scale, 1) => render_at_scale(harness, scale),
        ratio => render_at_ratio(harness, ratio, WIDTH),
    };
}

fn chrome_rects() -> [(Ratio, Rects); 3] {
    [
        (
            (1, 1),
            vec![
                ("rel", [1252.0, 159.0, 1695.0, 1024.0]),
                ("kid", [2199.0, 159.0, 748.0, 1024.0]),
                ("in1", [2392.0, 1152.0, 748.0, 1024.0]),
                ("rgt", [613.0, 1792.0, 947.0, 1024.0]),
                ("pct", [2277.0, 3072.0, 947.0, 1024.0]),
                ("rtl", [11405.0, 4096.0, 947.0, 1024.0]),
                ("wrap", [1189.0, 5248.0, 947.0, 1024.0]),
                ("wrap", [192.0, 6272.0, 3038.0, 1024.0]),
                ("wrap", [192.0, 7296.0, 2242.0, 1024.0]),
                ("parkid", [2200.0, 8320.0, 748.0, 1024.0]),
            ],
        ),
        (
            (7, 4),
            vec![
                ("rel", [1252.0, 158.8571, 1694.2858, 987.4286]),
                ("kid", [2198.8572, 158.8571, 747.4286, 987.4286]),
                ("in1", [2392.0, 1152.0, 747.4286, 987.4286]),
                ("rgt", [613.1429, 1792.0001, 946.8572, 987.4286]),
                ("pct", [2277.1431, 3072.0002, 946.8572, 987.4286]),
                ("rtl", [11405.1436, 4096.0, 946.8572, 987.4286]),
                ("wrap", [1189.1429, 5248.0, 946.8572, 987.4286]),
                ("wrap", [192.0, 6272.0005, 3038.2859, 987.4286]),
                ("wrap", [192.0, 7296.0005, 2241.7144, 987.4286]),
                ("parkid", [2200.0, 8320.0, 747.4286, 987.4286]),
            ],
        ),
        (
            (2, 1),
            vec![
                ("rel", [1252.0, 159.0, 1694.0, 992.0]),
                ("kid", [2198.5, 159.0, 747.5, 992.0]),
                ("in1", [2391.5, 1152.0, 747.5, 992.0]),
                ("rgt", [613.0, 1792.0, 946.5, 992.0]),
                ("pct", [2277.0, 3072.0, 946.5, 992.0]),
                ("rtl", [11405.5, 4096.0, 946.5, 992.0]),
                ("wrap", [1189.0, 5248.0, 946.5, 992.0]),
                ("wrap", [192.0, 6272.0, 3038.0, 992.0]),
                ("wrap", [192.0, 7296.0, 2241.5, 992.0]),
                ("parkid", [2199.5, 8320.0, 747.5, 992.0]),
            ],
        ),
    ]
}

#[test]
fn relative_inline_rects_move_with_their_offset_like_chrome() {
    let mut harness = open(App::new((), PAGE));
    let mut wrong = Vec::new();
    for (ratio, chrome) in chrome_rects() {
        render_at(&mut harness, ratio);
        let found = rects_in_units(&mut harness);
        let ids: Vec<&str> = found.iter().map(|(id, _)| *id).collect();
        let expected: Vec<&str> = chrome.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids, expected, "fragment count at {ratio:?}");
        for ((id, have), (_, want)) in found.iter().zip(&chrome) {
            if have
                .iter()
                .zip(want)
                .any(|(have, want)| (have - want).abs() >= 0.01)
            {
                wrong.push(format!("{ratio:?} #{id}: {have:?}, Chrome {want:?}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn relative_inline_offsets_measure_from_the_first_fragment_like_chrome() {
    let mut harness = open(App::new((), PAGE));
    let frame = render(&mut harness);
    assert_eq!(
        pixel(&frame, 42, 56),
        [0, 255, 0, 255],
        "10% of 200px moves #pct"
    );
    assert_eq!(
        pixel(&frame, 25, 56),
        [255, 255, 255, 255],
        "where #pct sat"
    );
    let doc = harness.base_mut();
    let ids = ["rel", "kid", "in1", "pct", "wrap", "parkid", "bkid"];
    let found: Vec<(&str, [f64; 2], String)> = ids
        .into_iter()
        .map(|id| {
            let node = doc
                .query_selector(&format!("#{id}"))
                .expect("valid selector")
                .expect("the span exists");
            let rect = doc.offset_rect(node).expect("the span has a box");
            let parent = doc
                .get_node(node)
                .and_then(|node| node.offset_parent())
                .and_then(|parent| parent.element_data())
                .map(|data| data.name.local.to_string())
                .unwrap_or_default();
            (id, [rect.x.round(), rect.y.round()], parent)
        })
        .collect();
    let chrome = [
        ("rel", [20.0, 2.0], "body"),
        ("kid", [15.0, 0.0], "span"),
        ("in1", [17.0, -1.0], "span"),
        ("pct", [36.0, 48.0], "body"),
        ("wrap", [19.0, 82.0], "body"),
        ("parkid", [15.0, 0.0], "span"),
        ("bkid", [22.0, 2.0], "span"),
    ];
    let found: Vec<(&str, [f64; 2], &str)> = found
        .iter()
        .map(|(id, offset, parent)| (*id, *offset, parent.as_str()))
        .collect();
    assert_eq!(found, chrome);
}
