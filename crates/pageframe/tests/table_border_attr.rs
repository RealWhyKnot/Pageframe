use pageframe::{App, Page};
use pageframe_test_harness::Harness;

const TABLES: &str = "<table id=t1 border='0px'><tr><td id=c1>a</td><td>b</td></tr></table>\
<table id=t2 border='3'><tr><td id=c2>a</td><td id=c2b>b</td></tr></table>\
<table id=t3 border=''><tr><td id=c3>a</td><td>b</td></tr></table>\
<table id=t4 border='0'><tr><td id=c4>a</td><td>b</td></tr></table>\
<table id=t5 border='abc'><tr><td id=c5>a</td><td>b</td></tr></table>\
<table id=t6 border='2px'><tr><td id=c6>a</td><td>b</td></tr></table>\
<table id=t10 border='-1'><tr><td id=c10>a</td></tr></table>\
<table id=t11 border=' 4'><tr><td id=c11>a</td></tr></table>";

const COLLAPSED: &str = "<table id=t7 border='3' style='border-collapse:collapse'>\
<tr><td id=c7>a</td><td id=c7b>b</td></tr><tr><td id=c7c>c</td><td id=c7d>d</td></tr></table>\
<table id=t8 style='border-collapse:collapse;border:6px solid'>\
<tr><td id=c8 style='border:4px solid'>a</td><td id=c8b style='border:2px solid'>b</td></tr>\
<tr><td id=c8c style='border:3px solid'>c</td><td id=c8d>d</td></tr></table>\
<table id=t9 style='border-collapse:collapse;border:5px solid'>\
<tr><td id=c9 style='border:1px solid;padding:0'>a</td></tr></table>\
<div style='padding:5px'><table id=t12 style='border-collapse:collapse;border:7px solid'>\
<tr><td id=c12 style='border:3px solid'>a</td></tr></table></div>";

type Expected<'a> = &'a [(&'a str, [f64; 4], [f64; 2])];

fn check(body: &str, expected: Expected<'_>) {
    let source = format!(
        "<!doctype html><meta charset=utf-8><style>html,body{{margin:0;font:14px Arial}}\
         td{{padding:1px}}</style>{body}"
    );
    let page: Page<()> = App::new((), &source)
        .page(640, 400)
        .expect("the page parses");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let doc = harness.base();
    for (id, want, offset) in expected {
        let node = doc
            .query_selector(&format!("#{id}"))
            .expect("valid selector")
            .expect("the element exists");
        let rect = doc
            .get_client_bounding_rect(node)
            .expect("the element has a box");
        let found = [rect.x, rect.y, rect.width, rect.height];
        assert!(
            found.iter().zip(want).all(|(a, b)| (a - b).abs() <= 0.01),
            "#{id}: found {found:?}, Chrome 157 has {want:?}"
        );
        let place = doc.offset_rect(node).expect("the element has an offset");
        assert_eq!([place.x, place.y], *offset, "#{id} offsetLeft/offsetTop");
    }
    drop(doc);
}

#[test]
fn border_attribute_sets_table_and_cell_borders() {
    check(
        TABLES,
        &[
            ("t1", [0.0, 0.0, 25.59, 22.0], [0.0, 0.0]),
            ("c1", [2.0, 2.0, 9.8, 18.0], [2.0, 2.0]),
            ("t2", [0.0, 22.0, 35.59, 30.0], [0.0, 22.0]),
            ("c2", [5.0, 27.0, 11.8, 20.0], [2.0, 2.0]),
            ("c2b", [18.8, 27.0, 11.8, 20.0], [16.0, 2.0]),
            ("t3", [0.0, 52.0, 31.59, 26.0], [0.0, 52.0]),
            ("c3", [3.0, 55.0, 11.8, 20.0], [2.0, 2.0]),
            ("t4", [0.0, 78.0, 25.59, 22.0], [0.0, 78.0]),
            ("c4", [2.0, 80.0, 9.8, 18.0], [2.0, 2.0]),
            ("t5", [0.0, 100.0, 31.59, 26.0], [0.0, 100.0]),
            ("c5", [3.0, 103.0, 11.8, 20.0], [2.0, 2.0]),
            ("t6", [0.0, 126.0, 33.59, 28.0], [0.0, 126.0]),
            ("c6", [4.0, 130.0, 11.8, 20.0], [2.0, 2.0]),
            ("t10", [0.0, 154.0, 17.8, 26.0], [0.0, 154.0]),
            ("c10", [3.0, 157.0, 11.8, 20.0], [2.0, 2.0]),
            ("t11", [0.0, 180.0, 23.8, 32.0], [0.0, 180.0]),
            ("c11", [6.0, 186.0, 11.8, 20.0], [2.0, 2.0]),
        ],
    );
}

#[test]
fn collapsed_cells_offset_from_the_inner_half_of_the_table_border() {
    check(
        COLLAPSED,
        &[
            ("t7", [0.0, 0.0, 26.59, 43.0], [0.0, 0.0]),
            ("c7", [1.5, 1.5, 11.8, 20.0], [0.0, 0.0]),
            ("c7b", [13.3, 1.5, 11.8, 20.0], [12.0, 0.0]),
            ("c7c", [1.5, 21.5, 11.8, 20.0], [0.0, 20.0]),
            ("c7d", [13.3, 21.5, 11.8, 20.0], [12.0, 20.0]),
            ("t8", [0.0, 43.0, 35.59, 52.0], [0.0, 43.0]),
            ("c8", [3.0, 46.0, 14.8, 23.0], [0.0, 0.0]),
            ("c8b", [17.8, 46.0, 14.8, 23.0], [15.0, 0.0]),
            ("c8c", [3.0, 69.0, 14.8, 23.0], [0.0, 23.0]),
            ("c8d", [17.8, 69.0, 14.8, 23.0], [15.0, 23.0]),
            ("t9", [0.0, 95.0, 17.8, 26.0], [0.0, 95.0]),
            ("c9", [2.5, 97.5, 12.8, 21.0], [0.0, 0.0]),
            ("t12", [5.0, 126.0, 23.8, 32.0], [5.0, 126.0]),
            ("c12", [8.5, 129.5, 16.8, 25.0], [0.0, 0.0]),
        ],
    );
}
