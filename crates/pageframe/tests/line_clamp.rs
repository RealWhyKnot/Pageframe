mod support;

use pageframe::App;
use support::{HEIGHT, WIDTH, open, pixel, render};

const BASE: &str = "html,body{margin:0;background:white;font:10px/10px monospace;color:black}\
div{margin:0}";

const LEGACY: &str = "display:-webkit-box;-webkit-box-orient:vertical;-webkit-line-clamp:2";

const WORDS: &str = "aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn oooo \
pppp qqqq rrrr ssss tttt";

const BALANCE_RULE: &str = "width:120px;font-size:8px";
const BALANCE_WORDS: &str = "aaaa bbb cc dddd ee aaaa bbb cc dddd ee";

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
    ink_columns(buffer, rows).next()
}

fn ink_columns(
    buffer: &[u8],
    rows: std::ops::Range<usize>,
) -> impl DoubleEndedIterator<Item = usize> {
    let width = usize::try_from(WIDTH).expect("test width fits in usize");
    (0..width).filter(move |&x| {
        rows.clone().any(|y| {
            let [red, green, blue, _] = pixel(buffer, x, y);
            red < 128 && green < 128 && blue < 128
        })
    })
}

fn ink_width(buffer: &[u8], rows: std::ops::Range<usize>) -> usize {
    let mut columns = ink_columns(buffer, rows);
    let first = columns.next().expect("line drew no text");
    columns.next_back().unwrap_or(first) + 1 - first
}

fn row_pixels(buffer: &[u8], rows: std::ops::Range<usize>) -> &[u8] {
    let stride = usize::try_from(WIDTH).expect("test width fits in usize") * 4;
    &buffer[rows.start * stride..rows.end * stride]
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
}

#[test]
fn unprefixed_line_clamp_is_ignored() {
    let plain = clamped("overflow:hidden");
    assert!(has_ink(&plain, 20..height()), "the text is too short");
    assert!(
        clamped("line-clamp:2;overflow:hidden") == plain,
        "line-clamp without the prefix clamped"
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

#[cfg(target_os = "windows")]
#[test]
fn balance_breaks_at_the_same_words_as_chrome_at_both_scales() {
    use pageframe_test_harness::Harness;
    use pageframe_traits::shell::Viewport;

    let html = format!(
        "<!doctype html><style>div{{width:150px;font:10px/20px Consolas;text-wrap:balance}}</style>\
         <div>aé {BALANCE_WORDS}<br>{BALANCE_WORDS}</div>"
    );
    let page = App::new((), &html).page(200, 100).expect("valid page");
    let mut harness = Harness::wrap(page);
    for scale in [1_u16, 2] {
        {
            let mut doc = harness.base_mut();
            let theme = doc.get_viewport().color_scheme;
            let size = u32::from(scale);
            doc.set_viewport(Viewport::new(
                200 * size,
                100 * size,
                f32::from(scale),
                theme,
            ));
        }
        harness.pump();
        let doc = harness.base_mut();
        let id = doc
            .query_selector("div")
            .expect("valid selector")
            .expect("div");
        let inline = doc
            .get_node(id)
            .and_then(|node| node.element_data())
            .and_then(|element| element.inline_layout_data.as_deref())
            .expect("div has inline layout");
        let lines: Vec<_> = inline
            .layout
            .lines()
            .map(|line| inline.text[line.text_range()].trim_end().to_owned())
            .collect();
        drop(doc);
        assert_eq!(
            lines,
            [
                "aé aaaa bbb cc dddd",
                "ee aaaa bbb cc dddd ee",
                "aaaa bbb cc dddd ee aaaa",
                "bbb cc dddd ee",
            ],
            "{scale}x"
        );
    }
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
fn balance_stops_at_the_first_forced_break() {
    let text = format!("{BALANCE_WORDS}<br>{BALANCE_WORDS}");
    let greedy = broken(BALANCE_RULE, &text);
    let balanced = balanced(BALANCE_RULE, &text);
    let first_width = ink_width(&balanced, 0..10);
    assert_eq!(first_width, ink_width(&balanced, 10..20));
    assert!(
        first_width < ink_width(&greedy, 0..10),
        "the first line stayed at its greedy width"
    );
    assert!(has_ink(&greedy, 30..40), "the fourth line drew no text");
    assert!(
        row_pixels(&balanced, 20..height()) == row_pixels(&greedy, 20..height()),
        "the second paragraph wrapped differently"
    );
}

#[test]
fn balance_leaves_a_later_paragraph_after_a_single_line() {
    let text = format!("aaaa<br>{BALANCE_WORDS}");
    let greedy = broken(BALANCE_RULE, &text);
    let single = balanced(BALANCE_RULE, &text);
    assert!(has_ink(&greedy, 20..30), "the third line drew no text");
    assert!(
        row_pixels(&single, 10..height()) == row_pixels(&greedy, 10..height()),
        "the second paragraph wrapped differently"
    );

    let rule = format!("{BALANCE_RULE};white-space:pre-line");
    let text = format!("{BALANCE_WORDS}\n{BALANCE_WORDS}");
    let multiline = balanced(&rule, &text);
    let even = balanced(BALANCE_RULE, BALANCE_WORDS);
    assert!(
        row_pixels(&multiline, 0..20) == row_pixels(&even, 0..20),
        "the first paragraph stayed uneven before a preserved newline"
    );
}

#[test]
fn balance_uses_the_first_plaintext_paragraphs_direction() {
    let rule = format!("{BALANCE_RULE};direction:rtl;unicode-bidi:plaintext");
    let text = format!("{BALANCE_WORDS}<br>א {BALANCE_WORDS}");
    let rtl = balanced(&rule, &text);
    let ltr = balanced(BALANCE_RULE, BALANCE_WORDS);
    for rows in [0..10, 10..20] {
        let left = first_ink_column(&rtl, rows.clone());
        assert!(
            left.is_some_and(|x| x <= 1),
            "rows {rows:?} start at {left:?}"
        );
    }
    assert!(
        row_pixels(&rtl, 0..20) == row_pixels(&ltr, 0..20),
        "the Latin paragraph differs from the balanced LTR text"
    );
}

#[test]
fn balance_aligns_an_rtl_plaintext_paragraph_to_the_right() {
    let rule = format!("{BALANCE_RULE};direction:ltr;unicode-bidi:plaintext");
    let text = format!("א {BALANCE_WORDS}<br>{BALANCE_WORDS}");
    let rtl = balanced(&rule, &text);
    let greedy = broken(&rule, &text);
    let right_edge = usize::try_from(WIDTH).expect("test width fits in usize") - 1;
    for rows in [0..10, 10..20] {
        let mut columns = ink_columns(&rtl, rows.clone());
        let left = columns.next().expect("balanced line drew no text");
        let right = columns.next_back().expect("balanced line drew one column");
        assert!(
            left > 20 && right >= right_edge - 1,
            "rows {rows:?} span {left}..={right}"
        );
    }
    assert!(
        row_pixels(&rtl, 20..height()) == row_pixels(&greedy, 20..height()),
        "the Latin paragraph wrapped differently"
    );
}

const PARAGRAPHS: &str =
    "p{margin:0}.c{display:-webkit-box;-webkit-box-orient:vertical;overflow:hidden}";

fn paragraphs(lines: u32, body: &str) -> Vec<u8> {
    draw(
        &format!("{PARAGRAPHS}.c{{-webkit-line-clamp:{lines}}}"),
        &format!("<div class=\"c\">{body}</div>"),
    )
}

#[test]
fn clamp_counts_lines_across_paragraphs() {
    let clamped = paragraphs(
        2,
        "<p>aaaa bbbb cccc</p><p>dddd eeee fffffff gggg hhhh iiii jjjj</p>",
    );
    let literal = draw(
        PARAGRAPHS,
        "<div><p>aaaa bbbb cccc</p><p>dddd eeee fffffff…</p></div>",
    );
    assert!(
        has_ink(&literal, 10..20),
        "the literal page drew no second line"
    );
    assert!(
        clamped == literal,
        "the clamp did not land in the second paragraph"
    );
}

#[test]
fn clamp_ellipsis_shows_at_a_paragraph_end() {
    let clamped = paragraphs(1, "<p>aaaa bbbb</p><p>cccc dddd</p>");
    let literal = draw(PARAGRAPHS, "<div><p>aaaa bbbb…</p></div>");
    assert!(has_ink(&literal, 0..10), "the literal page drew no text");
    assert!(clamped == literal, "the first paragraph has no ellipsis");
}

#[test]
fn clamp_leaves_paragraphs_that_fit() {
    let body = "<p>aaaa bbbb</p><p>cccc dddd</p>";
    let clamped = paragraphs(3, body);
    assert!(
        clamped == draw(PARAGRAPHS, &format!("<div>{body}</div>")),
        "paragraphs that fit were clamped"
    );
}

#[test]
fn clamped_box_keeps_the_padding_under_the_line() {
    let body = "<div style=\"padding-bottom:6px\"><p>aaaa</p><p>bbbb</p></div>";
    let bar = "<div style=\"height:4px;background:black\"></div>";
    let clamped = draw(
        &format!("{PARAGRAPHS}.c{{-webkit-line-clamp:1}}"),
        &format!("<div class=\"c\">{body}</div>{bar}"),
    );
    let [above, ..] = pixel(&clamped, 100, 15);
    let [inside, ..] = pixel(&clamped, 100, 17);
    let [below, ..] = pixel(&clamped, 100, 21);
    assert!(above > 200, "the bar starts above the padding");
    assert!(inside < 50, "the bar is not right under the padding");
    assert!(below > 200, "the second paragraph still takes up room");
}

#[test]
fn outer_clamp_keeps_an_inner_clamp() {
    let inner =
        "<div class=\"c\" style=\"-webkit-line-clamp:1\">aaaa bbbb cccc dddd eeee ffff</div>";
    let nested = draw(
        &format!("{PARAGRAPHS}.o{{-webkit-line-clamp:5}}"),
        &format!("<div class=\"c o\">{inner}<p>gggg</p></div>"),
    );
    let alone = draw(PARAGRAPHS, &format!("<div>{inner}<p>gggg</p></div>"));
    assert!(
        has_ink(&alone, 10..20),
        "the page without the outer clamp drew no second block"
    );
    assert!(nested == alone, "the outer clamp changed the inner one");
}

#[test]
fn clamped_box_keeps_its_min_height() {
    let bar = "<div style=\"height:4px;background:black\"></div>";
    let clamped = draw(
        &format!("{PARAGRAPHS}.c{{-webkit-line-clamp:1;min-height:30px}}"),
        &format!("<div class=\"c\"><p>aaaa</p><p>bbbb</p></div>{bar}"),
    );
    let [above, ..] = pixel(&clamped, 100, 29);
    let [inside, ..] = pixel(&clamped, 100, 31);
    assert!(above > 200, "the box is shorter than its min-height");
    assert!(inside < 50, "the bar is not right under the box");
}

#[test]
fn clamp_ellipsis_stays_inside_a_line_beside_a_float() {
    let buffer = draw(
        &format!(".c{{{LEGACY};overflow:hidden}}.f{{float:left;width:60px;height:20px}}"),
        "<div class=\"c\"><div class=\"f\"></div>\
         <div>aaaa bbbbb cccc bbbbb dddd eeee ffff</div></div>",
    );
    let ink = |columns: std::ops::Range<usize>, rows: std::ops::Range<usize>| {
        rows.into_iter().any(|y| {
            columns.clone().any(|x| {
                let [red, green, blue, _] = pixel(&buffer, x, y);
                red < 128 && green < 128 && blue < 128
            })
        })
    };
    assert!(ink(109..116, 0..5), "the first line lost its last letter");
    assert!(!ink(109..120, 10..16), "the last letter was not cut");
    assert!(ink(109..120, 16..19), "no ellipsis at the end of the line");
}

#[test]
fn overflow_ellipsis_stops_before_a_right_float() {
    let buffer = draw(
        ".o{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}\
         .f{float:right;width:60px;height:10px}",
        "<div class=\"o\"><div class=\"f\"></div>aaaa bbbbb cccc dddd</div>",
    );
    assert!(has_ink(&buffer, 0..10), "the line drew no text");
    let beside_float = (0..10).any(|y| {
        (60..120).any(|x| {
            let [red, green, blue, _] = pixel(&buffer, x, y);
            red < 128 && green < 128 && blue < 128
        })
    });
    assert!(!beside_float, "text runs under the float");
}

#[test]
fn right_aligned_cut_line_keeps_the_letters_chrome_keeps() {
    let row = "H".repeat(33);
    let buffer = draw(
        &format!(".c{{{LEGACY};overflow:hidden;text-align:right;letter-spacing:-2px}}"),
        &format!("<div class=\"c\">{row}<br>{row}<br>HHH</div>"),
    );
    let tall_ink = |columns: std::ops::Range<usize>| {
        (11..14).any(|y| {
            columns.clone().any(|x| {
                let [red, green, blue, _] = pixel(&buffer, x, y);
                red < 128 && green < 128 && blue < 128
            })
        })
    };
    assert!(tall_ink(100..110), "the second line drew no letters");
    assert!(tall_ink(113..116), "one letter too many was cut");
}
