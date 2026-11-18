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
