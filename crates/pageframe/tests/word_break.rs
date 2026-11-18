mod scaled;
mod support;

use pageframe::App;
use scaled::render_at_scale;
use support::{open, pixel, render};

const BREAK_ALL: &str = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}\
p{margin:0 0 4px;width:83px;border:1px solid #ccc;word-break:break-all}</style>\
<p id=a>Averyveryverylongwordthatmustbreak</p>\
<p id=b>abc.defghij,klmnop;qrstuv</p>\
<p id=c>12345678901234567890123</p>\
<p id=d>foo-barbazquxquux-12345</p>\
<p id=e>a(bcdefghijk)lmnopqrstu</p>\
<p id=f>word word wordwordwordwordword</p>\
<p id=h>https://example.com/a/very/long</p>\
<p id=i style='word-break:normal'>abc.defghij,klmnop;qrstuv</p>";

#[test]
fn break_all_splits_words_where_chrome_does() {
    let harness = open(App::new((), BREAK_ALL));
    let base = harness.base();
    let chrome = [
        ("#a", 50.0),
        ("#b", 34.0),
        ("#c", 50.0),
        ("#d", 34.0),
        ("#e", 34.0),
        ("#f", 50.0),
        ("#h", 50.0),
        ("#i", 18.0),
    ];
    let mismatches: Vec<String> = chrome
        .iter()
        .filter_map(|&(selector, height)| {
            let id = base
                .query_selector_all(selector)
                .expect("the selector parses")[0];
            let ours = base
                .get_node(id)
                .expect("the node exists")
                .final_layout()
                .size
                .height;
            (ours != height).then(|| format!("{selector} is {ours}px tall, Chrome has {height}px"))
        })
        .collect();
    drop(base);
    assert_eq!(mismatches, Vec::<String>::new());
}

const BREAK_ALL_MARKS: &str = "<!doctype html><style>html,body{margin:0;background:#fff}\
body{display:flex;align-items:flex-start;font:10px/10px Consolas,monospace}\
p{margin:0 8px 0 0;word-break:break-all}i{font-style:normal;color:#f00}</style>\
<p style='width:4ch'>abcd<i>-</i>efgh-abcd</p>\
<p style='width:4ch'>abcd<i>|</i>efgh|abcd</p>\
<p style='width:5ch'>abcd<i>+</i>efgh+abcd</p>";

#[test]
fn break_all_breaks_around_marks_like_chrome() {
    let mut harness = open(App::new((), BREAK_ALL_MARKS));
    let buffer = render(&mut harness);
    let cells: Vec<Option<(usize, usize)>> = [0, 30, 60]
        .iter()
        .map(|&left| {
            let red: Vec<(usize, usize)> = (0..40)
                .flat_map(|row| (0..28).map(move |column| (column, row)))
                .filter(|&(column, row)| {
                    let found = pixel(&buffer, left + column, row);
                    found[0] > 180 && found[1] < 120
                })
                .collect();
            let count = red.len();
            (count > 0).then(|| {
                let sum_x: usize = red.iter().map(|point| point.0).sum();
                let sum_y: usize = red.iter().map(|point| point.1).sum();
                (sum_x * 2 / (count * 11), sum_y / (count * 10))
            })
        })
        .collect();
    assert_eq!(cells, vec![Some((0, 1)), Some((0, 1)), Some((4, 0))]);
}

#[test]
fn break_all_text_stays_inside_its_box() {
    let mut harness = open(App::new((), BREAK_ALL));
    let buffer = render(&mut harness);
    let spill: Vec<(usize, usize)> = (0..80)
        .flat_map(|row| (90..120).map(move |column| (column, row)))
        .filter(|&(column, row)| pixel(&buffer, column, row)[0] < 200)
        .collect();
    assert_eq!(spill.first(), None, "ink right of the boxes");
}

const BOUNDARY: &str = "<!doctype html><style>html,body{margin:0}\
.boxes{font-size:0}.boxes span{display:inline-block;width:11px;height:5px;background:#f00}\
section{display:flow-root;font:10px/10px Consolas,monospace}\
b{float:left;width:10px;height:30px;background:#00f}</style>\
<div id=fits class=boxes style='width:21.99px'><span></span><span></span></div>\
<div id=wraps class=boxes style='width:21.98px'><span></span><span></span></div>\
<section style='width:calc(4ch + 10px)'><b></b><div id=beside>abcd abcd</div></section>\
<section style='width:calc(4ch + 9.98px)'><b></b><div id=below>abcd abcd</div></section>";

#[test]
fn lines_fit_one_layout_unit_past_their_box_like_chrome() {
    let mut harness = open(App::new((), BOUNDARY));
    let heights = ["#fits", "#wraps", "#beside", "#below"].map(|selector| {
        let doc = harness.base_mut();
        let id = doc
            .query_selector(selector)
            .expect("valid selector")
            .expect("the div exists");
        doc.get_client_bounding_rect(id)
            .map(|rect| rect.height)
            .expect("the div has a box")
    });
    assert_eq!(heights, [5.0, 10.0, 20.0, 50.0]);
}

const DEVICE_BOUNDARY: &str = "<!doctype html><style>html,body{margin:0}\
div{font-size:0}span{display:inline-block;width:11px;height:5px;background:#f00}</style>\
<div id=fits style='width:21.995px'><span></span><span></span></div>\
<div id=wraps style='width:21.99px'><span></span><span></span></div>";

#[test]
fn the_fit_epsilon_is_one_device_layout_unit_at_2x() {
    let mut harness = open(App::new((), DEVICE_BOUNDARY));
    render_at_scale(&mut harness, 2);
    let heights = ["#fits", "#wraps"].map(|selector| {
        let doc = harness.base_mut();
        let id = doc
            .query_selector(selector)
            .expect("valid selector")
            .expect("the div exists");
        doc.get_client_bounding_rect(id)
            .map(|rect| rect.height)
            .expect("the div has a box")
    });
    assert_eq!(heights, [5.0, 10.0]);
}

const LEADING_FLOAT: &str = "<!doctype html><style>html,body{margin:0}\
div{font:10px/10px Consolas,monospace}b{float:left;width:10px;height:30px}</style>\
<div id=below style='width:calc(4ch + 9.98px)'><b></b><span id=first>abcd</span> \
<span id=second>abcd</span></div>";

#[test]
fn a_line_too_wide_beside_its_leading_float_drops_below_it_like_chrome() {
    let mut harness = open(App::new((), LEADING_FLOAT));
    let rects = ["#below", "#first", "#second"].map(|selector| {
        let doc = harness.base_mut();
        let id = doc
            .query_selector(selector)
            .expect("valid selector")
            .expect("the element exists");
        doc.get_client_bounding_rect(id)
            .map(|rect| [rect.x, rect.y, rect.height])
            .expect("the element has a box")
    });
    assert_eq!(rects[0][2], 50.0);
    assert_eq!([rects[1][0], rects[2][0]], [0.0, 0.0]);
    assert_eq!(rects[2][1] - rects[1][1], 10.0);
}

const OVERWIDE_FLOAT: &str = "<!doctype html><style>html,body{margin:0}\
div{font:10px/10px Consolas,monospace;width:50px}b{float:left;width:80px;height:20px}</style>\
<div id=empty><b></b><i></i></div>\
<div id=thin><b></b><span id=nub style='display:inline-block;width:0;height:4px'></span></div>";

#[test]
fn zero_width_lines_stay_beside_a_float_wider_than_their_box_like_chrome() {
    let mut harness = open(App::new((), OVERWIDE_FLOAT));
    let rects = ["#empty", "#thin", "#nub"].map(|selector| {
        let doc = harness.base_mut();
        let id = doc
            .query_selector(selector)
            .expect("valid selector")
            .expect("the element exists");
        doc.get_client_bounding_rect(id)
            .map(|rect| [rect.x, rect.y, rect.height])
            .expect("the element has a box")
    });
    assert_eq!([rects[0][2], rects[1][2]], [0.0, 10.0]);
    assert_eq!(rects[2][0], 80.0);
    assert_eq!(rects[2][1] - rects[1][1], 4.0);
}
