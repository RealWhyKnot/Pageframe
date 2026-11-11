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
    let one_sided = painted(&boxed_span("padding-left:3px", "aa"), 20);
    let one_sided_spaced = painted(&boxed_span("padding-left:3px", "aa "), 20);
    assert_eq!(one_sided_spaced.len(), one_sided.len());
}

#[test]
fn a_line_wraps_after_a_span_that_ends_in_a_space() {
    let tight = painted(&boxed_span("padding:0 3px", "aa"), 20);
    let buffer = draw(
        "<div style=\"line-height:40px;width:30px\">\
         <span style=\"background:#0f0;color:transparent;padding:0 3px\">aa </span>aaa</div>",
    );
    let row = painted(&buffer, 20);
    assert_eq!(row.len(), tight.len());
    assert!(row.iter().all(|(_, colour)| *colour == LIME));
    assert!((40..80).any(|y| !painted(&buffer, y).is_empty()));
}

fn wrap_case(span_style: &str, inner: &str, after: &str) -> usize {
    let buffer = draw(&format!(
        "<div style=\"line-height:40px;width:30px\">\
         <span style=\"background:#0f0;color:transparent;padding:0 3px;{span_style}\">{inner}</span>\
         {after}</div>"
    ));
    painted(&buffer, 20).len()
}

#[test]
fn a_span_break_follows_the_span_text_and_wrap_mode() {
    let hung = wrap_case("", "aa ", "aaa");
    assert!(wrap_case("white-space:nowrap", "aa ", "aaa") > hung);
    assert_eq!(wrap_case("", "aa <!--c-->", "aaa"), hung);
    assert_eq!(
        wrap_case("", "aa <i style=\"display:none\">q</i>", "aaa"),
        hung
    );
    assert_eq!(
        wrap_case("", "aa ", "<span style=\"white-space:nowrap\">aaa</span>"),
        hung
    );
    for after in ["!aa", "/aa", "}aa", ",aa"] {
        assert_eq!(wrap_case("", "aa ", after), hung, "{after}");
    }
}

#[test]
fn a_cloned_span_keeps_its_padding_on_every_line() {
    let tight = painted(&boxed_span("padding:0 3px", "aa"), 20).len();
    let sliced = boxed_span("padding:0 3px", "aa aa");
    assert_eq!(painted(&sliced, 20).len() + 3, tight);
    assert_eq!(painted(&sliced, 60).len() + 3, tight);
    let cloned = boxed_span("padding:0 3px;box-decoration-break:clone", "aa aa");
    assert_eq!(painted(&cloned, 20).len(), tight);
    assert_eq!(painted(&cloned, 60), painted(&cloned, 20));
}

#[test]
fn a_cloned_span_end_overflows_and_its_start_takes_room() {
    let plain = painted(&boxed_span("", "aa a"), 20).len();
    let ended = boxed_span("padding-right:8px;box-decoration-break:clone", "aa a");
    assert_eq!(painted(&ended, 20).len(), plain + 8);
    assert_eq!(painted(&ended, 60).len(), 0);
    let word = painted(&boxed_span("", "aa"), 20).len();
    let sliced = boxed_span("padding-left:10px", "aa aa a");
    assert_eq!(painted(&sliced, 60).len(), plain);
    let started = boxed_span("padding-left:10px;box-decoration-break:clone", "aa aa a");
    assert_eq!(painted(&started, 60).len(), word + 10);
}

#[test]
fn a_cloned_span_closes_around_a_cloned_child_on_every_line() {
    let buffer = boxed_span(
        "padding-right:4px;box-decoration-break:clone",
        "<span style=\"background:#00f;padding-right:4px;box-decoration-break:clone\">aaa aaa</span>",
    );
    for y in [20, 60] {
        let row = painted(&buffer, y);
        let lime = row.iter().filter(|(_, colour)| *colour == LIME).count();
        assert_eq!(lime, 4);
        assert_eq!(row.last().map(|(_, colour)| *colour), Some(LIME));
    }
}

#[test]
fn a_span_break_follows_the_wrap_mode_of_its_last_text() {
    let hung = wrap_case("", "aa ", "aaa");
    let kept = wrap_case("white-space:nowrap", "aa ", "aaa");
    assert!(kept > hung);
    assert_eq!(
        wrap_case("", "<i style=\"white-space:nowrap\">aa </i>", "aaa"),
        kept
    );
    assert_eq!(
        wrap_case("", "<i style=\"white-space:pre\">aa </i>", "aaa"),
        kept
    );
    assert_eq!(
        wrap_case(
            "white-space:nowrap",
            "<i style=\"white-space:normal\">aa </i>",
            "aaa"
        ),
        hung
    );
    assert_eq!(
        wrap_case("", "aa<i style=\"display:contents\"> </i>", "aaa"),
        hung
    );
    assert!(wrap_case("", "<i style=\"white-space:pre-wrap\">aa </i>", "aaa") < kept);
}

#[test]
fn a_cloned_span_end_keeps_the_breaks_of_an_indented_line() {
    let lines = |style: &str| {
        let buffer = draw(&format!(
            "<div style=\"line-height:40px;width:30px;text-indent:6px\">\
             <span style=\"background:#0f0;color:transparent;{style}\">aa a aa</span></div>"
        ));
        (painted(&buffer, 20).len(), painted(&buffer, 60).len())
    };
    let (first, second) = lines("");
    assert_eq!(
        lines("padding-right:8px;box-decoration-break:clone"),
        (first + 8, second + 8)
    );
}

#[test]
fn a_cloned_span_in_right_to_left_text_starts_each_line_on_the_right() {
    let row = |style: &str, y: usize| {
        let buffer = draw(&format!(
            "<div dir=\"rtl\" style=\"line-height:40px;width:30px\">\
             <span style=\"background:#0f0;color:transparent;{style}\">aaa<br>aaa</span></div>"
        ));
        let row = painted(&buffer, y);
        (row.first().map(|(x, _)| *x), row.len())
    };
    let (_, word) = row("", 20);
    for y in [20, 60] {
        let start = "padding-right:10px;box-decoration-break:clone";
        assert_eq!(row(start, y), (Some(30 - word - 10), word + 10));
        let end = "padding-left:4px;box-decoration-break:clone";
        assert_eq!(row(end, y), (Some(30 - word - 4), word + 4));
    }
}

fn underline_row(inner: &str) -> Vec<usize> {
    let buffer = draw(&format!(
        "<div style=\"line-height:40px;width:30px;color:transparent;\
         text-decoration:underline black\">{inner}</div>"
    ));
    (0..40)
        .map(|y| painted(&buffer, y))
        .find(|row| !row.is_empty())
        .expect("the underline is painted")
        .into_iter()
        .map(|(x, _)| x)
        .collect()
}

#[test]
fn an_underline_stops_before_the_space_a_line_wraps_at() {
    let word = underline_row("aaa");
    assert_eq!(underline_row("aaa aaa"), word);
    assert!(
        underline_row("<span style=\"white-space:pre-wrap\">aaa aaa</span>").len() > word.len()
    );
}

#[test]
fn an_underline_skips_span_padding_and_inline_blocks() {
    let padded = underline_row("a<span style=\"padding:0 6px\">a</span>a");
    assert!(padded.contains(&2) && padded.contains(&14) && padded.contains(&26));
    assert!(!padded.contains(&8) && !padded.contains(&20));
    let own = underline_row("<span style=\"padding:0 6px;margin:0 3px\">aa</span>");
    assert_eq!(own.first(), Some(&9));
    assert_eq!(own.len(), 11);
    let boxed = underline_row("a<i style=\"display:inline-block;width:10px;height:5px\"></i>a");
    assert!(boxed.contains(&2) && boxed.contains(&18));
    assert!(!boxed.contains(&10));
}

#[test]
fn a_line_breaks_after_a_space_before_punctuation() {
    let word = painted(&boxed_span("", "aaa"), 20).len();
    for after in [",aa", "!aa", "/aa", ")aa", "}aa", ".aa"] {
        let buffer = boxed_span("", &format!("aaa {after}"));
        assert_eq!(painted(&buffer, 20).len(), word, "{after}");
        assert_eq!(painted(&buffer, 60).len(), word, "{after}");
    }
    let buffer = draw(
        "<div style=\"line-height:40px;width:30px;color:transparent\">aaa \n         <span style=\"background:#0f0;padding:0 1px\">,aa</span></div>",
    );
    assert_eq!(painted(&buffer, 20).len(), 0);
    assert_eq!(painted(&buffer, 60).len(), word + 2);
}
