mod support;

use pageframe::App;
use support::{HEIGHT, WIDTH, open, pixel, render};

const BASE: &str = "html,body{margin:0;background:white;font:10px/10px monospace;color:black}\
div{margin:0}";

const LEGACY: &str = "display:-webkit-box;-webkit-box-orient:vertical;-webkit-line-clamp:2";

const WORDS: &str = "aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo \
pppp qqqq rrrr ssss tttt";

fn draw(style: &str, body: &str) -> Vec<u8> {
    let html = format!("<html><head><style>{BASE}{style}</style></head><body>{body}</body></html>");
    let mut harness = open(App::new((), &html));
    harness.pump();
    render(&mut harness)
}

fn has_ink(buffer: &[u8], rows: std::ops::Range<usize>) -> bool {
    let width = usize::try_from(WIDTH).expect("test width fits in usize");
    rows.into_iter().any(|y| {
        (0..width).any(|x| {
            let [red, green, blue, _] = pixel(buffer, x, y);
            red < 128 && green < 128 && blue < 128
        })
    })
}

fn first_ink_column(buffer: &[u8], rows: std::ops::Range<usize>) -> Option<usize> {
    let width = usize::try_from(WIDTH).expect("test width fits in usize");
    (0..width).find(|&x| {
        rows.clone().any(|y| {
            let [red, green, blue, _] = pixel(buffer, x, y);
            red < 128 && green < 128 && blue < 128
        })
    })
}

fn height() -> usize {
    usize::try_from(HEIGHT).expect("test height fits in usize")
}

fn clamped(rule: &str) -> Vec<u8> {
    draw(
        &format!(".c{{{rule}}}"),
        &format!("<div class=\"c\">{WORDS}</div>"),
    )
}

#[test]
fn webkit_line_clamp_cuts_a_vertical_webkit_box() {
    let legacy = clamped(&format!("{LEGACY};overflow:hidden"));
    assert!(has_ink(&legacy, 0..20), "the kept lines drew no text");
    assert!(!has_ink(&legacy, 20..height()), "text below the clamp");
    assert!(
        legacy == clamped("line-clamp:2;overflow:hidden"),
        "differs from line-clamp"
    );
}

#[test]
fn webkit_line_clamp_works_in_an_inline_webkit_box() {
    let rule = "display:-webkit-inline-box;-webkit-box-orient:vertical;-webkit-line-clamp:2;\
                overflow:hidden;width:120px";
    let inline = clamped(rule);
    assert!(has_ink(&inline, 0..20), "the kept lines drew no text");
    assert!(!has_ink(&inline, 20..height()), "text below the clamp");
}

#[test]
fn webkit_line_clamp_needs_the_box_display() {
    let plain = clamped("overflow:hidden");
    assert!(has_ink(&plain, 20..height()), "the text is too short");
    assert!(
        clamped("-webkit-line-clamp:2;overflow:hidden") == plain,
        "a plain block was clamped"
    );
}

#[test]
fn webkit_line_clamp_needs_the_vertical_orient() {
    let flex = clamped("display:flex;overflow:hidden");
    assert!(has_ink(&flex, 20..height()), "the text is too short");
    assert!(
        clamped("display:-webkit-box;-webkit-line-clamp:2;overflow:hidden") == flex,
        "a horizontal box was clamped"
    );
}

#[test]
fn webkit_line_clamp_keeps_later_lines_when_overflow_shows() {
    let buffer = draw(
        &format!(".c{{{LEGACY}}}.n{{height:10px;background:rgb(0,0,255)}}"),
        &format!("<div class=\"c\">{WORDS}</div><div class=\"n\"></div>"),
    );
    let [red, green, blue, _] = pixel(&buffer, 118, 25);
    assert_eq!([red, green, blue], [0, 0, 255], "the box is two lines tall");
    assert!(has_ink(&buffer, 30..40), "the fourth line is hidden");
}

#[test]
fn clamp_ellipsis_follows_the_last_word() {
    let clamp = "display:-webkit-box;-webkit-box-orient:vertical;-webkit-line-clamp:1;\
                 overflow:hidden";
    let clamped = draw(
        &format!(".c{{{clamp}}}"),
        "<div class=\"c\">aaaa bbbb cccccccccccccccccc</div>",
    );
    let literal = draw("", "<div>aaaa bbbb\u{2026}</div>");
    assert!(has_ink(&literal, 0..10), "the literal page drew no text");
    assert!(
        clamped == literal,
        "the ellipsis is not right after the word"
    );
}

#[test]
fn clamp_ellipsis_keeps_the_last_word_on_a_right_aligned_line() {
    let clamp = "display:-webkit-box;-webkit-box-orient:vertical;-webkit-line-clamp:1;\
                 overflow:hidden;text-align:right";
    let clamped = draw(
        &format!(".c{{{clamp}}}"),
        "<div class=\"c\">aaaa bbbb cccccccccccccccccc</div>",
    );
    let literal = draw("", "<div style=\"text-align:right\">aaaa bbbb</div>");
    assert!(has_ink(&literal, 0..10), "the literal page drew no text");
    assert!(clamped == literal, "the last word was cut");
}

fn balanced(rule: &str, text: &str) -> Vec<u8> {
    draw(
        &format!(".b{{text-wrap:balance;{rule}}}"),
        &format!("<div class=\"b\">{text}</div>"),
    )
}

fn broken(rule: &str, text: &str) -> Vec<u8> {
    let buffer = draw(
        &format!(".b{{{rule}}}"),
        &format!("<div class=\"b\">{text}</div>"),
    );
    assert!(has_ink(&buffer, 0..20), "the literal page drew no text");
    buffer
}

#[test]
fn balance_evens_out_two_lines() {
    let greedy = broken("", "aaaa bbbb cccc dddd eeee ffff");
    let even = broken("", "aaaa bbbb cccc<br>dddd eeee ffff");
    let balanced = balanced("", "aaaa bbbb cccc dddd eeee ffff");
    assert!(greedy != even, "the text already wraps evenly");
    assert!(balanced == even, "the lines are uneven");
}

#[test]
fn balance_keeps_the_text_alignment() {
    for align in ["center", "right"] {
        let rule = format!("text-align:{align}");
        let balanced = balanced(&rule, "aaaa bbbb cccc dddd eeee ff");
        let even = broken(&rule, "aaaa bbbb cccc<br>dddd eeee ff");
        assert!(balanced == even, "text-align: {align}");
    }
}

#[test]
fn balance_keeps_the_start_edge_in_rtl() {
    let balanced = balanced("direction:rtl", "aaaa bbbb cccc dddd eeee ff");
    for rows in [0..10, 10..20] {
        let left = first_ink_column(&balanced, rows.clone());
        assert!(
            left.is_some_and(|x| x > 20),
            "rows {rows:?} start at {left:?}"
        );
    }
}

#[test]
fn balance_leaves_long_paragraphs_alone() {
    let rule = "width:60px";
    let text = "aa bb cc dd ee ff gg hh ii jj kk ll mm nn oo pp qq rr ss tt uu";
    let greedy = broken(rule, text);
    assert!(has_ink(&greedy, 60..70), "the text is under seven lines");
    assert!(
        balanced(rule, text) == greedy,
        "seven lines were rebalanced"
    );
}

#[test]
fn balance_skips_text_with_a_forced_break() {
    let text = "aaaa bbbb cccc dddd eeee ffff<br>gg";
    assert!(
        balanced("", text) == broken("", text),
        "text with a forced break was rebalanced"
    );
}
