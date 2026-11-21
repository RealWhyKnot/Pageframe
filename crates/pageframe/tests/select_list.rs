mod support;

use pageframe::events::Modifiers;
use pageframe::{App, Event, EventKind, Key, NamedKey, Page, Ui};
use pageframe_test_harness::Harness;
use support::{open, pixel, render};

type Opened = Harness<Page<Vec<String>>>;

const ROW: f32 = 17.0;
const PICKED: [u8; 4] = [25, 103, 210, 255];
const PICKED_UNFOCUSED: [u8; 4] = [206, 206, 206, 255];

#[cfg(target_os = "macos")]
const TOGGLE: Modifiers = Modifiers::META;
#[cfg(not(target_os = "macos"))]
const TOGGLE: Modifiers = Modifiers::CONTROL;

const PAGE: &str = r#"<html><head><style>
html{background:white}
body{margin:0;font:16px Arial}
select{position:absolute}
#many{left:0;top:0}
#two{left:70px;top:0}
#big{left:0;top:100px;font-size:20px}
#off{left:70px;top:40px}
</style></head><body>
<select id="many" multiple><option>Multi one</option><option>Multi two</option><option disabled>Multi three</option><option>Four</option><option>Five</option><option>Six</option></select>
<select id="two" size="2"><option value="s1">Size one</option><option>Size two</option><option>Size three</option></select>
<select id="big" size="3"><optgroup label="Group"><option>In group</option></optgroup><option>Out</option></select>
<select id="off" multiple disabled><option>Off one</option></select>
</body></html>"#;

fn page() -> Opened {
    open(
        App::new(Vec::new(), PAGE)
            .on("#many", EventKind::Input, record)
            .on("#two", EventKind::Input, record)
            .on("#off", EventKind::Input, record),
    )
}

fn record(seen: &mut Vec<String>, _: &mut Ui<'_>, event: &mut Event<'_>) {
    seen.push(event.value().unwrap_or_default().to_owned());
}

fn value(harness: &mut Opened, selector: &str) -> Option<String> {
    harness.doc.ui().select(selector).value()
}

fn row_point(harness: &Opened, selector: &str, row: u8) -> (f32, f32) {
    let rect = harness.layout_rect(selector);
    (
        rect.x + 10.0,
        rect.y + 1.0 + ROW * f32::from(row) + ROW / 2.0,
    )
}

fn click_row(harness: &mut Opened, selector: &str, row: u8) {
    let (x, y) = row_point(harness, selector, row);
    harness.click_at(x, y);
}

fn toggle_row(harness: &mut Opened, selector: &str, row: u8) {
    let (x, y) = row_point(harness, selector, row);
    harness.click_at_with(x, y, TOGGLE);
}

fn size(harness: &Opened, selector: &str) -> (f32, f32) {
    let rect = harness.layout_rect(selector);
    (rect.width, rect.height)
}

#[test]
fn list_boxes_take_the_sizes_chrome_gives_them() {
    let harness = page();
    assert_eq!(size(&harness, "#many"), (68.0, 70.0));
    assert_eq!(size(&harness, "#two"), (66.0, 36.0));
    assert_eq!(size(&harness, "#big"), (97.0, 77.0));
    let option = harness.layout_rect("#many option");
    assert_eq!((option.x, option.y, option.height), (1.0, 1.0, ROW));
}

#[test]
fn a_list_box_starts_with_nothing_picked() {
    let mut harness = page();
    assert_eq!(value(&mut harness, "#many"), None);
    assert_eq!(value(&mut harness, "#two"), None);
    assert!(!harness.base().select_popup_is_open());
}

#[test]
fn clicking_a_row_picks_it_and_fires_input() {
    let mut harness = page();
    click_row(&mut harness, "#two", 0);
    assert_eq!(value(&mut harness, "#two").as_deref(), Some("s1"));
    assert_eq!(harness.doc.state().as_slice(), ["s1"]);
    assert_eq!(harness.focused(), Some(harness.node("#two")));
    assert!(!harness.base().select_popup_is_open());
    click_row(&mut harness, "#two", 0);
    assert_eq!(harness.doc.state().len(), 1);
    click_row(&mut harness, "#two", 1);
    assert_eq!(value(&mut harness, "#two").as_deref(), Some("Size two"));
    assert_eq!(harness.doc.state().as_slice(), ["s1", "Size two"]);
}

#[test]
fn the_picked_row_is_blue_with_focus_and_grey_without() {
    let mut harness = page();
    click_row(&mut harness, "#two", 1);
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 110, 1 + 17 + 2), PICKED);
    click_row(&mut harness, "#many", 0);
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 110, 1 + 17 + 2), PICKED_UNFOCUSED);
    assert_eq!(pixel(&buffer, 60, 3), PICKED);
}

#[test]
fn the_toggle_key_adds_and_removes_rows_in_a_multiple_list() {
    let mut harness = page();
    click_row(&mut harness, "#many", 0);
    toggle_row(&mut harness, "#many", 3);
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 60, 3), PICKED);
    assert_eq!(pixel(&buffer, 60, 1 + 17 * 3 + 2), PICKED);
    assert_eq!(value(&mut harness, "#many").as_deref(), Some("Multi one"));
    toggle_row(&mut harness, "#many", 0);
    assert_eq!(value(&mut harness, "#many").as_deref(), Some("Four"));
    toggle_row(&mut harness, "#many", 3);
    assert_eq!(value(&mut harness, "#many"), None);
    click_row(&mut harness, "#many", 1);
    assert_eq!(value(&mut harness, "#many").as_deref(), Some("Multi two"));
    let buffer = render(&mut harness);
    assert_ne!(pixel(&buffer, 60, 3), PICKED);
}

#[test]
fn the_toggle_key_does_not_unpick_in_a_single_list() {
    let mut harness = page();
    click_row(&mut harness, "#two", 0);
    toggle_row(&mut harness, "#two", 0);
    assert_eq!(value(&mut harness, "#two").as_deref(), Some("s1"));
    toggle_row(&mut harness, "#two", 1);
    assert_eq!(value(&mut harness, "#two").as_deref(), Some("Size two"));
}

#[test]
fn disabled_rows_and_disabled_lists_ignore_clicks() {
    let mut harness = page();
    click_row(&mut harness, "#many", 2);
    assert_eq!(value(&mut harness, "#many"), None);
    assert_eq!(harness.focused(), Some(harness.node("#many")));
    click_row(&mut harness, "#off", 0);
    assert_eq!(value(&mut harness, "#off"), None);
    assert_eq!(harness.doc.state().len(), 0);
}

#[test]
fn arrow_keys_move_the_pick_and_skip_disabled_rows() {
    let mut harness = page();
    click_row(&mut harness, "#many", 1);
    harness.press(Key::Named(NamedKey::ArrowDown));
    assert_eq!(value(&mut harness, "#many").as_deref(), Some("Four"));
    harness.press(Key::Named(NamedKey::ArrowUp));
    assert_eq!(value(&mut harness, "#many").as_deref(), Some("Multi two"));
    harness.press(Key::Named(NamedKey::End));
    assert_eq!(value(&mut harness, "#many").as_deref(), Some("Six"));
    harness.press(Key::Named(NamedKey::Home));
    assert_eq!(value(&mut harness, "#many").as_deref(), Some("Multi one"));
    assert_eq!(
        harness.doc.state().as_slice(),
        ["Multi two", "Four", "Multi two", "Six", "Multi one"]
    );
    harness.press(Key::Named(NamedKey::Enter));
    assert!(!harness.base().select_popup_is_open());
}

#[test]
fn changing_size_switches_between_list_and_dropdown() {
    let mut harness = page();
    harness.doc.ui().select("#two").set_attr("size", "1");
    harness.pump();
    assert_eq!(size(&harness, "#two").1, 19.0);
    assert_eq!(value(&mut harness, "#two").as_deref(), Some("s1"));
    harness.doc.ui().select("#two").set_attr("size", "3");
    harness.pump();
    assert_eq!(size(&harness, "#two").1, 53.0);
    assert_eq!(value(&mut harness, "#two").as_deref(), Some("s1"));
}

#[test]
fn a_multiple_list_turned_into_a_dropdown_keeps_every_pick() {
    let mut harness = page();
    click_row(&mut harness, "#many", 0);
    toggle_row(&mut harness, "#many", 3);
    harness.doc.ui().select("#many").set_attr("size", "1");
    harness.pump();
    let picks = |harness: &Opened| {
        harness
            .base()
            .query_selector_all("#many option:checked")
            .expect("valid selector")
            .len()
    };
    assert_eq!(size(&harness, "#many").1, 19.0);
    assert_eq!(value(&mut harness, "#many").as_deref(), Some("Multi one"));
    assert_eq!(picks(&harness), 2);
    harness.press(Key::Named(NamedKey::ArrowUp));
    harness.press(Key::Named(NamedKey::ArrowDown));
    assert_eq!(value(&mut harness, "#many").as_deref(), Some("Multi one"));
    assert_eq!(picks(&harness), 2);
}

#[test]
fn clicking_a_group_label_leaves_the_pick_alone() {
    let mut harness = page();
    let rect = harness.layout_rect("#big");
    harness.click_at(rect.x + 10.0, rect.y + 1.0 + 25.0 + 12.0);
    assert_eq!(value(&mut harness, "#big").as_deref(), Some("In group"));
    harness.click_at(rect.x + 10.0, rect.y + 1.0 + 12.0);
    assert_eq!(value(&mut harness, "#big").as_deref(), Some("In group"));
    harness.click_at_with(rect.x + 10.0, rect.y + 1.0 + 12.0, TOGGLE);
    assert_eq!(value(&mut harness, "#big").as_deref(), Some("In group"));
    assert_eq!(harness.focused(), Some(harness.node("#big")));
}

#[test]
fn oversized_lists_stay_inside_their_rows() {
    let mut harness = page();
    let options: String = (0..3_000)
        .map(|index| format!("<option>Item {index}</option>"))
        .collect();
    harness.doc.ui().select("#two").set_html(&options);
    harness.pump();
    assert_eq!(size(&harness, "#two").1, 36.0);
    harness
        .doc
        .ui()
        .select("#two")
        .set_attr("size", "99999999999");
    harness.pump();
    assert_eq!(size(&harness, "#two").1, 170_002.0);
    harness.doc.ui().select("#two").set_attr("size", "2x");
    harness
        .doc
        .ui()
        .select("#two")
        .set_html(&format!("<option>{}</option>", "W".repeat(20_000)));
    harness.pump();
    assert_eq!(size(&harness, "#two").1, 36.0);
    click_row(&mut harness, "#two", 0);
    assert_eq!(
        value(&mut harness, "#two").map(|text| text.len()),
        Some(20_000)
    );
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 110, 36 + 2), [255, 255, 255, 255]);
}
