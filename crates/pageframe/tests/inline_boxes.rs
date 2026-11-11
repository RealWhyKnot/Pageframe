mod support;

use pageframe::App;
use support::{open, pixel, render};

const BASE: &str = "html,body{margin:0;background:white;font:10px/10px monospace;color:black}";
const LIME: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

fn draw(body: &str) -> Vec<u8> {
    let html = format!("<html><head><style>{BASE}</style></head><body>{body}</body></html>");
    let mut harness = open(App::new((), &html));
    harness.pump();
    render(&mut harness)
}

fn boxed_span(style: &str, inner: &str) -> Vec<u8> {
    draw(&format!(
        "<div style=\"line-height:40px;width:30px\">\
         <span style=\"background:#0f0;color:transparent;{style}\">{inner}</span></div>"
    ))
}

fn painted(buffer: &[u8], y: usize) -> Vec<(usize, [u8; 4])> {
    (0..60)
        .map(|x| (x, pixel(buffer, x, y)))
        .filter(|(_, colour)| *colour != WHITE)
        .collect()
}

#[test]
fn span_padding_and_margin_take_room_on_the_line() {
    let plain = painted(&boxed_span("", "aa"), 20);
    let padded = painted(&boxed_span("padding:0 5px", "aa"), 20);
    let pushed = painted(&boxed_span("margin-left:7px", "aa"), 20);
    assert_eq!(padded.len(), plain.len() + 10);
    assert_eq!(pushed.len(), plain.len());
    assert_eq!(pushed.first().map(|(x, _)| *x), Some(7));
}

#[test]
fn a_span_border_is_painted_around_its_background() {
    let buffer = boxed_span("border:2px solid blue", "aa");
    let row = painted(&buffer, 20);
    let colours: Vec<[u8; 4]> = row.iter().map(|(_, colour)| *colour).collect();
    assert_eq!(colours[..3], [BLUE, BLUE, LIME]);
    assert_eq!(colours[colours.len() - 3..], [LIME, BLUE, BLUE]);
    let column: Vec<[u8; 4]> = (0..40)
        .map(|y| pixel(&buffer, 5, y))
        .filter(|colour| *colour != WHITE)
        .collect();
    assert_eq!(column[..3], [BLUE, BLUE, LIME]);
    assert_eq!(column[column.len() - 3..], [LIME, BLUE, BLUE]);
}

#[test]
fn span_padding_grows_the_background_without_moving_the_line() {
    let top = |buffer: &[u8]| (0..40).find(|y| pixel(buffer, 8, *y) == LIME);
    let plain = top(&boxed_span("", "aa"));
    let padded = top(&boxed_span("padding:5px", "aa"));
    assert!(plain.is_some());
    assert_eq!(padded.map(|y| y + 5), plain);
}

#[test]
fn a_wrapped_span_keeps_its_side_borders_at_its_two_ends() {
    let buffer = boxed_span("border:2px solid blue", "aa aa");
    let first = painted(&buffer, 20);
    let second = painted(&buffer, 60);
    assert_eq!(first.first().map(|(_, colour)| *colour), Some(BLUE));
    assert_eq!(first.last().map(|(_, colour)| *colour), Some(LIME));
    assert_eq!(second.first().map(|(_, colour)| *colour), Some(LIME));
    assert_eq!(second.last().map(|(_, colour)| *colour), Some(BLUE));
}

#[test]
fn a_span_radius_rounds_its_background() {
    let buffer = boxed_span("border-radius:6px;padding:0 6px", "aa");
    let row = painted(&buffer, 20);
    let top = (0..40).find(|y| pixel(&buffer, 12, *y) == LIME);
    assert!(row.len() > 12);
    assert_eq!(top.map(|y| pixel(&buffer, 0, y)), Some(WHITE));
}

#[test]
fn a_span_background_stops_before_the_space_a_line_wraps_at() {
    let wrapped = painted(&boxed_span("", "aaa aaa"), 20);
    let single = painted(&boxed_span("", "aaa"), 20);
    assert_eq!(wrapped.len(), single.len());
}

#[test]
fn a_span_holding_only_an_inline_block_paints_its_background() {
    let buffer = boxed_span(
        "",
        "<i style=\"display:inline-block;width:20px;height:2px\"></i>",
    );
    assert_eq!(painted(&buffer, 20).len(), 20);
}

fn narrow(inner: &str) -> Vec<u8> {
    draw(&format!(
        "<div style=\"line-height:40px;width:30px;color:transparent\">{inner}</div>"
    ))
}

#[test]
fn a_line_wraps_at_the_space_before_a_padded_span() {
    let buffer = narrow("aaa <span style=\"background:#0f0;padding:0 3px\">aaa</span>");
    let plain = painted(&boxed_span("", "aaa"), 20).len();
    assert_eq!(painted(&buffer, 20).len(), 0);
    assert_eq!(painted(&buffer, 60).len(), plain + 6);
}

#[test]
fn a_span_closes_on_the_line_its_text_ends_on() {
    let buffer =
        narrow("<span style=\"background:#0f0;border-right:3px solid blue\">aaa </span>aaa");
    assert_eq!(
        painted(&buffer, 20).last().map(|(_, colour)| *colour),
        Some(BLUE)
    );
    assert_eq!(painted(&buffer, 60).len(), 0);
}

#[test]
fn a_span_in_right_to_left_text_keeps_both_side_borders() {
    let buffer = draw(
        "<div dir=\"rtl\" style=\"line-height:40px;width:60px\"><span style=\"background:#0f0;\
         color:transparent;border-left:3px solid blue;border-right:2px solid blue\">aa</span></div>",
    );
    let colours: Vec<[u8; 4]> = painted(&buffer, 20)
        .iter()
        .map(|(_, colour)| *colour)
        .collect();
    assert_eq!(colours[..4], [BLUE, BLUE, BLUE, LIME]);
    assert_eq!(colours[colours.len() - 3..], [LIME, BLUE, BLUE]);
}

#[test]
fn a_span_outline_is_painted_outside_its_box() {
    let plain = painted(&boxed_span("margin-left:5px", "aa"), 20);
    let row = painted(
        &boxed_span("margin-left:5px;outline:2px solid blue", "aa"),
        20,
    );
    let colours: Vec<[u8; 4]> = row.iter().map(|(_, colour)| *colour).collect();
    assert_eq!(row.first().map(|(x, _)| *x), Some(3));
    assert_eq!(row.len(), plain.len() + 4);
    assert_eq!(colours[..3], [BLUE, BLUE, LIME]);
    assert_eq!(colours[colours.len() - 3..], [LIME, BLUE, BLUE]);
}

#[test]
fn a_span_box_shadow_is_painted_beside_its_box() {
    let plain = painted(&boxed_span("", "aa"), 20);
    let row = painted(&boxed_span("box-shadow:3px 0 0 blue", "aa"), 20);
    let colours: Vec<[u8; 4]> = row.iter().map(|(_, colour)| *colour).collect();
    assert_eq!(row.len(), plain.len() + 3);
    assert_eq!(colours[colours.len() - 4..], [LIME, BLUE, BLUE, BLUE]);
}

#[test]
fn a_relative_span_moves_its_box_by_its_offsets() {
    let top = |buffer: &[u8]| (0..40).find(|y| pixel(buffer, 8, *y) == LIME);
    let plain = boxed_span("", "aa");
    let moved = boxed_span("position:relative;top:6px;left:4px", "aa");
    assert_eq!(top(&moved), top(&plain).map(|y| y + 6));
    let row = top(&moved).expect("the span is painted");
    assert_eq!(painted(&moved, row).first().map(|(x, _)| *x), Some(4));
    assert_eq!(painted(&moved, row).len(), painted(&plain, row - 6).len());
}

#[test]
fn a_relative_span_moves_its_text_and_underline() {
    let buffer = draw(
        "<div style=\"line-height:40px\">\
         <span style=\"position:relative;left:20px;text-decoration:underline\">a</span></div>",
    );
    let columns: Vec<usize> = (0..60)
        .filter(|x| (0..40).any(|y| pixel(&buffer, *x, y) != WHITE))
        .collect();
    assert_eq!(columns.first(), Some(&20));
    assert!(
        columns.len() >= 6,
        "the glyph and its underline are painted"
    );
}

#[test]
fn a_faded_span_fades_its_box_and_text_as_one() {
    let buffer = draw(
        "<div style=\"line-height:40px\">\
         <span style=\"opacity:.5;background:#000;padding:0 3px\">aa</span></div>",
    );
    let row = painted(&buffer, 20);
    assert!(row.len() >= 16, "the span is painted");
    for (x, colour) in row {
        assert!(
            (126..=129).contains(&colour[0]) && colour[0] == colour[1] && colour[1] == colour[2],
            "{colour:?} at {x}"
        );
    }
}

#[test]
fn a_faded_span_without_a_box_fades_its_text() {
    let darkest = |style: &str| {
        let buffer = draw(&format!(
            "<div style=\"font-size:30px;line-height:40px\"><span style=\"{style}\">ll</span></div>"
        ));
        (0..40)
            .flat_map(|y| (0..60).map(move |x| (x, y)))
            .map(|(x, y)| pixel(&buffer, x, y)[0])
            .min()
    };
    assert_eq!(darkest(""), Some(0));
    assert!(darkest("opacity:.5").is_some_and(|value| (126..=129).contains(&value)));
}

#[test]
fn an_underline_fades_under_a_faded_child_only() {
    let buffer = draw(
        "<div style=\"line-height:40px;text-decoration:underline\">\
         aa<span style=\"opacity:.5\">aa</span>aa</div>",
    );
    let row = (0..40)
        .rev()
        .find(|y| painted(&buffer, *y).len() >= 30)
        .expect("the underline is painted");
    let line = painted(&buffer, row);
    let right = line.last().map_or(0, |(x, _)| *x);
    assert_eq!(pixel(&buffer, 2, row), [0, 0, 0, 255]);
    assert_eq!(pixel(&buffer, right - 2, row), [0, 0, 0, 255]);
    assert!((126..=129).contains(&pixel(&buffer, right / 2, row)[0]));
}

#[test]
fn percent_span_padding_resolves_against_the_block_width() {
    let row = |style: &str| {
        let buffer = draw(&format!(
            "<div style=\"line-height:40px;width:50px\">\
             <span style=\"background:#0f0;color:transparent;{style}\">aa</span></div>"
        ));
        painted(&buffer, 20).len()
    };
    assert_eq!(row("padding:0 10%"), row("padding:0 5px"));
    assert_eq!(row("padding-right:10%"), row("padding-right:5px"));
    let left = |style: &str| {
        let buffer = draw(&format!(
            "<div style=\"line-height:40px;width:50px\">\
             <span style=\"background:#0f0;color:transparent;{style}\">aa</span></div>"
        ));
        painted(&buffer, 20).first().map(|(x, _)| *x)
    };
    assert_eq!(left("margin-left:10%"), Some(5));
}

#[test]
fn a_space_before_a_span_close_hangs_at_the_line_end() {
    let tight = painted(&boxed_span("padding:0 3px", "aa"), 20);
    let spaced = painted(&boxed_span("padding:0 3px", "aa "), 20);
    assert_eq!(spaced.len(), tight.len());
    let inner = painted(&boxed_span("padding:0 3px", "aa <i>a</i>"), 20);
    assert!(inner.len() > tight.len() + 6);
}
