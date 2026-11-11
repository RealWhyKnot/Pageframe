mod support;

use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use support::{open, pixel, render};

type Opened = Harness<Page<()>>;

const PAGE: &str = r#"<html><head><style>
body{margin:0;font:16px Arial}
#narrow{width:60px}
</style></head><body>
<select id="fruit"><option value="a">Apple</option><option selected>Banana
  split</option><option disabled>Cherry</option></select>
<select id="plain"><option disabled>Pick</option><option>One</option><option>Two</option></select>
<select id="none"></select>
<select id="narrow"><option>x</option></select>
</body></html>"#;

fn value(harness: &mut Opened, selector: &str) -> Option<String> {
    harness.doc.ui().select(selector).value()
}

fn size(harness: &Opened, selector: &str) -> (f32, f32) {
    let rect = harness.layout_rect(selector);
    (rect.width, rect.height)
}

#[test]
fn value_is_the_selected_option() {
    let mut harness = open(App::new((), PAGE));
    assert_eq!(
        value(&mut harness, "#fruit").as_deref(),
        Some("Banana split")
    );
    assert_eq!(value(&mut harness, "#plain").as_deref(), Some("One"));
    assert_eq!(value(&mut harness, "#none"), None);
}

#[test]
fn set_value_picks_the_matching_option() {
    let mut harness = open(App::new((), PAGE));
    harness.doc.ui().select("#fruit").set_value("a");
    assert_eq!(value(&mut harness, "#fruit").as_deref(), Some("a"));
    harness.doc.ui().select("#fruit").set_value("missing");
    assert_eq!(value(&mut harness, "#fruit"), None);
    harness.doc.ui().select("#fruit").set_value("Cherry");
    assert_eq!(value(&mut harness, "#fruit").as_deref(), Some("Cherry"));
}

#[test]
fn selected_attribute_moves_the_selection() {
    let mut harness = open(App::new((), PAGE));
    harness
        .doc
        .ui()
        .select("#plain option:nth-child(3)")
        .set_attr("selected", "");
    assert_eq!(value(&mut harness, "#plain").as_deref(), Some("Two"));
    assert_eq!(harness.query_all("#plain option:checked").len(), 1);
}

#[test]
fn new_options_get_a_selection() {
    let mut harness = open(App::new((), PAGE));
    harness
        .doc
        .ui()
        .select("#fruit")
        .set_html("<option>X</option><option>Y</option>");
    assert_eq!(value(&mut harness, "#fruit").as_deref(), Some("X"));
    harness
        .doc
        .ui()
        .select("#fruit")
        .set_html("<option>X</option><option selected>Y</option><option selected>Z</option>");
    assert_eq!(value(&mut harness, "#fruit").as_deref(), Some("Z"));
    harness.doc.ui().select("#fruit").set_html("");
    assert_eq!(value(&mut harness, "#fruit"), None);
}

#[test]
fn closed_select_is_one_line_as_wide_as_its_longest_option() {
    let mut harness = open(App::new((), PAGE));
    let (wide, height) = size(&harness, "#fruit");
    assert!((height - 19.0).abs() < 0.01, "height {height}");
    assert!((wide - 96.0).abs() < 0.01, "width {wide}");
    harness.doc.ui().select("#fruit").set_value("a");
    harness.pump();
    assert_eq!(size(&harness, "#fruit"), (wide, height));
    let shown = harness.layout_rect("#fruit option:checked");
    let hidden = harness.layout_rect("#fruit option:not(:checked)");
    assert!(shown.height > 10.0, "shown {}", shown.height);
    assert!(hidden.height.abs() < 0.01, "hidden {}", hidden.height);
}

#[test]
fn grouped_options_count_as_options() {
    let mut harness = open(App::new((), PAGE));
    harness.doc.ui().select("#plain").set_html(
        "<optgroup label=\"G\"><option>In</option><option selected>Second</option></optgroup>         <optgroup disabled><option>Off</option></optgroup><option>Out</option>",
    );
    harness.pump();
    assert_eq!(value(&mut harness, "#plain").as_deref(), Some("Second"));
    let (_, height) = size(&harness, "#plain");
    assert!((height - 19.0).abs() < 0.01, "height {height}");
    harness.doc.ui().select("#plain").set_value("Out");
    assert_eq!(value(&mut harness, "#plain").as_deref(), Some("Out"));
}

#[test]
fn closed_select_paints_its_arrow_and_only_the_selected_text() {
    let mut harness = open(App::new((), PAGE));
    let buffer = render(&mut harness);
    let dark = |x: usize, y: usize| pixel(&buffer, x, y)[0] < 80;
    let arrow = (83..93).flat_map(|x| (6..13).map(move |y| (x, y)));
    assert!(arrow.filter(|&(x, y)| dark(x, y)).count() >= 8);
    let text = (4..80).flat_map(|x| (2..17).map(move |y| (x, y)));
    assert!(text.filter(|&(x, y)| dark(x, y)).count() >= 20);
    harness
        .doc
        .ui()
        .select("#fruit option:first-child")
        .set_text("Pear");
    harness.pump();
    let after = render(&mut harness);
    let changed = after.iter().zip(&buffer).filter(|(a, b)| a != b).count();
    assert_eq!(changed, 0);
}

#[test]
fn empty_selects_keep_one_line() {
    let mut harness = open(App::new((), PAGE));
    assert_eq!(size(&harness, "#none"), (22.0, 19.0));
    harness
        .doc
        .ui()
        .select("#none")
        .set_html("<option></option>");
    harness.pump();
    assert_eq!(size(&harness, "#none"), (22.0, 19.0));
}

#[test]
fn oversized_options_stay_inside_a_sized_select() {
    let mut harness = open(App::new((), PAGE));
    let rows: String = (0..2_000)
        .map(|index| format!("<option>Row {index}</option>"))
        .collect();
    let html = format!("<option>{}</option>{rows}", "W".repeat(20_000));
    harness.doc.ui().select("#narrow").set_html(&html);
    harness.pump();
    assert_eq!(size(&harness, "#narrow"), (60.0, 19.0));
    assert_eq!(
        value(&mut harness, "#narrow").map(|text| text.len()),
        Some(20_000)
    );
}
