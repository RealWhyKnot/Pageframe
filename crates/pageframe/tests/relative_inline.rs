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

fn rects_in_units(harness: &mut Harness<Page<()>>, ids: &[&'static str]) -> Rects {
    let doc = harness.base_mut();
    ids.iter()
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
        let found = rects_in_units(&mut harness, &IDS);
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

#[test]
fn a_relative_span_paints_over_the_text_after_it_like_chrome() {
    let page = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial;color:#f00}</style>\
<div>ab<span style=\"position:relative;left:40px;background:#0f0;color:#00f\">cd</span>WWW</div>";
    let mut harness = open(App::new((), page));
    let frame = render(&mut harness);
    let covered: Vec<[u8; 4]> = (57..70)
        .flat_map(|column| (2..14).map(move |row| (column, row)))
        .map(|(column, row)| pixel(&frame, column, row))
        .collect();
    let red = covered
        .iter()
        .filter(|[red, green, blue, _]| *red > 128 && *green < 128 && *blue < 128)
        .count();
    let blue = covered
        .iter()
        .filter(|[red, green, blue, _]| *red < 128 && *green < 128 && *blue > 128)
        .count();
    assert_eq!(red, 0, "the span covers the text it moved over");
    assert!(blue > 0, "the span's own text is painted");
}

#[test]
fn a_faded_span_inside_a_relative_span_stays_faded() {
    let page = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}</style>\
<div><span style=\"position:relative;left:2px\"><span style=\"opacity:0.5;background:#000;color:#000\">ab</span></span></div>";
    let mut harness = open(App::new((), page));
    let frame = render(&mut harness);
    let [red, green, blue, _] = pixel(&frame, 5, 1);
    assert!(
        (120..=136).contains(&red) && red == green && green == blue,
        "half-transparent black over white: {:?}",
        [red, green, blue]
    );
}

#[test]
fn vertical_percentages_need_a_definite_height_like_chrome() {
    let page = "<!doctype html><style>html,body{margin:0;font:14px Arial}p{margin:0;width:200px;height:50%}</style>\
<div style=\"width:200px;height:100px\">ab<span id=fix style=\"position:relative;top:10%;left:5%\">cd</span></div>\
<div style=\"height:200px\"><p>ab<span id=pcth style=\"position:relative;top:10%\">cd</span></p></div>\
<div><p>ab<span id=autoh style=\"position:relative;top:10%;bottom:5px\">cd</span></p></div>\
<div style=\"width:200px;height:100px\">ab<span id=bot style=\"position:relative;bottom:10%\">cd</span></div>";
    let mut harness = open(App::new((), page));
    render(&mut harness);
    let found: Vec<(&str, [f64; 2])> =
        rects_in_units(&mut harness, &["fix", "pcth", "autoh", "bot"])
            .into_iter()
            .map(|(id, [left, top, _, _])| (id, [left, top]))
            .collect();
    assert_eq!(
        found,
        [
            ("fix", [1637.0, 640.0]),
            ("pcth", [997.0, 7040.0]),
            ("autoh", [997.0, 18880.0]),
            ("bot", [997.0, 19584.0]),
        ]
    );
}

#[test]
fn a_visible_child_of_a_hidden_relative_span_still_paints() {
    let page = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}</style>\
<div>ab<span style=\"position:relative;left:10px;visibility:hidden\">xx<span style=\"visibility:visible;color:#00f\">cd</span></span></div>";
    let mut harness = open(App::new((), page));
    let frame = render(&mut harness);
    let blue = (0..80)
        .flat_map(|column| (0..16).map(move |row| (column, row)))
        .map(|(column, row)| pixel(&frame, column, row))
        .filter(|[red, green, blue, _]| *red < 128 && *green < 128 && *blue > 128)
        .count();
    assert!(blue > 0, "the visible child's text is painted");
}

#[test]
fn the_selection_moves_with_a_relative_span_and_stays_over_its_background() {
    let frame = selected_frame(
        "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial;color:#fff}</style>\
<div>ab<span style=\"position:relative;left:20px;background:#0f0\">cd</span></div>",
    );
    let highlight = [180, 213, 255, 255];
    assert_eq!(
        pixel(&frame, 2, 1),
        highlight,
        "the unmoved text is selected"
    );
    assert_eq!(
        pixel(&frame, 38, 1),
        highlight,
        "the moved text's highlight covers the span background"
    );
    assert_eq!(
        pixel(&frame, 18, 1),
        [255, 255, 255, 255],
        "no highlight where the span sat"
    );
}

fn selected_frame(page: &str) -> Vec<u8> {
    let mut harness = open(App::new((), page));
    let mut doc = harness.base_mut();
    let div = doc
        .query_selector("div")
        .expect("valid selector")
        .expect("the div exists");
    doc.set_text_selection(div, 0, div, usize::MAX);
    drop(doc);
    render(&mut harness)
}

fn highlighted_rows(frame: &[u8], column: usize) -> usize {
    (0..40)
        .filter(|row| pixel(frame, column, *row) == [180, 213, 255, 255])
        .count()
}

#[test]
fn text_under_display_contents_takes_the_relative_highlight_along() {
    let frame = selected_frame(
        "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial;color:#fff}</style>\
<div>ab<span style=\"position:relative;left:30px\">c<span style=\"display:contents\">d</span></span></div>",
    );
    assert_eq!(highlighted_rows(&frame, 26), 0, "where the text sat");
    assert!(highlighted_rows(&frame, 56) > 0, "where the text moved");
}

#[test]
fn hidden_text_gets_no_highlight() {
    let frame = selected_frame(
        "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial;color:#fff}</style>\
<div>ab<span style=\"visibility:hidden\">xx</span>cd<span style=\"position:relative;left:20px;visibility:hidden\">yy</span></div>",
    );
    assert!(highlighted_rows(&frame, 5) > 0, "visible text is selected");
    assert_eq!(highlighted_rows(&frame, 22), 0, "the hidden span");
    assert_eq!(highlighted_rows(&frame, 70), 0, "the moved hidden span");
}
