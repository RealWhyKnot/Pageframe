use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use pageframe_traits::shell::Viewport;

const SELECTS: &str = "<div><select id=s1 multiple size=1><option selected>Apple</option>\
<option selected>Banana</option><option>Cherry</option></select></div>\
<div><select id=s2 multiple size=1><option>Apple</option><option>Banana</option></select></div>\
<div><select id=s3 multiple size=1><option selected>Apple</option>\
<option>Banana long name</option></select></div>\
<div><select id=s5 size=1><option>Apple</option><option>Banana</option></select></div>\
<div><select id=s7 multiple size=1 style='font-size:20px'><option selected>A</option>\
<option selected>B</option><option selected>C</option></select></div>\
<div><select id=s8 multiple size=1><option selected>A</option><option>B</option></select></div>\
<div><select id=s9 multiple size=1><option>A</option><option>B</option></select></div>\
<div><select id=s10 multiple size=1><option selected>A</option>\
<option selected>B</option></select></div>\
<div><select id=s12 multiple size=1 style='width:40px'><option selected>A</option>\
<option selected>B</option></select></div>\
<div><select id=s13 multiple size=1 dir=rtl><option selected>A</option>\
<option selected>B</option></select></div>\
<div><select id=s14 multiple size=1><option selected>Apple</option><option selected>B</option>\
<option selected>C</option><option selected>D</option><option selected>E</option>\
<option selected>F</option><option selected>G</option><option selected>H</option>\
<option selected>I</option><option selected>J</option><option selected>K</option></select></div>";

const GROUPED: &str = "<div><select id=o1><optgroup label=g><option>Apple</option></optgroup>\
</select></div>\
<div><select id=o2 dir=rtl><optgroup label=g><option>Apple</option></optgroup></select></div>\
<div><select id=o3><optgroup label=g><option>A</option></optgroup><option>Banana</option>\
</select></div>\
<div><select id=o4><optgroup label=g><option>Strawberry jam</option></optgroup>\
<option>B</option></select></div>\
<div><select id=o5 style='font:20px Georgia'><optgroup label=g><option>Apple</option>\
</optgroup></select></div>\
<div><select id=o6 multiple size=1><optgroup label=g><option selected>Apple</option>\
</optgroup></select></div>\
<div><select id=o7 multiple size=1><optgroup label=g><option selected>A</option></optgroup>\
<option selected>B</option></select></div>";

fn check(scale: u16, expected: &[(&str, [f64; 4])]) {
    check_page(SELECTS, scale, expected);
}

fn check_page(body: &str, scale: u16, expected: &[(&str, [f64; 4])]) {
    let source = format!(
        "<!doctype html><meta charset=utf-8><style>html,body{{margin:0;font:14px Arial}}\
         </style>{body}"
    );
    let page: Page<()> = App::new((), &source)
        .page(640, 400)
        .expect("the page parses");
    let mut harness = Harness::wrap(page);
    harness.pump();
    if scale != 1 {
        let mut doc = harness.base_mut();
        let color_scheme = doc.get_viewport().color_scheme;
        doc.set_viewport(Viewport::new(
            640 * u32::from(scale),
            400 * u32::from(scale),
            f32::from(scale),
            color_scheme,
        ));
        drop(doc);
        harness.pump();
    }
    let doc = harness.base();
    for (id, want) in expected {
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
            "#{id} at {scale}x: found {found:?}, Chrome 157 has {want:?}"
        );
    }
    drop(doc);
}

#[test]
fn single_row_multiple_select_sizes_like_a_dropdown() {
    check(
        1,
        &[
            ("s1", [0.0, 0.0, 83.0, 19.0]),
            ("s2", [0.0, 19.0, 83.0, 19.0]),
            ("s3", [0.0, 38.0, 134.0, 19.0]),
            ("s5", [0.0, 57.0, 68.0, 19.0]),
            ("s7", [0.0, 76.0, 114.0, 26.0]),
            ("s8", [0.0, 102.0, 83.0, 19.0]),
            ("s9", [0.0, 121.0, 83.0, 19.0]),
            ("s10", [0.0, 140.0, 83.0, 19.0]),
            ("s12", [0.0, 159.0, 40.0, 19.0]),
            ("s13", [0.0, 178.0, 83.0, 19.0]),
            ("s14", [0.0, 197.0, 90.0, 19.0]),
        ],
    );
}

#[test]
fn selected_count_label_snaps_to_device_pixels() {
    check(
        2,
        &[
            ("s1", [0.0, 0.0, 83.0, 19.0]),
            ("s7", [0.0, 76.0, 113.5, 26.0]),
            ("s14", [0.0, 197.0, 89.5, 19.0]),
        ],
    );
}

#[test]
fn grouped_options_widen_the_dropdown_by_an_indent() {
    let rows = |o4: f64, o5: f64, o6: f64| {
        [
            ("o1", [0.0, 0.0, 71.0, 19.0]),
            ("o2", [0.0, 19.0, 71.0, 19.0]),
            ("o3", [0.0, 38.0, 68.0, 19.0]),
            ("o4", [0.0, 57.0, o4, 19.0]),
            ("o5", [0.0, 76.0, 93.0, o5]),
            ("o6", [0.0, o6, 83.0, 19.0]),
            ("o7", [0.0, o6 + 19.0, 83.0, 19.0]),
        ]
    };
    check_page(GROUPED, 1, &rows(127.0, 26.0, 102.0));
    check_page(GROUPED, 2, &rows(126.5, 27.0, 103.0));
}

#[test]
fn adding_a_selected_attribute_keeps_the_other_picks() {
    let source = "<!doctype html><meta charset=utf-8><style>html,body{margin:0;font:14px Arial}\
         </style><select id=m multiple size=1><optgroup label=g><option id=a>A</option>\
         </optgroup><option selected>B</option></select>";
    let page: Page<()> = App::new((), source)
        .page(640, 400)
        .expect("the page parses");
    let mut harness = Harness::wrap(page);
    harness.pump();
    harness.doc.ui().select("#a").set_attr("selected", "");
    harness.pump();
    let doc = harness.base();
    let picks = doc
        .query_selector_all("#m option:checked")
        .expect("valid selector")
        .len();
    assert_eq!(picks, 2, "both options stay picked");
    let select = doc
        .query_selector("#m")
        .expect("valid selector")
        .expect("the select exists");
    let rect = doc
        .get_client_bounding_rect(select)
        .expect("the select has a box");
    assert_eq!([rect.width, rect.height], [83.0, 19.0]);
    drop(doc);
}
