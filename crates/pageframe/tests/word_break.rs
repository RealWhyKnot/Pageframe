mod support;

use pageframe::App;
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
