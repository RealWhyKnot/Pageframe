#[path = "tables_cases/basics.rs"]
mod basics;
#[path = "tables_cases/captions.rs"]
mod captions;
#[path = "tables_cases/columns.rs"]
mod columns;
#[path = "tables_cases/heights.rs"]
mod heights;
#[path = "tables_cases/nested.rs"]
mod nested;
#[path = "tables_cases/rows.rs"]
mod rows;
#[path = "tables_cases/spacing.rs"]
mod spacing;
#[path = "tables_cases/spans.rs"]
mod spans;
#[path = "tables_cases/widths.rs"]
mod widths;

mod support;

use pageframe::App;
use support::{open, pixel, render};

const STYLE: &str = "html,body{margin:0;background:white}\
table{border-collapse:collapse}\
td{border:2px solid black;padding:5px 10px;background:#0f0}\
.thin td{border-width:1px}\
i{display:inline-block;height:10px}\
.bare td{padding:0;font-size:0}\
caption{background:#00f;font-size:0}";

const BLACK: [u8; 4] = [0, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

fn draw(body: &str) -> Vec<u8> {
    let html = format!("<html><head><style>{STYLE}</style></head><body>{body}</body></html>");
    let mut harness = open(App::new((), &html));
    harness.pump();
    render(&mut harness)
}

fn row(buffer: &[u8], xs: &[usize], y: usize) -> Vec<[u8; 4]> {
    xs.iter().map(|&x| pixel(buffer, x, y)).collect()
}

fn column(buffer: &[u8], x: usize, ys: &[usize]) -> Vec<[u8; 4]> {
    ys.iter().map(|&y| pixel(buffer, x, y)).collect()
}

fn check_row_heights(body: &str, expected: &[(&str, f32, f32)]) {
    let html = format!(
        "<html><head><style>{STYLE}table{{height:122px}}\
         td{{padding:0;width:10px;font-size:0}}\
         i{{width:6px;vertical-align:top}}\
         .z{{border-collapse:separate;border-spacing:0;height:120px}}\
         .z td{{border:0}}</style></head><body>{body}</body></html>"
    );
    let mut harness = open(App::new((), &html));
    harness.pump();
    for &(id, top, height) in expected {
        let doc = harness.base_mut();
        let node_id = doc
            .query_selector_all(&format!("#{id}"))
            .expect("row selector is valid")[0];
        let node = doc.get_node(node_id).expect("row exists");
        let actual_top = node.absolute_position(0.0, 0.0).y;
        let actual_height = node.final_layout().size.height;
        drop(doc);
        assert!(
            (actual_top - top).abs() < 0.95,
            "{id}: top {actual_top}, expected {top}"
        );
        assert!(
            (actual_height - height).abs() < 0.95,
            "{id}: height {actual_height}, expected {height}"
        );
    }
}

fn check_table_boxes(body: &str, expected: &[(&str, [f32; 4])]) {
    let html = format!(
        "<html><head><style>{STYLE}td{{padding:0;width:10px;font-size:0}}\
         i{{width:6px;vertical-align:top}}</style></head><body>{body}</body></html>"
    );
    let mut harness = open(App::new((), &html));
    harness.pump();
    for &(id, expected) in expected {
        let doc = harness.base_mut();
        let node_id = doc
            .query_selector_all(&format!("#{id}"))
            .expect("box selector is valid")[0];
        let node = doc.get_node(node_id).expect("box exists");
        let position = node.absolute_position(0.0, 0.0);
        let size = node.final_layout().size;
        let actual = [position.x, position.y, size.width, size.height];
        drop(doc);
        assert!(
            actual
                .iter()
                .zip(expected)
                .all(|(actual, expected)| (actual - expected).abs() < 0.95),
            "{id}: {actual:?}, expected {expected:?}"
        );
    }
}

const SPAN_STYLE: &str = "<style>table{border-collapse:separate;border-spacing:2px}td{width:auto}b{display:inline-block;width:6px;height:10px;background:yellow}col.x{visibility:collapse}div{position:absolute;left:0;top:0}</style>";

fn seam_table(above: &str, hidden: &str, below: &str) -> Vec<u8> {
    draw(&format!(
        r#"<table class="bare"><col><col style="visibility:collapse"><tr><td style="{above}"><i style="width:6px"></i></td><td><i style="width:6px"></i></td></tr><tr style="visibility:collapse"><td style="border-color:red;{hidden}"><i style="width:6px"></i></td><td style="border-color:blue"><i style="width:6px"></i></td></tr><tr><td style="{below}"><i style="width:6px"></i></td><td><i style="width:6px"></i></td></tr></table>"#
    ))
}

const CAPTION_STYLE: &str = "<style>table{border-collapse:separate;border-spacing:2px;border:2px solid black}td{width:auto;border:0;padding:0;font-size:0}caption{font-size:0}div{position:absolute;left:0;top:0}b{display:inline-block;width:40px;height:10px}</style>";

fn check_caption_boxes(table: &str, expected: &[(&str, [f32; 4])]) {
    check_table_boxes(&format!("{CAPTION_STYLE}<div>{table}</div>"), expected);
}

const CAPTION_PAINT_STYLE: &str = "<style>table{border-collapse:separate;border-spacing:2px;border:2px solid black}td{border:0;padding:0;font-size:0}b{display:inline-block;width:40px;height:10px}</style>";
