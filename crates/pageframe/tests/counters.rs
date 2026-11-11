mod support;

use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use support::{open, pixel, render};

const BASE: &str = "html,body{margin:0;background:white;font:10px/16px monospace;color:black}\
p,div{margin:0}\
.hide{display:none}";

const LITERAL: &str = ".t::before{content:attr(data-b)}.t::after{content:attr(data-a)}";

const QUOTED: &str = "q::before{content:attr(data-b)}q::after{content:attr(data-a)}";

fn frame(harness: &mut Harness<Page<()>>) -> Vec<u8> {
    harness.pump();
    render(harness)
}

fn page(style: &str, body: &str) -> String {
    format!("<html><head><style>{BASE}{style}</style></head><body>{body}</body></html>")
}

fn draw(style: &str, body: &str) -> Vec<u8> {
    let mut harness = open(App::new((), &page(style, body)));
    frame(&mut harness)
}

fn has_ink(buffer: &[u8]) -> bool {
    buffer.as_chunks::<4>().0.iter().any(|rgba| rgba[0] < 128)
}

fn assert_same(generated: &[u8], literal: &[u8]) {
    assert!(has_ink(literal), "the literal page drew no text");
    assert!(generated == literal, "generated text differs from literal");
}

fn rows(labels: &[&str]) -> String {
    labels
        .iter()
        .map(|label| format!("<p class=\"t\" data-b=\"{label}\">x</p>"))
        .collect()
}

#[test]
fn counter_numbers_each_incremented_element() {
    let generated = draw(
        "div{counter-reset:sec}p{counter-increment:sec}p::before{content:counter(sec) \". \"}",
        "<div><p>x</p><p>x</p><p>x</p></div><div><p>x</p></div>",
    );
    let literal = draw(
        LITERAL,
        &format!(
            "<div>{}</div><div>{}</div>",
            rows(&["1. ", "2. ", "3. "]),
            rows(&["1. "])
        ),
    );
    assert_same(&generated, &literal);
}

#[test]
fn counter_uses_the_named_style() {
    let generated = draw(
        "div{counter-reset:sec 2}p{counter-increment:sec}\
         p::before{content:counter(sec,upper-roman) \") \"}\
         p.a::before{content:counter(sec,lower-alpha) \") \"}",
        "<div><p>x</p><p>x</p><p class=\"a\">x</p></div>",
    );
    let literal = draw(
        LITERAL,
        &format!("<div>{}</div>", rows(&["III) ", "IV) ", "e) "])),
    );
    assert_same(&generated, &literal);
}

#[test]
fn counter_follows_reset_step_and_set() {
    let generated = draw(
        "div{counter-reset:k 10}p{counter-increment:k 5}p.z{counter-set:k 100}\
         p::before{content:\"[\" counter(k) \"] \"}",
        "<div><p>x</p><p class=\"z\">x</p><p>x</p></div>",
    );
    let literal = draw(
        LITERAL,
        &format!("<div>{}</div>", rows(&["[15] ", "[100] ", "[105] "])),
    );
    assert_same(&generated, &literal);
}

#[test]
fn counters_joins_every_nested_scope() {
    let style = "section{counter-reset:item}p{counter-increment:item}";
    let generated = draw(
        &format!("{style}p::before{{content:counters(item,\".\") \" \"}}"),
        "<section><p>x</p><p>x</p><div><section><p>x</p><p>x</p></section></div><p>x</p></section>",
    );
    let literal = draw(
        &format!("{style}{LITERAL}"),
        &format!(
            "<section>{}<div><section>{}</section></div>{}</section>",
            rows(&["1 ", "2 "]),
            rows(&["2.1 ", "2.2 "]),
            rows(&["3 "])
        ),
    );
    assert_same(&generated, &literal);
}

#[test]
fn unknown_counter_reads_zero() {
    let generated = draw(
        "p::before{content:counter(missing) \"|\" counters(missing,\".\") \" \"}",
        "<p>x</p>",
    );
    let literal = draw(LITERAL, &rows(&["0|0 "]));
    assert_same(&generated, &literal);
}

#[test]
fn list_item_counter_counts_list_items() {
    let style = "ol{margin:0;padding:0;list-style:none}";
    let generated = draw(
        &format!("{style}li::before{{content:counter(list-item) \"= \"}}"),
        "<ol><li>x</li><li>x</li></ol><ol><li>x</li></ol>",
    );
    let literal = draw(
        &format!("{style}{LITERAL}"),
        "<ol><li class=\"t\" data-b=\"1= \">x</li><li class=\"t\" data-b=\"2= \">x</li></ol>\
         <ol><li class=\"t\" data-b=\"1= \">x</li></ol>",
    );
    assert_same(&generated, &literal);
}

#[test]
fn counter_updates_when_an_earlier_element_is_hidden() {
    let style = "div{counter-reset:sec}p{counter-increment:sec}";
    let html = page(
        &format!("{style}p::before{{content:counter(sec) \". \"}}"),
        "<div><p id=\"first\">x</p><p>x</p><p>x</p></div>",
    );
    let mut harness = open(App::new((), &html));
    let before = frame(&mut harness);
    harness.doc.ui().select("#first").add_class("hide");
    let after = frame(&mut harness);

    let all = draw(
        &format!("{style}{LITERAL}"),
        &format!("<div>{}</div>", rows(&["1. ", "2. ", "3. "])),
    );
    let two = draw(
        &format!("{style}{LITERAL}"),
        &format!("<div>{}</div>", rows(&["1. ", "2. "])),
    );
    assert_same(&before, &all);
    assert_same(&after, &two);
}

#[test]
fn q_nests_quote_marks() {
    let generated = draw("", "<p><q>a <q>b <q>c</q></q></q></p>");
    let literal = draw(
        QUOTED,
        "<p><q data-b=\"\u{201c}\" data-a=\"\u{201d}\">a \
         <q data-b=\"\u{2018}\" data-a=\"\u{2019}\">b \
         <q data-b=\"\u{2018}\" data-a=\"\u{2019}\">c</q></q></q></p>",
    );
    assert_same(&generated, &literal);
}

#[test]
fn q_uses_the_quotes_property() {
    let generated = draw(
        "p{quotes:\"<\" \">\" \"(\" \")\"}",
        "<p><q>a <q>b</q></q></p>",
    );
    let literal = draw(
        QUOTED,
        "<p><q data-b=\"&lt;\" data-a=\"&gt;\">a <q data-b=\"(\" data-a=\")\">b</q></q></p>",
    );
    assert_same(&generated, &literal);
}

#[test]
fn q_follows_the_language() {
    let generated = draw("", "<p lang=\"de\"><q>a <q>b</q></q></p>");
    let literal = draw(
        QUOTED,
        "<p><q data-b=\"\u{201e}\" data-a=\"\u{201c}\">a \
         <q data-b=\"\u{201a}\" data-a=\"\u{2018}\">b</q></q></p>",
    );
    assert_same(&generated, &literal);
}

#[test]
fn quotes_none_draws_no_marks() {
    let generated = draw("q{quotes:none}", "<p><q>a</q></p>");
    let literal = draw("q::before,q::after{content:none}", "<p><q>a</q></p>");
    assert_same(&generated, &literal);
}

#[test]
fn no_open_quote_only_deepens_the_nesting() {
    let generated = draw(
        ".n::before{content:no-open-quote \"x\" open-quote}.n::after{content:close-quote no-close-quote}",
        "<p><span class=\"n\">a</span></p><p><q>b</q></p>",
    );
    let literal = draw(
        &format!("{LITERAL}{QUOTED}"),
        "<p><span class=\"t\" data-b=\"x\u{2018}\" data-a=\"\u{2019}\">a</span></p>\
         <p><q data-b=\"\u{201c}\" data-a=\"\u{201d}\">b</q></p>",
    );
    assert_same(&generated, &literal);
}

#[test]
fn inline_background_covers_nested_inlines_and_quotes() {
    let buffer = draw(
        "p{font:20px/20px monospace;color:transparent}.bg{background:rgb(255,0,0)}",
        "<p><span class=\"bg\"><b>ab</b></span></p><p><q class=\"bg\"><i>cd</i></q></p>",
    );
    for (x, y) in [(6, 10), (18, 10), (6, 30), (18, 30), (30, 30), (42, 30)] {
        let [red, green, blue, _] = pixel(&buffer, x, y);
        assert_eq!([red, green, blue], [255, 0, 0], "pixel ({x}, {y})");
    }
}

#[test]
fn inline_background_stops_at_the_block() {
    let buffer = draw(
        "div{font:20px/20px monospace;color:transparent}.bg{background:rgba(0,0,255,0.5)}",
        "<div class=\"bg\"><span>ab</span><div>c</div></div>",
    );
    let block = pixel(&buffer, 60, 30);
    for (x, y) in [(6, 10), (18, 10)] {
        assert_eq!(pixel(&buffer, x, y), block, "pixel ({x}, {y})");
    }
}

#[test]
fn list_item_counter_follows_start_and_reversed() {
    let style = "ol{margin:0;padding:0;list-style:none}";
    let generated = draw(
        &format!("{style}li::before{{content:counter(list-item) \"= \"}}"),
        "<ol start=\"5\"><li>x</li><li>x</li></ol>         <ol reversed start=\"10\"><li>x</li><li>x</li></ol>",
    );
    let literal = draw(
        &format!("{style}{LITERAL}"),
        "<ol><li class=\"t\" data-b=\"5= \">x</li><li class=\"t\" data-b=\"6= \">x</li></ol>         <ol><li class=\"t\" data-b=\"10= \">x</li><li class=\"t\" data-b=\"9= \">x</li></ol>",
    );
    assert_same(&generated, &literal);
}

#[test]
fn list_item_counter_set_wins_over_the_implicit_step() {
    let style = "ol{margin:0;padding:0;list-style:none}";
    let generated = draw(
        &format!("{style}li::before{{content:counter(list-item) \"= \"}}"),
        "<ol><li style=\"counter-set:list-item 9\">x</li><li>x</li></ol>",
    );
    let literal = draw(
        &format!("{style}{LITERAL}"),
        "<ol><li class=\"t\" data-b=\"9= \">x</li><li class=\"t\" data-b=\"10= \">x</li></ol>",
    );
    assert_same(&generated, &literal);
}

#[test]
fn markers_follow_list_item_counter_properties() {
    let style = "ol{margin:0;padding:0 0 0 40px}";
    let styled = draw(
        style,
        "<ol><li style=\"counter-set:list-item 9\">x</li><li>x</li></ol>         <ol><li style=\"counter-increment:list-item 3\">x</li><li>x</li></ol>",
    );
    let numbered = draw(
        style,
        "<ol start=\"9\"><li>x</li><li>x</li></ol><ol start=\"3\"><li>x</li><li>x</li></ol>",
    );
    assert_same(&styled, &numbered);
}
