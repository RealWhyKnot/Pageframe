mod support;

use pageframe::App;
use support::{open, pixel, render};

const BASE: &str = "html,body{margin:0;background:white;font:10px/20px monospace;color:transparent}";
const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

fn draw(body: &str) -> Vec<u8> {
    let html = format!("<html><head><style>{BASE}</style></head><body>{body}</body></html>");
    let mut harness = open(App::new((), &html));
    harness.pump();
    render(&mut harness)
}

fn column(buffer: &[u8], x: usize) -> Vec<(usize, [u8; 4])> {
    (0..60)
        .map(|y| (y, pixel(buffer, x, y)))
        .filter(|(_, colour)| *colour != WHITE)
        .collect()
}

fn red_columns(buffer: &[u8]) -> Vec<usize> {
    let row = (0..60).find(|y| (0..80).any(|x| pixel(buffer, x, *y) == RED));
    row.map(|y| (0..80).filter(|x| pixel(buffer, *x, y) == RED).collect())
        .unwrap_or_default()
}

#[test]
fn decoration_lines_fill_whole_pixel_rows() {
    for (size, rows) in [(10, 1), (16, 1), (20, 2), (32, 3)] {
        for line in ["underline", "line-through", "overline"] {
            let buffer = draw(&format!(
                "<div style=\"margin-top:0.4px;font-size:{size}px;line-height:50px;\
                 text-decoration:{line} red\">aaa</div>"
            ));
            let colours: Vec<[u8; 4]> = column(&buffer, 4).iter().map(|(_, c)| *c).collect();
            assert_eq!(colours, vec![RED; rows], "{line} at {size}px");
        }
    }
}

#[test]
fn an_underline_starts_one_row_below_the_baseline() {
    let buffer = draw(
        "<div style=\"text-decoration:underline red\">aaa\
         <span style=\"display:inline-block;width:6px;height:5px;background:blue\"></span></div>",
    );
    let block: Vec<usize> = (0..80)
        .filter(|x| column(&buffer, *x).iter().any(|(_, c)| *c == BLUE))
        .collect();
    let x = *block.get(2).expect("the inline-block is painted");
    let last_blue = column(&buffer, x)
        .iter()
        .filter(|(_, c)| *c == BLUE)
        .map(|(y, _)| *y)
        .max();
    let first_red = column(&buffer, 4)
        .iter()
        .find(|(_, c)| *c == RED)
        .map(|(y, _)| *y);
    assert_eq!(first_red, last_blue.map(|y| y + 2));
}

#[test]
fn an_underline_on_a_padded_span_covers_its_text_only() {
    let plain = red_columns(&draw(
        "<div><span style=\"text-decoration:underline red\">aa</span></div>",
    ));
    let padded = red_columns(&draw(
        "<div><span style=\"padding:0 10px;text-decoration:underline red\">aa</span></div>",
    ));
    assert_eq!(plain.first(), Some(&0));
    assert_eq!(padded.first(), Some(&10));
    assert_eq!(padded.len(), plain.len());
}

#[test]
fn a_parent_underline_runs_across_a_child_span_padding() {
    let plain = red_columns(&draw(
        "<div><u style=\"text-decoration-color:red\"><span>aa</span></u></div>",
    ));
    let padded = red_columns(&draw(
        "<div><u style=\"text-decoration-color:red\">\
         <span style=\"padding:0 10px\">aa</span></u></div>",
    ));
    assert_eq!(padded.first(), Some(&0));
    assert_eq!(padded.len(), plain.len() + 20);
}
