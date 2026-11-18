mod support;

use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use support::{open, pixel, render};

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

fn ink_columns(buffer: &[u8], left: usize, right: usize) -> Vec<[usize; 4]> {
    let is_ink = |across: usize, down: usize| {
        let [red, green, blue, _] = pixel(buffer, across, down);
        red.max(green).max(blue) < 128
    };
    let mut columns: Vec<[usize; 4]> = Vec::new();
    for across in (left..right).filter(|&across| (0..60).any(|down| is_ink(across, down))) {
        match columns.last_mut() {
            Some(column) if column[1] == across => column[1] = across + 1,
            _ => columns.push([across, across + 1, 0, 0]),
        }
    }
    for column in &mut columns {
        let rows: Vec<usize> = (0..60)
            .filter(|&down| (column[0]..column[1]).any(|across| is_ink(across, down)))
            .collect();
        column[2] = rows.first().copied().unwrap_or(0);
        column[3] = rows.last().map_or(0, |last| last + 1);
    }
    columns
}

#[test]
fn vertical_text_paints_in_columns_like_chrome() {
    let html = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}\
                div{float:left;width:46px;height:36px;margin:4px 8px 4px 4px}</style>\
                <div style=\"writing-mode:vertical-rl\">HHH II</div>\
                <div style=\"writing-mode:vertical-lr\">HHH II</div>";
    let mut harness = open(App::new((), html));
    let buffer = render(&mut harness);
    let chrome = [
        [[21, 31, 5, 10], [37, 47, 5, 33]],
        [[65, 75, 5, 33], [81, 91, 5, 10]],
    ];
    for ((left, right), expected) in [(0, 58), (58, 120)].into_iter().zip(chrome) {
        let columns = ink_columns(&buffer, left, right);
        assert_eq!(
            columns.len(),
            2,
            "two columns in {left}..{right}: {columns:?}"
        );
        for (column, chrome_column) in columns.iter().zip(expected) {
            assert!(
                column
                    .iter()
                    .zip(chrome_column)
                    .all(|(ours, theirs)| ours.abs_diff(theirs) <= 1),
                "ink {column:?}, Chrome {chrome_column:?}"
            );
        }
    }
}

#[test]
fn a_vertical_root_reports_physical_computed_sizes() {
    let html = "<!doctype html><html style=\"writing-mode:vertical-rl\"><head><style>\
                html,body{margin:0;font:16px/18px Arial}</style></head>\
                <body><div style=\"width:30px;height:40px\"></div></body></html>";
    let page = App::new((), html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let doc = harness.base();
    let root = doc
        .query_selector_all("html")
        .expect("root selector is valid")[0];
    assert_eq!(doc.resolved_style_value(root, "width"), "30px");
    assert_eq!(doc.resolved_style_value(root, "height"), "400px");
}

fn ink_rows(buffer: &[u8], left: usize, right: usize, height: usize) -> Vec<[usize; 2]> {
    let mut rows: Vec<[usize; 2]> = Vec::new();
    for down in (0..height).filter(|&down| {
        (left..right).any(|across| {
            let [red, green, blue, _] = pixel(buffer, across, down);
            red.max(green).max(blue) < 128
        })
    }) {
        match rows.last_mut() {
            Some(row) if row[1] == down => row[1] = down + 1,
            _ => rows.push([down, down + 1]),
        }
    }
    rows
}

#[test]
fn upright_letters_stand_in_their_own_cells_like_chrome() {
    let html = "<!doctype html><style>html,body{margin:0;background:#fff;font:16px Arial}\
                div{float:left;width:30px;height:70px;margin:4px 8px 4px 4px;\
                writing-mode:vertical-rl;text-orientation:upright}</style><div>HHH</div>";
    let mut harness = open(App::new((), html));
    let buffer = render(&mut harness);
    assert_eq!(ink_columns(&buffer, 0, 42), [[21, 30, 6, 52]]);
    assert_eq!(ink_rows(&buffer, 0, 42, 78), [[6, 18], [23, 35], [40, 52]]);
}

fn near(ours: &[[usize; 2]], chrome: &[[usize; 2]]) -> bool {
    ours.len() == chrome.len()
        && ours
            .iter()
            .flatten()
            .zip(chrome.iter().flatten())
            .all(|(ours, theirs)| ours.abs_diff(*theirs) <= 1)
}

#[test]
fn han_stands_upright_in_mixed_vertical_text_like_chrome() {
    let html = "<!doctype html><style>html,body{margin:0;background:#fff;font:16px Arial}\
                div{float:left;width:30px;height:70px;margin:4px 8px 4px 4px;\
                writing-mode:vertical-rl}</style><div>一一一</div>";
    let mut harness = open(App::new((), html));
    let buffer = render(&mut harness);
    let [[left, right, ..]] = ink_columns(&buffer, 0, 42)[..] else {
        panic!("one column of ink");
    };
    let rows = ink_rows(&buffer, 0, 42, 78);
    assert!(
        near(&[[left, right]], &[[17, 32]]),
        "columns {left}..{right}"
    );
    assert!(
        near(&rows, &[[11, 12], [27, 28], [43, 44]]),
        "rows {rows:?}"
    );
}

#[test]
fn changing_text_orientation_reshapes_the_text() {
    let html = "<!doctype html><style>html,body{margin:0;background:#fff;font:16px Arial}\
                div{float:left;width:30px;height:70px;margin:4px 8px 4px 4px;\
                writing-mode:vertical-rl}</style><div id=t>HHH</div>";
    let mut harness = open(App::new((), html));
    let before = render(&mut harness);
    assert_ne!(ink_rows(&before, 0, 42, 78), [[6, 18], [23, 35], [40, 52]]);
    let id = harness
        .base()
        .query_selector_all("#t")
        .expect("the selector parses")[0];
    let name = pageframe_dom::QualName::new(
        None,
        pageframe_dom::ns!(),
        pageframe_dom::local_name!("style"),
    );
    harness
        .base_mut()
        .mutate()
        .set_attribute(id, name, "text-orientation:upright");
    harness.pump();
    let after = render(&mut harness);
    assert_eq!(ink_rows(&after, 0, 42, 78), [[6, 18], [23, 35], [40, 52]]);
}

fn ink_across(buffer: &[u8], top: usize, bottom: usize) -> [usize; 2] {
    let inked: Vec<usize> = (0..60)
        .filter(|&across| {
            (top..bottom).any(|down| {
                let [red, green, blue, _] = pixel(buffer, across, down);
                red.max(green).max(blue) < 128
            })
        })
        .collect();
    [
        inked.first().copied().unwrap_or(0),
        inked.last().map_or(0, |last| last + 1),
    ]
}

#[test]
fn mixed_vertical_runs_share_the_central_baseline_like_chrome() {
    let html = "<!doctype html><style>html,body{margin:0;background:#fff;font:16px Arial}\
                div{float:left;height:70px;margin:4px 8px 4px 4px;writing-mode:vertical-rl}\
                </style><div>H<span style=\"font-size:32px\">H</span>H</div>";
    let buffer = render(&mut open(App::new((), html)));
    assert_eq!(ink_across(&buffer, 4, 16), [18, 29]);
    assert_eq!(ink_across(&buffer, 20, 38), [12, 35]);
}

#[test]
fn fallback_han_sits_on_the_strut_baseline_like_chrome() {
    let html = "<!doctype html><style>html,body{margin:0;background:#fff;font:16px Arial}\
                div{float:left;height:70px;margin:4px 8px 4px 4px;writing-mode:vertical-rl}\
                </style><div>漢漢</div>";
    let buffer = render(&mut open(App::new((), html)));
    assert_eq!(ink_across(&buffer, 4, 60), [8, 23]);
}

#[test]
fn vertical_boxes_inside_columns_report_their_column_rects_like_chrome() {
    let html = "<!doctype html><style>html,body{margin:0}\
                .mc{columns:2;column-gap:20px;width:300px;height:100px;margin:4px}\
                .blk{height:60px}.v{writing-mode:vertical-rl;height:50px}\
                .v div{width:12px;height:20px;margin:3px}</style>\
                <div class=mc><div class=blk></div><div class=blk></div>\
                <div class=v id=v1><div id=c1></div><div id=c2></div></div></div>";
    let mut harness = Harness::wrap(App::new((), html).page(640, 400).expect("valid selectors"));
    harness.pump();
    for (id, expected) in [
        ("v1", [164.0, 39.0, 33.0, 50.0]),
        ("c1", [182.0, 42.0, 12.0, 20.0]),
        ("c2", [167.0, 42.0, 12.0, 20.0]),
    ] {
        assert_eq!(rect(&harness, id), expected, "{id}");
    }
}

#[test]
fn atomic_inlines_centre_on_the_vertical_baseline_like_chrome() {
    let html = "<!doctype html><style>html,body{margin:0;background:#fff;font:16px Arial}\
                .v{float:left;height:70px;margin:4px 8px 4px 4px;writing-mode:vertical-rl}\
                .b{display:inline-block;width:10px;height:10px}\
                </style><div class=v id=d1>H<span class=b id=b1></span>H</div>\
                <div class=v id=d4>H<span id=b4 style=\"display:inline-block;width:20px;height:6px\">\
                </span>H</div><div class=v style=\"text-orientation:upright\">H\
                <span class=b id=b5></span>H</div><div class=v style=\"text-orientation:sideways\">\
                H<span id=b6 style=\"display:inline-block;width:4px;height:10px\"></span>H</div>\
                <div class=v id=d7>H<span class=b id=b7 style=\"vertical-align:middle\"></span>H</div>\
                <div class=v id=d8>H<span class=b id=b8 style=\"vertical-align:text-top\"></span>H</div>";
    let mut harness = Harness::wrap(App::new((), html).page(640, 400).expect("valid selectors"));
    harness.pump();
    for (id, expected) in [
        ("d1", [4.0, 18.0]),
        ("b1", [8.0, 10.0]),
        ("d4", [34.0, 20.0]),
        ("b4", [34.0, 20.0]),
        ("b5", [70.0, 10.0]),
        ("b6", [100.0, 4.0]),
        ("d7", [126.0, 18.15625]),
        ("b7", [134.15625, 10.0]),
        ("d8", [156.15625, 18.0]),
        ("b8", [164.15625, 10.0]),
    ] {
        let [x, _, width, _] = rect(&harness, id);
        assert_eq!([x, width], expected, "{id}");
    }
}

#[test]
fn vertical_spans_report_rotated_rects_and_hits_like_chrome() {
    let html = "<html><head><style>html,body{margin:0;font:16px/20px Arial}\
                div{float:left;margin:10px;height:120px;border:2px solid #999;padding:3px 5px}\
                </style></head><body>\
                <div id=\"d1\" style=\"writing-mode:vertical-rl\">aaa <span id=\"s1\">bbb</span> \
                ccc ddd eee <span id=\"s2\">fff ggg</span> hhh</div>\
                <div id=\"d2\" style=\"writing-mode:vertical-lr\">aaa <span id=\"s3\">bbb</span> \
                ccc ddd eee <span id=\"s4\">fff ggg</span> hhh</div><pre></pre></body></html>";
    let page = App::new((), html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let doc = harness.base();
    let id_of = |id: &str| {
        doc.query_selector_all(&format!("#{id}"))
            .expect("selector is valid")[0]
    };
    for (span, root, chrome, point, offset) in [
        ("s1", "d1", [39.0, 62.141, 17.0, 26.703], (47.5, 70.152), 5),
        ("s2", "d1", [19.0, 62.141, 17.0, 43.906], (27.5, 75.313), 23),
        ("s3", "d2", [93.0, 62.141, 17.0, 26.703], (101.5, 70.152), 5),
        (
            "s4",
            "d2",
            [113.0, 62.141, 17.0, 43.906],
            (121.5, 75.313),
            23,
        ),
    ] {
        let ours = doc
            .get_client_bounding_rect(id_of(span))
            .expect("span has a rect");
        let [left, top, width, height] = chrome;
        assert!(
            (ours.y - top).abs() < 0.01 && (ours.height - height).abs() < 0.01,
            "{span} inline extent {} {}",
            ours.y,
            ours.height
        );
        assert!(
            ours.x <= left && ours.x + ours.width >= left + width && ours.width <= 20.0,
            "{span} block extent {} {}",
            ours.x,
            ours.width
        );
        let (x, y) = point;
        assert_eq!(
            doc.hit(x, y).map(|hit| hit.node_id),
            Some(id_of(span)),
            "{span}"
        );
        assert_eq!(
            doc.find_text_position(x, y),
            Some((id_of(root), offset)),
            "{span}"
        );
    }
}

#[test]
fn absolute_boxes_keep_their_static_line_in_vertical_text_like_chrome() {
    let rl = "writing-mode:vertical-rl";
    let lr = "writing-mode:vertical-lr";
    let rows: String = [
        (
            "d1",
            rl,
            "<i id=\"a1\"></i> ccc ddd eee fff<i id=\"a2\"></i> ggg",
        ),
        (
            "d2",
            lr,
            "<i id=\"a3\"></i> ccc ddd eee fff<i id=\"a4\"></i> ggg",
        ),
        ("d3", lr, "<b id=\"a5\"></b> ccc ddd eee fff ggg"),
        ("d4", rl, "<b id=\"a6\"></b> ccc ddd eee fff ggg"),
        ("d5", lr, " ccc<b id=\"a7\"></b> ddd eee"),
        (
            "d6",
            "writing-mode:vertical-lr;align-content:center;width:80px",
            "<i id=\"a8\"></i> ccc ddd eee fff<i id=\"a9\"></i> ggg",
        ),
        (
            "d7",
            "writing-mode:vertical-lr;align-content:end;width:80px",
            " ccc ddd<b id=\"a10\"></b> eee",
        ),
    ]
    .iter()
    .map(|(id, style, tail)| format!("<div id=\"{id}\" style=\"{style}\">aaa bbb{tail}</div>"))
    .collect();
    let html = format!(
        "<html><head><style>html,body{{margin:0;font:16px/20px Arial}}\
         div{{float:left;margin:10px;height:120px;border:2px solid #999;padding:3px 5px;\
         position:relative}}i{{position:absolute;width:6px;height:6px}}\
         b{{position:absolute;display:block;width:6px;height:6px}}</style></head><body>{rows}\
         <pre></pre></body></html>"
    );
    let page = App::new((), &html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    for (id, chrome) in [
        ("a1", [51.0, 88.844]),
        ("a2", [31.0, 74.906]),
        ("a3", [91.0, 88.844]),
        ("a4", [111.0, 74.906]),
        ("a5", [185.0, 31.0]),
        ("a6", [253.0, 31.0]),
        ("a7", [333.0, 31.0]),
        ("a8", [407.0, 88.844]),
        ("a9", [427.0, 74.906]),
        ("a10", [561.0, 31.0]),
    ] {
        let [left, top, ..] = rect(&harness, id);
        assert!(
            (left - chrome[0]).abs() < 0.01 && (top - chrome[1]).abs() < 0.01,
            "{id} at {left},{top}, Chrome {chrome:?}"
        );
    }
}

#[test]
fn text_clipped_backgrounds_follow_vertical_text_like_chrome() {
    let html = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}\
                div{float:left;width:46px;height:36px;margin:4px 8px 4px 4px;background:#000;\
                background-clip:text;color:transparent}</style>\
                <div style=\"writing-mode:vertical-rl\">HHH II</div>\
                <div style=\"writing-mode:vertical-lr\">HHH II</div>";
    let mut harness = open(App::new((), html));
    let buffer = render(&mut harness);
    let chrome = [
        [[21, 31, 5, 10], [37, 47, 5, 33]],
        [[65, 75, 5, 33], [81, 91, 5, 10]],
    ];
    for ((left, right), expected) in [(0, 58), (58, 120)].into_iter().zip(chrome) {
        let columns = ink_columns(&buffer, left, right);
        assert_eq!(columns.len(), 2, "{left}..{right}: {columns:?}");
        for (column, chrome_column) in columns.iter().zip(expected) {
            assert!(
                column
                    .iter()
                    .zip(chrome_column)
                    .all(|(ours, theirs)| ours.abs_diff(theirs) <= 1),
                "ink {column:?}, Chrome {chrome_column:?}"
            );
        }
    }
}

#[test]
fn text_clipped_backgrounds_line_up_with_the_text_at_fractional_offsets() {
    let page = |paint: &str| {
        format!(
            "<!doctype html><style>html,body{{margin:0;background:#fff;font:14px Arial}}\
             div{{float:left;width:46px;height:36px;margin:4.4px 3.3px 0 0.6px;\
             padding:0.3px 0 0 0.45px;{paint}}}</style><div>HHH II</div>\
             <div style=\"writing-mode:vertical-rl\">HHH II</div>\
             <div style=\"writing-mode:vertical-lr\">HHH II</div>"
        )
    };
    let mut text = open(App::new((), &page("color:#000")));
    let mut clipped = open(App::new(
        (),
        &page("background:#000;background-clip:text;color:transparent"),
    ));
    let text = render(&mut text);
    let clipped = render(&mut clipped);
    let inked = text
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|rgba| rgba[0] < 128)
        .count();
    assert!(inked > 200, "only {inked} dark pixels");
    let worst = text
        .iter()
        .zip(&clipped)
        .map(|(ours, theirs)| ours.abs_diff(*theirs))
        .max()
        .unwrap_or(0);
    assert_eq!(worst, 0, "worst channel difference");
}

fn first_ink_row(
    buffer: &[u8],
    columns: std::ops::Range<usize>,
    rows: std::ops::Range<usize>,
) -> Option<usize> {
    rows.into_iter().find(|&row| {
        columns.clone().any(|column| {
            let [red, green, blue, _] = pixel(buffer, column, row);
            u16::from(red) + u16::from(green) + u16::from(blue) < 3 * 236
        })
    })
}

#[test]
fn upright_cjk_glyphs_start_where_chrome_puts_them() {
    let cases = [
        ("Microsoft YaHei", "漢", "4px", 4),
        ("Microsoft YaHei", "一", "4px", 11),
        ("Yu Gothic", "あ", "4px", 6),
        ("MS Gothic", "口", "4px", 7),
        ("SimSun", "あ", "4px", 5),
        ("Malgun Gothic", "あ", "40px", 42),
        ("Malgun Gothic", "漢", "40.75px", 43),
    ];
    let boxes: String = cases
        .iter()
        .enumerate()
        .map(|(index, (font, glyph, top, _))| {
            format!(
                "<div style=\"left:{}px;top:{top};font-family:'{font}'\">{glyph}</div>",
                2 + 17 * index
            )
        })
        .collect();
    for mode in ["vertical-rl", "vertical-lr"] {
        let html = format!(
            "<!doctype html><meta charset=utf-8><style>html,body{{margin:0;background:#fff;\
             font-size:16px}}div{{position:absolute;writing-mode:{mode};height:30px;\
             line-height:16px}}</style>{boxes}"
        );
        let mut harness = open(App::new((), &html));
        let buffer = render(&mut harness);
        for (index, (font, glyph, top, chrome)) in cases.iter().enumerate() {
            let left = 2 + 17 * index;
            assert_eq!(
                first_ink_row(&buffer, left..left + 16, 0..80),
                Some(*chrome),
                "{mode}: {font} {glyph} at top {top}"
            );
        }
    }
}
