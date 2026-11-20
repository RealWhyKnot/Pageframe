#![cfg(target_os = "windows")]

use pageframe::{App, Page};
use pageframe_dom::BaseDocument;
use pageframe_test_harness::Harness;
use pageframe_traits::shell::Viewport;
use parley::PositionedLayoutItem;

const HTML: &str = "<!doctype html><meta charset=utf-8><style>\
    html,body{margin:0;font:14px/24px Arial}div{width:90px;margin-left:10px}\
    span{background:lime}</style>\
    <div dir=rtl style='text-align:left'>שלום <span>עולם </span>טוב מאוד</div>";

fn open(html: &str, scale: u16) -> Harness<Page<()>> {
    let page = App::new((), html)
        .page(640, 600)
        .expect("valid text fixture");
    let mut harness = Harness::wrap(page);
    harness.pump();
    {
        let mut doc = harness.base_mut();
        let scheme = doc.get_viewport().color_scheme;
        doc.set_viewport(Viewport::new(
            640 * u32::from(scale),
            600 * u32::from(scale),
            f32::from(scale),
            scheme,
        ));
    }
    harness.pump();
    harness
}

fn line_glyphs(doc: &BaseDocument, scale: u16) -> Vec<Vec<(u32, f64)>> {
    let div = doc
        .query_selector("div")
        .expect("valid selector")
        .expect("fixture paragraph");
    let node = doc.get_node(div).expect("paragraph node");
    let left = node.absolute_position(0.0, 0.0).x * f32::from(scale);
    let data = node
        .element_data()
        .expect("paragraph element")
        .inline_layout_data
        .as_ref()
        .expect("paragraph has inline layout");
    let mut lines = Vec::new();
    for line in data.layout.lines() {
        let mut origins = Vec::new();
        for item in line.items() {
            if let PositionedLayoutItem::GlyphRun(run) = item {
                origins.extend(
                    run.positioned_glyphs()
                        .filter(|g| g.advance > 0.0 && g.x >= 0.0)
                        .map(|g| (g.id, f64::from(g.x + left))),
                );
            }
        }
        lines.push(origins);
    }
    lines
}

fn check_positions(scale: u16, span_edges: [f64; 2], expected: [&[f64]; 2]) {
    let harness = open(HTML, scale);
    let doc = harness.base();
    let span = doc
        .query_selector("span")
        .expect("valid selector")
        .expect("fixture span");
    let rects: Vec<_> = doc
        .inline_fragment_rects(span)
        .expect("span has fragments")
        .collect();
    assert_eq!(rects.len(), 1);
    assert_eq!([rects[0].x, rects[0].x + rects[0].width], span_edges);

    let lines = line_glyphs(&doc, scale);
    drop(doc);
    let origins: Vec<Vec<_>> = lines
        .into_iter()
        .map(|line| line.into_iter().map(|(_, x)| x).collect())
        .collect();
    assert_eq!(origins, expected);
}

#[test]
fn left_aligned_hebrew_at_1x() {
    check_positions(
        1,
        [29.3125, 58.9375],
        [
            &[
                10.0,
                17.587890625,
                21.0400390625,
                29.3125,
                33.2021484375,
                41.5830078125,
                48.0634765625,
                51.515625,
                58.9375,
                62.8271484375,
                71.2080078125,
                74.66015625,
                81.140625,
            ],
            &[10.0, 17.1162109375, 20.568359375, 28.4501953125],
        ],
    );
}

#[test]
fn left_aligned_hebrew_at_2x() {
    check_positions(
        2,
        [29.3046875, 58.921875],
        [
            &[
                20.0,
                35.17578125,
                42.080078125,
                58.609375,
                66.388671875,
                83.150390625,
                96.111328125,
                103.015625,
                117.84375,
                125.623046875,
                142.384765625,
                149.2890625,
                162.25,
            ],
            &[20.0, 34.232421875, 41.13671875, 56.900390625],
        ],
    );
}

fn check_single_line(direction: &str, text: &str, glyphs: &[(u32, [f64; 2])]) {
    let html = format!(
        "<!doctype html><meta charset=utf-8><style>\
        html,body{{margin:0;font:14px/24px Arial}}div{{margin-left:10px;width:200px;text-align:left}}\
        </style><div dir={direction}>{text}</div>"
    );
    for (column, scale) in [1, 2].into_iter().enumerate() {
        let harness = open(&html, scale);
        let actual = line_glyphs(&harness.base(), scale);
        let expected: Vec<_> = glyphs.iter().map(|&(id, x)| (id, x[column])).collect();
        assert_eq!(actual, [expected], "{direction}, {text}, {scale}x");
    }
}

#[test]
fn hebrew_with_digits() {
    for direction in ["rtl", "ltr"] {
        check_single_line(
            direction,
            "אב12גד",
            &[
                (675, [10.0, 20.0]),
                (674, [17.1162109375, 34.232421875]),
                (20, [22.703125, 45.40625]),
                (21, [30.4892578125, 60.978515625]),
                (673, [38.28125, 76.5625]),
                (672, [45.869140625, 91.73828125]),
            ],
        );
    }
}

#[test]
fn rtl_hebrew_with_latin() {
    check_single_line(
        "rtl",
        "אבxyגד",
        &[
            (675, [10.0, 20.0]),
            (674, [17.1162109375, 34.232421875]),
            (91, [22.703125, 45.40625]),
            (92, [29.703125, 59.40625]),
            (673, [36.703125, 73.40625]),
            (672, [44.291015625, 88.58203125]),
        ],
    );
}

#[test]
fn ltr_hebrew_with_latin() {
    check_single_line(
        "ltr",
        "אבxyגד",
        &[
            (673, [10.0, 20.0]),
            (672, [17.587890625, 35.17578125]),
            (91, [25.484375, 50.953125]),
            (92, [32.484375, 64.953125]),
            (675, [39.484375, 78.953125]),
            (674, [46.6005859375, 93.185546875]),
        ],
    );
}

#[test]
fn ltr_latin_with_digits() {
    check_single_line(
        "ltr",
        "ab12cd",
        &[
            (68, [10.0, 20.0]),
            (69, [17.7861328125, 35.572265625]),
            (20, [25.572265625, 51.14453125]),
            (21, [33.3583984375, 66.716796875]),
            (70, [41.14453125, 82.2890625]),
            (71, [48.14453125, 96.2890625]),
        ],
    );
}
