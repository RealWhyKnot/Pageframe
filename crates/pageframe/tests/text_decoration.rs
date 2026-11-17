mod support;

use pageframe::App;
use support::{open, pixel, render};

const BASE: &str =
    "html,body{margin:0;background:white;font:10px/20px monospace;color:transparent}";
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
fn an_underline_starts_half_its_thickness_below_the_baseline() {
    for (size, gap) in [(10, 1), (20, 1), (32, 2)] {
        let buffer = draw(&format!(
            "<div style=\"font-size:{size}px;line-height:50px;text-decoration:underline red\">aaa             <span style=\"display:inline-block;width:6px;height:5px;background:blue\"></span></div>"
        ));
        let block: Vec<usize> = (0..120)
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
        assert_eq!(first_red, last_blue.map(|y| y + 1 + gap), "{size}px");
    }
}

fn line_gaps(style: &str) -> usize {
    let columns = red_columns(&draw(&format!(
        "<div style=\"font-size:32px;line-height:50px;{style}\">aga</div>"
    )));
    assert!(columns.len() > 20, "{style} paints a line");
    columns
        .windows(2)
        .filter(|pair| pair[1] != pair[0] + 1)
        .count()
}

#[test]
fn an_underline_breaks_around_a_descender() {
    assert_eq!(line_gaps("text-decoration:underline red"), 1);
}

#[test]
fn an_underline_with_skip_ink_none_is_unbroken() {
    assert_eq!(
        line_gaps("text-decoration:underline red;text-decoration-skip-ink:none"),
        0
    );
}

#[test]
fn a_line_through_is_never_broken() {
    assert_eq!(line_gaps("text-decoration:line-through red"), 0);
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
fn a_parent_underline_skips_a_child_span_padding() {
    let plain = red_columns(&draw(
        "<div><u style=\"text-decoration-color:red\"><span>aa</span></u></div>",
    ));
    let padded = red_columns(&draw(
        "<div><u style=\"text-decoration-color:red\">\
         <span style=\"padding:0 10px\">aa</span></u></div>",
    ));
    assert_eq!(padded.first(), Some(&10));
    assert_eq!(padded.len(), plain.len());
}

fn wavy_ink(thickness: u32) -> Vec<Vec<u8>> {
    let buffer = draw(&format!(
        "<div style=\"font:16px Arial;white-space:pre;text-decoration:underline wavy black \
         {thickness}px;text-decoration-skip-ink:none\">|                    |</div>"
    ));
    (0..support::WIDTH)
        .map(|x| {
            (0..40)
                .map(|y| 255 - pixel(&buffer, usize::try_from(x).expect("fits"), y)[0])
                .collect()
        })
        .collect()
}

#[test]
fn wavy_underlines_repeat_and_sit_where_chrome_draws_them() {
    for (thickness, period, rows) in [(1, 7, 15..=19), (3, 15, 16..=24)] {
        let ink = wavy_ink(thickness);
        for x in 20..40 {
            let drift = ink[x]
                .iter()
                .zip(&ink[x + period])
                .map(|(here, there)| here.abs_diff(*there))
                .max();
            assert!(
                drift <= Some(2),
                "{thickness}px wave doesn't repeat every {period}px at column {x}"
            );
        }
        let inked: Vec<usize> = (0..40)
            .filter(|&y| (20..60).any(|x| ink[x][y] > 12))
            .collect();
        assert_eq!(
            (inked.first(), inked.last()),
            (Some(rows.start()), Some(rows.end())),
            "{thickness}px wave rows"
        );
    }
}

#[test]
fn one_pixel_wavy_underlines_are_hairlines_like_chrome() {
    let ink = wavy_ink(1);
    let total: u32 = (20..62)
        .flat_map(|x| ink[x].iter().map(|value| u32::from(*value)))
        .sum();
    let per_column = f64::from(total) / 255.0 / 42.0;
    assert!(
        (0.85..1.15).contains(&per_column),
        "a 1px wave carries {per_column} px of ink per column"
    );
}
